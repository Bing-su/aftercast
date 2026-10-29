using System.Diagnostics;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    // One monitor makes timestamp/zone snapshots and combat transitions consistent without lock-order inversion.
    private readonly object logGate = ActGlobals.actionDataLock;
    private const int MaximumRetiredFollowers = 32;
    private readonly HashSet<LogReader> retiredFollowers = new();
    private LogReader? follower;
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
    // Live timers keep aging on their monotonic baseline while an import changes parser timestamps.
    public DateTime LastEstimatedTime { get { lock (logGate) return activeImport?.LiveEstimatedTime ?? lastKnownTime + Stopwatch.GetElapsedTime(lastKnownStamp); } }
    public string CurrentZone { get { lock (logGate) return currentZone; } set { lock (logGate) currentZone = value; } }
    public event LogLineEventDelegate? OnLogLineRead;

    public void OpenLog(bool getCurrentZone, bool getCharNameFromFile) => OpenLogCore(getCurrentZone, getCharNameFromFile, [], TimeSpan.FromSeconds(30));

    internal void OpenLogWithCompanions(bool getCurrentZone, IReadOnlyList<Aftercast.ActCompat.ImportCompanion> companions, TimeSpan? maxClockWait, Aftercast.ActCompat.ImportTextOptions? mainText = null)
    {
        ArgumentNullException.ThrowIfNull(companions);
        mainText?.Validate();
        var selected = companions.ToArray();
        if (selected.Length > 8) throw new ArgumentException("At most eight live companions can be followed");
        var names = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var source in selected)
        {
            ArgumentNullException.ThrowIfNull(source);
            CompanionLogCatalog.ValidateName(source.Name);
            if (!names.Add(source.Name)) throw new ArgumentException("Duplicate live companion name");
            source.Text?.Validate();
        }
        var wait = maxClockWait ?? TimeSpan.FromSeconds(30);
        if (wait <= TimeSpan.Zero || wait > TimeSpan.FromMinutes(5)) throw new ArgumentOutOfRangeException(nameof(maxClockWait));
        OpenLogCore(getCurrentZone, false, selected.OrderBy(source => source.Name, StringComparer.OrdinalIgnoreCase).ToArray(), wait, mainText);
    }

    internal Aftercast.ActCompat.LiveLogSourceStatus[] ReadLiveLogSources()
    {
        LogReader? current;
        bool paused;
        lock (logGate) { current = follower; paused = ReadThreadLock || logPauseDepth != 0; }
        if (current is LiveCompanionGroup group) return group.ReadStatus();
        if (current is LogFollower single) return [new("", single.SourcePath, !single.IsAlive ? "failed" : paused ? "paused" : "following", 0,
            null, single.ImageVersion, single.Dropped, single.LastDiagnostic)];
        return [];
    }

    private void OpenLogCore(bool getCurrentZone, bool getCharNameFromFile, Aftercast.ActCompat.ImportCompanion[] companions, TimeSpan maxClockWait, Aftercast.ActCompat.ImportTextOptions? mainText = null)
    {
        var encoding = mainText?.Resolve() ?? Aftercast.ActCompat.ImportTextOptions.Validate(LogEncoding);
        EnsureCanWaitForLogReader();
        if (getCharNameFromFile)
            throw new PlatformNotSupportedException("Character-name discovery from log filenames is not implemented");
        LogReader? previous;
        string path;
        long generation;
        lock (logGate)
        {
            ObjectDisposedException.ThrowIf(logHostStopped, this);
            PruneRetiredFollowers();
            // Reserve a cleanup slot for a candidate whose snapshot fails during repeated reentrant replacement.
            var candidateCost = companions.Length == 0 ? 1 : companions.Length + 2;
            if (retiredFollowers.Sum(reader => reader.ResourceCost) + (follower?.ResourceCost ?? 0) + candidateCost > MaximumRetiredFollowers)
                throw new InvalidOperationException("Too many log readers are still retiring");
            path = logFilePath;
            generation = ++logGeneration;
            previous = follower;
            if (previous is not null) retiredFollowers.Add(previous);
            follower = null;
        }
        // Never join under the root gate: an in-flight subscriber may open another log or read host state.
        if (previous is not null) DisposeRetiredFollower(previous);
        LogReader? next = null;
        try
        {
            lock (logGate)
            {
                if (logHostStopped || generation != logGeneration) return;
                next = companions.Length == 0
                    ? new LogFollower(path, line => ReceiveLogLine(generation, path, line),
                        error => WriteExceptionLog(error, "Log follower"), () => ReadThreadLock || IsLogDeliveryPaused(generation), initialSnapshot: true, encoding: encoding, imageChanged: null, detectBom: mainText?.DetectBom ?? true)
                    : new LiveCompanionGroup(path, encoding, companions, maxClockWait, GetLiveTimestamp,
                        () => LastEstimatedTime, () => ReadThreadLock || IsLogDeliveryPaused(generation),
                        (source, name, line, time, valid) => ReceiveLogLine(generation, source, line, name, time, valid),
                        error => WriteExceptionLog(error, "Live companion"), mainText?.DetectBom ?? true);
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

    internal long LiveLogGeneration { get { lock (logGate) return logGeneration; } }
    internal void CloseLiveLog(long expectedGeneration)
    {
        EnsureCanWaitForLogReader();
        LogReader? previous;
        lock (logGate)
        {
            if (logHostStopped || logGeneration != expectedGeneration) throw new InvalidOperationException("Live selection is stale");
            ++logGeneration; readyGeneration = 0;
            previous = follower; follower = null;
            if (previous is not null) retiredFollowers.Add(previous);
        }
        if (previous is not null) DisposeRetiredFollower(previous);
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

    private LogReader[] ReadersUnderGate() => follower is null
        ? retiredFollowers.ToArray()
        : retiredFollowers.Append(follower).ToArray();

    private void DisposeRetiredFollower(LogReader reader)
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
        LogReader[] readers;
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
    private LogSuspension SuspendLogReading(bool importOperation, bool wait = true)
    {
        // Import has already performed lifecycle preflight before reserving its exclusive scope.
        if (!importOperation) EnsureCanWaitForLogReader();
        LogReader[] readers;
        lock (logGate)
        {
            logPauseDepth++;
            PruneRetiredFollowers();
            readers = ReadersUnderGate();
        }
        var suspended = new List<LogReader>();
        try
        {
            foreach (var reader in readers)
            {
                if (wait) reader.Suspend(PumpOwnerCallbacks);
                else reader.RequestSuspension();
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

    private sealed class LogSuspension(FormActMain root, List<LogReader> readers) : IDisposable
    {
        internal bool IsReady => readers.All(reader => !reader.CallbackActive);
        public void Dispose()
        {
            foreach (var reader in readers) reader.Resume();
            lock (root.logGate) root.logPauseDepth--;
        }
    }

    private DateTime GetLiveTimestamp(string line) => GetDateTimeFromLog(line);

    private void ReceiveLogLine(long generation, string sourcePath, string line, string companionName = "", DateTime? parsedTime = null, Func<bool>? sourceCurrent = null)
    {
        bool Valid() => IsCurrentLog(generation) && (sourceCurrent?.Invoke() ?? true);
        if (!Valid()) return;
        var timestamp = parsedTime ?? GetDateTimeFromLog(line);
        DateTime detectedTime;
        string zone;
        int timeSorter;
        lock (logGate)
        {
            if (logHostStopped || generation != logGeneration || !(sourceCurrent?.Invoke() ?? true)) return;
            if (companionName.Length == 0 || timestamp > lastKnownTime) LastKnownTime = timestamp;
            GlobalTimeSorter++;
            timeSorter = GlobalTimeSorter;
            if (companionName.Length == 0) UpdateRawZone(line);
            detectedTime = companionName.Length == 0 ? lastKnownTime : timestamp;
            zone = currentZone;
        }
        var wasInCombat = InCombat;
        CheckIdleEndCombat();
        var info = new LogLineEventArgs(line, 0, detectedTime, zone, InCombat, companionName);
        if (!Valid()) return;
        PublishOriginalLogRecord(false, sourcePath, line, detectedTime, timeSorter, companionName);
        InvokeLogHandlers(BeforeLogLineRead, false, info, generation, sourceCurrent);
        LastLogLine = info.logLine;
        if (!Valid()) return;
        RecordEncounterLog(info, timeSorter, wasInCombat);
        var triggerText = info.logLine;
        InvokeLogHandlers(OnLogLineRead, false, info, generation, sourceCurrent);
        if (Valid()) ProcessCustomTriggerRecord(line, info, false, timeSorter, triggerText);
    }

    private void InvokeLogHandlers(LogLineEventDelegate? handlers, bool isImport, LogLineEventArgs info, long? generation = null, Func<bool>? sourceCurrent = null)
    {
        foreach (var callback in Delegate.EnumerateInvocationList(handlers))
        {
            if (generation is { } active && (!IsCurrentLog(active) || !(sourceCurrent?.Invoke() ?? true))) return;
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
        LogReader[] readers;
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
        if (!disposing) { base.Dispose(false); return; }
        if (!EnsureCapturedImportShutdownAllowed()) EnsureCanWaitForLogReader();
        var errors = new List<Exception>();
        try { CloseHistorySourceCapture(); } catch (Exception error) { errors.Add(error); }
        try { CancelCapturedImportForShutdown(); } catch (Exception error) { errors.Add(error); }
        try { BeginCombatShutdown(); } catch (Exception error) { errors.Add(error); }
        try { CloseNotifications(); } catch (Exception error) { errors.Add(error); }
        try { StopLogFollower(); } catch (Exception error) { errors.Add(error); }
        try { StopCombatProcessing(); } catch (Exception error) { errors.Add(error); }
        try { DisposePluginSettingsWindow(); } catch (Exception error) { errors.Add(error); }
        try { DisposeColorScalars(); } catch (Exception error) { errors.Add(error); }
        try { base.Dispose(true); } catch (Exception error) { errors.Add(error); }
        if (errors.Count != 0) throw new AggregateException("ACT root disposal failed", errors);
    }
}
