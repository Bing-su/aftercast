using System.Runtime.ExceptionServices;
using Aftercast.ActCompat;

namespace Aftercast.ActCompat
{
    // RecordsDispatched measures raw hook dispatch, not whether a plugin accepted a combat record.
    public sealed record ImportResult(long LinesRead, long RecordsDispatched, int Encounters,
        int Combatants, long Damage, long Healed);
}

namespace Advanced_Combat_Tracker
{
    public partial class FormActMain
    {
        private ImportOperation? activeImport;

        private void EnsureImportAdmission()
        {
            // A pumped owner callback remains a foreign producer even on the import's thread.
            if (activeImport is { Exclusive: true } operation &&
                (Environment.CurrentManagedThreadId != operation.OwnerThread || IsDispatchingCallback?.Invoke() == true))
                throw new InvalidOperationException("Combat state is owned by an active log import");
        }

        private void RecordImportFailure(Exception error)
        {
            lock (CombatGate) { if (activeImport is { } operation) operation.Failure ??= error; }
        }

        internal ImportResult ImportLog(string absolutePath)
        {
            ArgumentException.ThrowIfNullOrEmpty(absolutePath);
            if (!Path.IsPathFullyQualified(absolutePath)) throw new ArgumentException("An absolute import path is required", nameof(absolutePath));
            if (Environment.CurrentManagedThreadId != ownerThread)
                throw new InvalidOperationException("Log import must run on the host owner thread");
            if (IsDispatchingCallback?.Invoke() == true)
                throw new InvalidOperationException("Cannot import a log from a dispatched owner-thread callback");
            EnsureCanWaitForLogReader();
            var operation = new ImportOperation(this);
            lock (CombatGate)
            {
                EnsureCombatOpen();
                if (inCombat) throw new InvalidOperationException("End live combat before importing a log file");
                activeImport = operation;
            }
            try { return operation.Run(absolutePath); }
            finally { lock (CombatGate) activeImport = null; }
        }

        // Own temporary parser state for exactly one import, including restoration and partial failure.
        private sealed class ImportOperation(FormActMain root)
        {
            internal int OwnerThread { get; } = Environment.CurrentManagedThreadId;
            internal bool Exclusive;
            internal Exception? Failure;

            internal ImportResult Run(string path)
            {
                ImportResult? result = null;
                // Drain live delivery before capturing the parser context, e.g. its latest zone and timestamp.
                root.RunWithLogReadingSuspended(() =>
                {
                    lock (root.CombatGate) Exclusive = true;
                    try
                    {
                        root.WaitForCombatTransitions();
                        root.WaitForCombatActions();
                        lock (root.CombatGate)
                        {
                            if (root.inCombat) throw new InvalidOperationException("End live combat before importing a log file");
                            Failure = null;
                        }
                        using var reader = new ImportLogReader(path);
                        result = ReadImport(reader, path);
                    }
                    finally { lock (root.CombatGate) Exclusive = false; }
                }, importOperation: true);
                return result!;
            }

            private ImportResult ReadImport(ImportLogReader reader, string path)
            {
                var snapshotBefore = root.GetCombatSnapshotRevision();
                ZoneData liveZone, importZone;
                string livePath, liveCurrentZone;
                string? liveCharacter;
                DateTime liveTime, liveHostile;
                (string Name, DateTime Start) liveContext;
                bool liveReadLock, liveImport;
                int firstEncounter;
                lock (root.CombatGate)
                {
                    root.EnsureCombatStateInitialized();
                    importZone = root.combatZones[0];
                    liveZone = root.combatZone;
                    livePath = root.logFilePath;
                    liveCurrentZone = root.currentZone;
                    liveCharacter = ActGlobals.charName;
                    liveTime = root.lastKnownTime;
                    liveHostile = root.lastHostileTime;
                    liveContext = root.zoneContext;
                    liveReadLock = root.ReadThreadLock;
                    liveImport = root.importingLogs;
                    firstEncounter = importZone.Items.Count;
                    root.combatZone = importZone;
                    root.logFilePath = path;
                    root.currentZone = "Import Zone";
                    root.lastKnownTime = DateTime.MinValue;
                    root.importingLogs = true;
                    root.ReadThreadLock = true;
                }
                long lines = 0, dispatched = 0;
                var failures = new List<Exception>();
                ImportResult? result = null;
                try
                {
                    root.NotifyImportFileChanged(true, path);
                    while (reader.TryRead(out var line))
                    {
                        lines++;
                        if (line.Length == 0) continue;
                        var detectedTime = root.GetDateTimeFromLog(line);
                        int timeSorter;
                        lock (root.CombatGate)
                        {
                            // Original raw import retains host time on MinValue, but passes the local MinValue to hooks.
                            if (detectedTime != DateTime.MinValue) root.lastKnownTime = detectedTime;
                            root.GlobalTimeSorter++;
                            timeSorter = root.GlobalTimeSorter;
                            root.UpdateRawZone(line);
                        }
                        root.CheckIdleEndCombat();
                        var info = new LogLineEventArgs(line, 0, detectedTime, root.CurrentZone, root.InCombat);
                        dispatched++;
                        root.PublishOriginalLogRecord(true, path, line, detectedTime, timeSorter);
                        root.InvokeLogHandlers(root.BeforeLogLineRead, true, info);
                        root.InvokeLogHandlers(root.OnLogLineRead, true, info);
                        // Cooperative hosts must service an action subscriber's synchronous owner-thread invocation during long files.
                        if (dispatched % 64 == 0) root.PumpOwnerCallbacks();
                    }
                }
                catch (Exception error) { failures.Add(error); }
                finally
                {
                    // Even a failed source retains its partial history, but pending actions must settle before restoring live state.
                    try { root.EndCombat(false); root.WaitForCombatActions(); }
                    catch (Exception error) { failures.Add(error); }
                    try
                    {
                        lock (root.CombatGate)
                        {
                            var added = importZone.Items.Skip(firstEncounter).ToArray();
                            result = new(lines, dispatched, added.Length, added.Sum(encounter => encounter.NumCombatants),
                                added.Sum(encounter => encounter.Damage), added.Sum(encounter => encounter.Healed));
                        }
                    }
                    catch (Exception error) { failures.Add(error); }
                    finally
                    {
                        lock (root.CombatGate)
                        {
                            root.combatZone = liveZone;
                            root.logFilePath = livePath;
                            root.currentZone = liveCurrentZone;
                            ActGlobals.charName = liveCharacter;
                            root.lastKnownTime = liveTime;
                            root.lastHostileTime = liveHostile;
                            root.zoneContext = liveContext;
                            root.inCombat = false;
                            root.importingLogs = liveImport;
                            root.ReadThreadLock = liveReadLock;
                        }
                        root.NotifyImportFileChanged(false, livePath);
                    }
                }
                lock (root.CombatGate)
                    if (Failure is { } callbackError)
                        failures.Add(new InvalidDataException("Log import encountered a subscriber or aggregation failure; partial history was retained", callbackError));
                // Finalize status after restoration notifications, which can themselves fail or publish a newer live result.
                root.CompleteImportSnapshotState(snapshotBefore, failures.Count == 0);
                if (failures.Count == 1) ExceptionDispatchInfo.Capture(failures[0]).Throw();
                if (failures.Count > 1) throw new AggregateException("Log import failed; partial history was retained", failures);
                return result!;
            }
        }

        private void NotifyImportFileChanged(bool importing, string path)
        {
            foreach (var callback in LogFileChanged?.GetInvocationList() ?? [])
                try { ((LogFileChangedDelegate)callback)(importing, path); }
                catch (Exception error) { RecordImportFailure(error); WriteExceptionLog(error, "LogFileChanged subscriber"); }
        }
    }
}
