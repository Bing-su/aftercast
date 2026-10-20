using System.Runtime.ExceptionServices;
using Bxivact.ActCompat;

namespace Bxivact.ActCompat
{
    // RecordsDispatched measures raw hook dispatch, not whether a plugin accepted a combat record.
    public sealed record ImportResult(long LinesRead, long RecordsDispatched, int Encounters,
        int Combatants, long Damage, long Healed);
}

namespace Advanced_Combat_Tracker
{
    public partial class FormActMain
    {
        private bool importRunning;
        private bool importExclusive;
        private int importOwner;
        private Exception? importFailure;

        private void EnsureImportAdmission()
        {
            // Only the synchronous parser owns import state. A pumped owner callback is still a foreign producer.
            if (importExclusive && (Environment.CurrentManagedThreadId != importOwner || IsDispatchingCallback?.Invoke() == true))
                throw new InvalidOperationException("Combat state is owned by an active log import");
        }

        private void RecordImportFailure(Exception error)
        {
            lock (CombatGate) { if (importRunning) importFailure ??= error; }
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
            lock (CombatGate)
            {
                EnsureCombatOpen();
                if (inCombat) throw new InvalidOperationException("End live combat before importing a log file");
                importRunning = true;
                importOwner = Environment.CurrentManagedThreadId;
                importFailure = null;
            }
            try
            {
                ImportResult? result = null;
                // Suspend and join in-flight live deliveries before changing any parser-visible state or firing hooks.
                RunWithLogReadingSuspended(() =>
                {
                    // Let an already-delivering live reader finish before closing combat admission to foreign producers.
                    lock (CombatGate) importExclusive = true;
                    try
                    {
                        WaitForCombatTransitions();
                        WaitForCombatActions();
                        lock (CombatGate)
                        {
                            if (inCombat) throw new InvalidOperationException("End live combat before importing a log file");
                            importFailure = null;
                        }
                        using var reader = new ImportLogReader(absolutePath);
                        result = ReadImport(reader, absolutePath);
                    }
                    finally { lock (CombatGate) importExclusive = false; }
                }, importOperation: true);
                return result!;
            }
            finally
            {
                lock (CombatGate) { importRunning = false; importExclusive = false; importOwner = 0; importFailure = null; }
            }
        }

        private ImportResult ReadImport(ImportLogReader reader, string path)
        {
            var snapshotBefore = GetCombatSnapshotRevision();
            ZoneData liveZone, importZone;
            string livePath, liveCurrentZone;
            string? liveCharacter;
            DateTime liveTime, liveHostile;
            (string Name, DateTime Start) liveContext;
            bool liveReadLock, liveImport;
            int firstEncounter;
            lock (CombatGate)
            {
                EnsureCombatStateInitialized();
                importZone = combatZones[0];
                liveZone = combatZone;
                livePath = logFilePath;
                liveCurrentZone = currentZone;
                liveCharacter = ActGlobals.charName;
                liveTime = lastKnownTime;
                liveHostile = lastHostileTime;
                liveContext = zoneContext;
                liveReadLock = ReadThreadLock;
                liveImport = importingLogs;
                firstEncounter = importZone.Items.Count;
                combatZone = importZone;
                logFilePath = path;
                currentZone = "Import Zone";
                lastKnownTime = DateTime.MinValue;
                importingLogs = true;
                ReadThreadLock = true;
            }
            long lines = 0, dispatched = 0;
            var failures = new List<Exception>();
            ImportResult? result = null;
            try
            {
                NotifyImportFileChanged(true, path);
                while (reader.TryRead(out var line))
                {
                    lines++;
                    if (line.Length == 0) continue;
                    var detectedTime = GetDateTimeFromLog(line);
                    int timeSorter;
                    lock (CombatGate)
                    {
                        // Original raw import retains host time on MinValue, but passes the local MinValue to hooks.
                        if (detectedTime != DateTime.MinValue) lastKnownTime = detectedTime;
                        GlobalTimeSorter++;
                        timeSorter = GlobalTimeSorter;
                        UpdateRawZone(line);
                    }
                    CheckIdleEndCombat();
                    var info = new LogLineEventArgs(line, 0, detectedTime, CurrentZone, InCombat);
                    dispatched++;
                    PublishOriginalLogRecord(true, path, line, detectedTime, timeSorter);
                    InvokeLogHandlers(BeforeLogLineRead, true, info);
                    InvokeLogHandlers(OnLogLineRead, true, info);
                    // Cooperative hosts must service an action subscriber's synchronous owner-thread invocation during long files.
                    if (dispatched % 64 == 0) PumpOwnerCallbacks();
                }
            }
            catch (Exception error) { failures.Add(error); }
            finally
            {
                // Even a failed source retains its partial history, but pending actions must settle before restoring live state.
                try { EndCombat(false); WaitForCombatActions(); }
                catch (Exception error) { failures.Add(error); }
                try
                {
                    lock (CombatGate)
                    {
                        var added = importZone.Items.Skip(firstEncounter).ToArray();
                        result = new(lines, dispatched, added.Length, added.Sum(encounter => encounter.NumCombatants),
                            added.Sum(encounter => encounter.Damage), added.Sum(encounter => encounter.Healed));
                    }
                }
                catch (Exception error) { failures.Add(error); }
                finally
                {
                    lock (CombatGate)
                    {
                        combatZone = liveZone;
                        logFilePath = livePath;
                        currentZone = liveCurrentZone;
                        ActGlobals.charName = liveCharacter;
                        lastKnownTime = liveTime;
                        lastHostileTime = liveHostile;
                        zoneContext = liveContext;
                        inCombat = false;
                        importingLogs = liveImport;
                        ReadThreadLock = liveReadLock;
                    }
                    NotifyImportFileChanged(false, livePath);
                }
            }
            lock (CombatGate)
                if (importFailure is { } callbackError)
                    failures.Add(new InvalidDataException("Log import encountered a subscriber or aggregation failure; partial history was retained", callbackError));
            // Finalize status after restoration notifications, which can themselves fail or publish a newer live result.
            CompleteImportSnapshotState(snapshotBefore, failures.Count == 0);
            if (failures.Count == 1) ExceptionDispatchInfo.Capture(failures[0]).Throw();
            if (failures.Count > 1) throw new AggregateException("Log import failed; partial history was retained", failures);
            return result!;
        }

        private void NotifyImportFileChanged(bool importing, string path)
        {
            foreach (var callback in LogFileChanged?.GetInvocationList() ?? [])
                try { ((LogFileChangedDelegate)callback)(importing, path); }
                catch (Exception error) { RecordImportFailure(error); WriteExceptionLog(error, "LogFileChanged subscriber"); }
        }
    }
}
