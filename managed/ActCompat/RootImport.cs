using System.Diagnostics;
using System.Runtime.ExceptionServices;
using System.Threading.Channels;
using Aftercast.ActCompat;

namespace Aftercast.ActCompat
{
    // Counts describe raw hook dispatch, e.g. a parser can ignore a delivered line.
    public sealed record ImportResult(long LinesRead, long RecordsDispatched, int Encounters,
        int Combatants, long Damage, long Healed);
    public sealed record ImportProgress(long LinesRead, long RecordsDispatched, long BytesRead, long TotalBytes);
}

namespace Advanced_Combat_Tracker
{
    public partial class FormActMain
    {
        private ImportOperation? activeImport;

        private void EnsureImportAdmission()
        {
            if (activeTableQuery) throw new InvalidOperationException("Combat mutations are not permitted during plugin table callbacks");
            // Pumped plugin callbacks cannot mutate the graph owned by an import step.
            if (activeImport is { Exclusive: true } operation &&
                (Environment.CurrentManagedThreadId != operation.OwnerThread || IsDispatchingCallback?.Invoke() == true))
                throw new InvalidOperationException("Combat state is owned by an active log import");
        }

        private void RecordImportFailure(Exception error)
        {
            lock (CombatGate) { if (activeImport is { } operation) operation.Failure ??= error; }
        }
        // Keep the live parser context together; both capture and restoration require CombatGate.
        private sealed class LiveParserContext(FormActMain root)
        {
            private readonly ZoneData zone = root.combatZone;
            internal readonly string Path = root.logFilePath;
            private readonly string currentZone = root.currentZone;
            private readonly string? character = ActGlobals.charName;
            private readonly DateTime time = root.lastKnownTime, hostileTime = root.lastHostileTime;
            private readonly long clock = root.lastKnownStamp;
            private readonly (string Name, DateTime Start) zoneContext = root.zoneContext;
            private readonly bool readLock = root.ReadThreadLock, importing = root.importingLogs;
            internal DateTime EstimatedTime => time + Stopwatch.GetElapsedTime(clock);

            internal void Restore()
            {
                root.combatZone = zone;
                root.logFilePath = Path;
                root.currentZone = currentZone;
                ActGlobals.charName = character;
                root.lastKnownTime = time;
                // Continue the live estimate across import, e.g. an existing timer still ages during replay.
                root.lastKnownStamp = clock;
                root.lastHostileTime = hostileTime;
                root.zoneContext = zoneContext;
                root.inCombat = false;
                root.importingLogs = importing;
                root.ReadThreadLock = readLock;
            }
        }

        internal void BeginImport(string path, ImportRange? range = null)
        {
            ArgumentException.ThrowIfNullOrEmpty(path);
            if (!Path.IsPathFullyQualified(path)) throw new ArgumentException("An absolute import path is required", nameof(path));
            if (Environment.CurrentManagedThreadId != ownerThread || IsDispatchingCallback?.Invoke() == true)
                throw new InvalidOperationException("Import must start outside a dispatched callback on the owner thread");
            EnsureCanWaitForLogReader();
            lock (CombatGate)
            {
                EnsureCombatOpen();
                if (inCombat) throw new InvalidOperationException("End live combat before importing a log file");
                activeImport = new ImportOperation(this, path, range);
            }
            try { activeImport.Start(); }
            catch
            {
                try { activeImport.AbortStart(); }
                finally { lock (CombatGate) activeImport = null; }
                throw;
            }
        }

        internal ImportProgress CurrentImportProgress => activeImport?.Progress ?? throw new InvalidOperationException("No active import");

        internal ImportResult? AdvanceImport(bool cancel)
        {
            var operation = activeImport ?? throw new InvalidOperationException("No active import");
            try { return operation.Advance(cancel); }
            finally { if (operation.Completed) lock (CombatGate) activeImport = null; }
        }

        internal ImportResult ImportLog(string path, ImportRange? range = null)
        {
            BeginImport(path, range);
            while (true)
            {
                var before = CurrentImportProgress;
                if (AdvanceImport(false) is { } result) return result;
                PumpOwnerCallbacks();
                // The synchronous adapter pumps while waiting, but does not busy-spin on slow input or callbacks.
                if (CurrentImportProgress == before) Thread.Sleep(1);
            }
        }

        private sealed class ImportOperation(FormActMain root, string path, ImportRange? range)
        {
            internal int OwnerThread { get; } = Environment.CurrentManagedThreadId;
            internal bool Exclusive;
            internal Exception? Failure;
            internal bool Completed;
            private bool canceled, ending, prepared;
            private LogSuspension? suspension;
            private LiveParserContext? live;
            private ZoneData? importZone;
            private int firstEncounter;
            private ulong snapshotBefore;
            private long lines, dispatched, bytesRead, totalBytes;
            private readonly CancellationTokenSource cancellation = new();
            private readonly Channel<string[]> input = Channel.CreateBounded<string[]>(new BoundedChannelOptions(4) {
                SingleReader = true, SingleWriter = true, FullMode = BoundedChannelFullMode.Wait });
            private readonly Queue<string> buffered = new();
            private (string Line, DateTime Time)? nextLine;
            private Task? reading;
            private readonly List<Exception> failures = new();
            internal ImportProgress Progress => new(lines, dispatched, Interlocked.Read(ref bytesRead), Interlocked.Read(ref totalBytes));
            internal DateTime? LiveEstimatedTime => live?.EstimatedTime;

            internal void Start()
            {
                suspension = root.SuspendLogReading(true, wait: false);
            }

            private bool Prepare()
            {
                if (!suspension!.IsReady) return false;
                lock (root.CombatGate)
                {
                    if (!root.actionsIdle.IsSet || !root.combatTransitionsIdle.IsSet) return false;
                    Exclusive = true;
                    if (root.inCombat) throw new InvalidOperationException("End live combat before importing a log file");
                    root.EnsureCombatStateInitialized();
                    snapshotBefore = root.GetCombatSnapshotRevision();
                    importZone = root.combatZones[0];
                    live = new(root);
                    firstEncounter = importZone.Items.Count;
                    root.combatZone = importZone;
                    root.logFilePath = path;
                    root.currentZone = "Import Zone";
                    root.lastKnownTime = DateTime.MinValue;
                    root.lastKnownStamp = Stopwatch.GetTimestamp();
                    root.importingLogs = true;
                    root.ReadThreadLock = true;
                }
                root.NotifyImportFileChanged(true, path);
                // Only the input task owns the reader; parsing and ACT callbacks stay on the owner thread.
                reading = Task.Run(async () => {
                    var batch = new List<string>();
                    try
                    {
                        using var reader = new ImportLogReader(path);
                        Interlocked.Exchange(ref totalBytes, reader.TotalBytes);
                        var characters = 0;
                        while (await reader.ReadAsync(cancellation.Token).ConfigureAwait(false) is { } line)
                        {
                            cancellation.Token.ThrowIfCancellationRequested();
                            batch.Add(line);
                            characters += line.Length;
                            // Bound both scheduling overhead and read-ahead memory, including one maximum-size record.
                            if (batch.Count == 64 || characters >= 128 * 1024)
                            {
                                await input.Writer.WriteAsync(batch.ToArray(), cancellation.Token).ConfigureAwait(false);
                                batch.Clear(); characters = 0;
                            }
                            Interlocked.Exchange(ref bytesRead, reader.BytesRead);
                        }
                        if (batch.Count != 0) await input.Writer.WriteAsync(batch.ToArray(), cancellation.Token).ConfigureAwait(false);
                        input.Writer.TryComplete();
                    }
                    catch (Exception error)
                    {
                        // Preserve complete records before a malformed suffix, e.g. valid damage followed by bad UTF-8.
                        try
                        {
                            if (batch.Count != 0 && !cancellation.IsCancellationRequested)
                                await input.Writer.WriteAsync(batch.ToArray(), cancellation.Token).ConfigureAwait(false);
                        }
                        catch (OperationCanceledException) { }
                        input.Writer.TryComplete(error);
                    }
                });
                prepared = true;
                return true;
            }

            internal void AbortStart()
            {
                try { if (live is not null) lock (root.CombatGate) live.Restore(); }
                finally { Exclusive = false; suspension?.Dispose(); cancellation.Dispose(); }
            }

            internal ImportResult? Advance(bool cancel)
            {
                if (Completed) throw new InvalidOperationException("Import is complete");
                if (!prepared)
                {
                    try
                    {
                        if (cancel) throw new OperationCanceledException("Import canceled before parser state was acquired");
                        if (!Prepare()) return null;
                    }
                    catch
                    {
                        Completed = true;
                        AbortStart();
                        throw;
                    }
                }
                if (cancel && !canceled) { canceled = true; cancellation.Cancel(); ending = true; }
                if (!ending)
                {
                    var started = Stopwatch.GetTimestamp();
                    try
                    {
                        for (var count = 0; count < 64 && !ending && Stopwatch.GetElapsedTime(started) < TimeSpan.FromMilliseconds(5); count++)
                        {
                            if (nextLine is null)
                            {
                                if (buffered.Count == 0 && input.Reader.TryRead(out var batch))
                                    foreach (var record in batch) buffered.Enqueue(record);
                                if (!buffered.TryDequeue(out var line)) break;
                                nextLine = (line, line.Length == 0 ? DateTime.MinValue : root.GetDateTimeFromLog(line));
                            }
                            if (SkipOutsideRange(nextLine.Value.Line, nextLine.Value.Time))
                            {
                                nextLine = null;
                                continue;
                            }
                            if (!TryDispatch(nextLine.Value.Line, nextLine.Value.Time)) break;
                            nextLine = null;
                        }
                        if (!ending && nextLine is null && buffered.Count == 0 && input.Reader.Completion.IsCompleted)
                        {
                            input.Reader.Completion.GetAwaiter().GetResult();
                            ending = true;
                        }
                    }
                    catch (Exception error) { failures.Add(error); ending = true; cancellation.Cancel(); }
                }
                if (!ending) return null;
                // Unblock a producer waiting on the bounded channel before waiting for its resource disposal.
                while (input.Reader.TryRead(out _)) { }
                buffered.Clear();
                nextLine = null;
                if (reading is { IsCompleted: false }) return null;
                // Keep diagnostics responsive while an already accepted action callback is finishing.
                if (!root.actionsIdle.IsSet || !root.combatTransitionsIdle.IsSet) return null;
                return Finish();
            }

            private bool SkipOutsideRange(string line, DateTime time)
            {
                if (range is null || line.Length == 0 || time >= range.Start && time <= range.End) return false;
                lines++;
                if (time > range.End)
                {
                    // ACT stops at the first later timestamp, even if subsequent records go backwards.
                    ending = true;
                    cancellation.Cancel();
                }
                else
                {
                    // Recover preceding raw zone context without dispatching out-of-range plugin callbacks.
                    lock (root.CombatGate) root.UpdateRawZone(line);
                }
                return true;
            }

            private bool TryDispatch(string line, DateTime detectedTime)
            {
                if (line.Length == 0) { lines++; return true; }
                int sorter;
                lock (root.CombatGate)
                {
                    var time = detectedTime == DateTime.MinValue ? root.lastKnownTime : detectedTime;
                    // Keep this record pending until the previous encounter can end without a synchronous action drain.
                    if (root.inCombat && time - root.lastHostileTime > TimeSpan.FromSeconds(6) &&
                        (!root.actionsIdle.IsSet || !root.combatTransitionsIdle.IsSet)) return false;
                    root.LastKnownTime = detectedTime;
                    sorter = ++root.GlobalTimeSorter;
                    root.UpdateRawZone(line);
                }
                lines++;
                var wasInCombat = root.InCombat;
                root.CheckIdleEndCombat();
                var info = new LogLineEventArgs(line, 0, detectedTime, root.CurrentZone, root.InCombat);
                dispatched++;
                root.PublishOriginalLogRecord(true, path, line, detectedTime, sorter);
                root.InvokeLogHandlers(root.BeforeLogLineRead, true, info);
                root.LastLogLine = info.logLine;
                root.RecordEncounterLog(info, sorter, wasInCombat);
                var triggerText = info.logLine;
                root.InvokeLogHandlers(root.OnLogLineRead, true, info);
                root.ProcessCustomTriggerRecord(line, info, true, sorter, triggerText);
                return true;
            }

            private ImportResult Finish()
            {
                ImportResult result = new(lines, dispatched, 0, 0, 0, 0);
                try
                {
                    try { root.EndCombat(false); root.WaitForCombatActions(); }
                    catch (Exception error) { failures.Add(error); }
                    try
                    {
                        lock (root.CombatGate)
                        {
                            var encounters = 0; var combatants = 0; long damage = 0, healed = 0;
                            for (var index = firstEncounter; index < importZone!.Items.Count; index++)
                            {
                                var encounter = importZone.Items[index];
                                encounters++;
                                combatants = checked(combatants + encounter.NumCombatants);
                                damage = checked(damage + encounter.Damage);
                                healed = checked(healed + encounter.Healed);
                            }
                            result = new(lines, dispatched, encounters, combatants, damage, healed);
                        }
                    }
                    catch (Exception error) { failures.Add(error); }
                    finally
                    {
                        lock (root.CombatGate) live!.Restore();
                        root.NotifyImportFileChanged(false, live.Path);
                    }
                    if (Failure is { } callbackError) failures.Add(new InvalidDataException("Import subscriber or aggregation failed; partial history was retained", callbackError));
                    root.CompleteImportQueries(importZone!, firstEncounter, failures.Count == 0 && !canceled);
                    root.CompleteImportSnapshotState(snapshotBefore, failures.Count == 0 && !canceled);
                }
                finally
                {
                    Completed = true;
                    Exclusive = false;
                    suspension?.Dispose();
                    cancellation.Dispose();
                }
                if (failures.Count == 1) ExceptionDispatchInfo.Capture(failures[0]).Throw();
                if (failures.Count > 1) throw new AggregateException("Log import failed; partial history was retained", failures);
                if (canceled) throw new OperationCanceledException("Log import canceled; partial history was retained");
                return result;
            }
        }

        private void NotifyImportFileChanged(bool importing, string path)
        {
            foreach (var callback in Delegate.EnumerateInvocationList(LogFileChanged))
                try { callback(importing, path); }
                catch (Exception error) { RecordImportFailure(error); WriteExceptionLog(error, "LogFileChanged subscriber"); }
        }
    }
}
