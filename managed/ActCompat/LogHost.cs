using System.Diagnostics;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    // One monitor makes timestamp/zone snapshots and combat transitions consistent without lock-order inversion.
    private readonly object logGate = ActGlobals.actionDataLock;
    private const int MaximumRetiredFollowers = 32;
    private readonly HashSet<LogFollower> retiredFollowers = new();
    private LogFollower? follower;
    private long logGeneration;
    private long readyGeneration;
    private bool logHostStopped;
    private int logPauseDepth;
    internal Action? PumpCallbacks { get; set; }
    internal Func<bool>? IsDispatchingCallback { get; set; }
    private string logFilePath = "", logFileFilter = "eq2log*.txt", currentZone = "";
    private DateTime lastKnownTime = DateTime.Now;
    private long lastKnownStamp = Stopwatch.GetTimestamp();

    public string LogFileFilter { get { lock (logGate) return logFileFilter; } set { lock (logGate) logFileFilter = value; } }
    public string LogFilePath { get { lock (logGate) return logFilePath; } set { lock (logGate) logFilePath = value; } }
    public DateTime LastKnownTime
    {
        get { lock (logGate) return lastKnownTime; }
        set
        {
            if (value == DateTime.MinValue) return;
            lock (logGate) { lastKnownTime = value; lastKnownStamp = Stopwatch.GetTimestamp(); }
        }
    }
    public DateTime LastEstimatedTime { get { lock (logGate) return lastKnownTime + Stopwatch.GetElapsedTime(lastKnownStamp); } }
    public string CurrentZone { get { lock (logGate) return currentZone; } set { lock (logGate) currentZone = value; } }
    public event LogLineEventDelegate? OnLogLineRead;

    public void OpenLog(bool getCurrentZone, bool getCharNameFromFile)
    {
        EnsureCanWaitForLogReader();
        if (getCharNameFromFile)
            throw new PlatformNotSupportedException("Character-name discovery from log filenames is not implemented");
        LogFollower? previous;
        string path;
        long generation;
        lock (logGate)
        {
            ObjectDisposedException.ThrowIf(logHostStopped, this);
            PruneRetiredFollowers();
            // Reserve a cleanup slot for a candidate whose snapshot fails during repeated reentrant replacement.
            if (retiredFollowers.Count >= MaximumRetiredFollowers - 1)
                throw new InvalidOperationException("Too many log readers are still retiring");
            path = logFilePath;
            generation = ++logGeneration;
            previous = follower;
            if (previous is not null) retiredFollowers.Add(previous);
            follower = null;
        }
        // Never join under the root gate: an in-flight subscriber may open another log or read host state.
        if (previous is not null) DisposeRetiredFollower(previous);
        LogFollower? next = null;
        try
        {
            lock (logGate)
            {
                if (logHostStopped || generation != logGeneration) return;
                next = new LogFollower(path, line => ReceiveLogLine(generation, path, line),
                    error => WriteExceptionLog(error, "Log follower"), () => ReadThreadLock || IsLogDeliveryPaused(generation), initialSnapshot: true);
                follower = next;
            }
            var snapshot = getCurrentZone ? next.ReadInitialZone(ZoneChangeRegex, () =>
            {
                lock (logGate) return logHostStopped || generation != logGeneration;
            }) : null;
            lock (logGate)
            {
                if (logHostStopped || generation != logGeneration) return;
                if (getCurrentZone) currentZone = snapshot ?? "";
                next.CompleteInitialSnapshot();
                readyGeneration = generation;
            }
        }
        catch
        {
            lock (logGate)
            {
                if (ReferenceEquals(follower, next))
                {
                    if (next is not null) retiredFollowers.Add(next);
                    follower = null;
                    ++logGeneration;
                }
            }
            if (next is not null) DisposeRetiredFollower(next);
            throw;
        }
        foreach (var callback in Delegate.EnumerateInvocationList(LogFileChanged))
        {
            if (!IsCurrentLog(generation)) return;
            try { callback(false, path); }
            catch (Exception error) { WriteExceptionLog(error, "LogFileChanged subscriber"); }
        }
    }

    private bool IsCurrentLog(long generation)
    {
        lock (logGate) return !logHostStopped && generation == logGeneration && generation == readyGeneration;
    }

    private bool IsLogDeliveryPaused(long generation)
    {
        lock (logGate) return logPauseDepth > 0 || logHostStopped || generation != logGeneration || generation != readyGeneration;
    }

    private void PumpOwnerCallbacks()
    {
        if (Environment.CurrentManagedThreadId == ownerThread) PumpCallbacks?.Invoke();
    }

    private void PruneRetiredFollowers() => retiredFollowers.RemoveWhere(reader => !reader.IsAlive);

    private LogFollower[] ReadersUnderGate() => follower is null
        ? retiredFollowers.ToArray()
        : retiredFollowers.Append(follower).ToArray();

    private void DisposeRetiredFollower(LogFollower reader)
    {
        try { reader.Dispose(PumpOwnerCallbacks); }
        finally
        {
            // Self-disposal only signals cancellation; keep ownership until that callback has actually returned.
            lock (logGate) { if (!reader.IsAlive) retiredFollowers.Remove(reader); }
        }
    }

    internal void EnsureCanWaitForLogReader()
    {
        // An action callback cannot join a reader that may itself be waiting for the action drain.
        EnsureCanWaitForCombatLifecycle();
        if (Environment.CurrentManagedThreadId != ownerThread || IsDispatchingCallback?.Invoke() != true) return;
        LogFollower[] readers;
        lock (logGate) readers = ReadersUnderGate();
        // A dispatched UI callback must return before its waiting reader can finish delivery.
        if (readers.Any(reader => reader.CallbackActive))
            throw new InvalidOperationException("Cannot wait for a log callback from its own dispatched UI callback");
    }

    internal void RunWithLogReadingSuspended(Action action, bool importOperation = false)
    {
        ArgumentNullException.ThrowIfNull(action);
        using var suspension = SuspendLogReading(importOperation);
        action();
    }

    // Hold suspension across import steps, e.g. while disk input is pending between host ticks.
    private IDisposable SuspendLogReading(bool importOperation)
    {
        // Import has already performed lifecycle preflight before reserving its exclusive scope.
        if (!importOperation) EnsureCanWaitForLogReader();
        LogFollower[] readers;
        lock (logGate)
        {
            logPauseDepth++;
            PruneRetiredFollowers();
            readers = ReadersUnderGate();
        }
        var suspended = new List<LogFollower>();
        try
        {
            foreach (var reader in readers)
            {
                reader.Suspend(PumpOwnerCallbacks);
                suspended.Add(reader);
            }
            return new LogSuspension(this, suspended);
        }
        catch
        {
            foreach (var reader in suspended) reader.Resume();
            lock (logGate) logPauseDepth--;
            throw;
        }
    }

    private sealed class LogSuspension(FormActMain root, List<LogFollower> readers) : IDisposable
    {
        public void Dispose()
        {
            foreach (var reader in readers) reader.Resume();
            lock (root.logGate) root.logPauseDepth--;
        }
    }

    private void ReceiveLogLine(long generation, string sourcePath, string line)
    {
        if (!IsCurrentLog(generation)) return;
        var timestamp = GetDateTimeFromLog(line);
        DateTime detectedTime;
        string zone;
        int timeSorter;
        lock (logGate)
        {
            if (logHostStopped || generation != logGeneration) return;
            LastKnownTime = timestamp;
            GlobalTimeSorter++;
            timeSorter = GlobalTimeSorter;
            UpdateRawZone(line);
            detectedTime = lastKnownTime;
            zone = currentZone;
        }
        CheckIdleEndCombat();
        var info = new LogLineEventArgs(line, 0, detectedTime, zone, InCombat);
        if (!IsCurrentLog(generation)) return;
        PublishOriginalLogRecord(false, sourcePath, line, detectedTime, timeSorter);
        InvokeLogHandlers(BeforeLogLineRead, false, info, generation);
        LastLogLine = info.logLine;
        InvokeLogHandlers(OnLogLineRead, false, info, generation);
        if (IsCurrentLog(generation)) ProcessCustomTriggers(line, info, false);
    }

    private void InvokeLogHandlers(LogLineEventDelegate? handlers, bool isImport, LogLineEventArgs info, long? generation = null)
    {
        foreach (var callback in Delegate.EnumerateInvocationList(handlers))
        {
            if (generation is { } active && !IsCurrentLog(active)) return;
            try { callback(isImport, info); }
            catch (Exception error) { RecordImportFailure(error); WriteExceptionLog(error, "Log line subscriber"); }
        }
    }

    private void UpdateRawZone(string line)
    {
        var match = ZoneChangeRegex?.Match(line);
        if (match is { Success: true } && match.Groups["zone"].Success)
            currentZone = match.Groups["zone"].Value;
    }

    internal void StopLogFollower()
    {
        EnsureCanWaitForLogReader();
        LogFollower[] readers;
        lock (logGate)
        {
            logHostStopped = true;
            ++logGeneration;
            if (follower is not null) retiredFollowers.Add(follower);
            follower = null;
            readers = retiredFollowers.ToArray();
        }
        var errors = new List<Exception>();
        foreach (var reader in readers)
            try { DisposeRetiredFollower(reader); } catch (Exception error) { errors.Add(error); }
        if (errors.Count != 0) throw new AggregateException("Log reader shutdown failed", errors);
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing) { EnsureCanWaitForLogReader(); BeginCombatShutdown(); }
        try { if (disposing) StopLogFollower(); }
        finally
        {
            try { if (disposing) StopCombatProcessing(); }
            finally { base.Dispose(disposing); }
        }
    }
}
