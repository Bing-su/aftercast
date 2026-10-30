using System.Diagnostics;
using System.Text;
using System.Runtime.ExceptionServices;
using System.Threading.Channels;
using Aftercast.ActCompat;

namespace Aftercast.ActCompat
{
    // Counts describe raw hook dispatch, e.g. a parser can ignore a delivered line.
    public sealed record ImportResult(long LinesRead, long RecordsDispatched, int Encounters,
        int Combatants, long Damage, long Healed, long CompanionHeadsExcluded = 0);
    public sealed record ImportProgress(long LinesRead, long RecordsDispatched, long BytesRead, long TotalBytes, long CompanionHeadsExcluded = 0);
}

namespace Advanced_Combat_Tracker
{
    public partial class FormActMain
    {
        private ImportOperation? activeImport;
        internal void ApplyRuntimeSelectiveParsing(SelectiveParsingModeEnum mode, string[] selected)
        {
            lock (CombatGate)
            {
                if (activeImport is not null) throw new InvalidOperationException("Cannot change selective parsing during an import");
                ConfigureSelectiveParsing(mode, selected);
            }
        }
        public Encoding LogEncoding { get; set; } = new ImportTextOptions().Resolve();

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

        internal void BeginImport(string path, ImportRange? range = null, ImportTextOptions? text = null, IReadOnlyList<ImportCompanion>? companions = null,
            CapturedImportSource[]? captured = null, string? character = null, ImportCharacterOptions? viewpoint = null)
            => BeginImportCore(path, range, text, companions, captured, character, null, viewpoint);

        internal void BeginClipboardImport(string content, ImportCharacterOptions? viewpoint = null)
        {
            _ = ImportTextReader.Validate(content);
            BeginImportCore("", null, null, null, null, null, content, viewpoint);
        }

        private void BeginImportCore(string path, ImportRange? range, ImportTextOptions? text, IReadOnlyList<ImportCompanion>? companions,
            CapturedImportSource[]? captured, string? character, string? snippet, ImportCharacterOptions? viewpoint)
        {
            if (snippet is null) ArgumentException.ThrowIfNullOrEmpty(path);
            if (snippet is null && !Path.IsPathFullyQualified(path)) throw new ArgumentException("An absolute import path is required", nameof(path));
            if (Environment.CurrentManagedThreadId != ownerThread || IsDispatchingCallback?.Invoke() == true)
                throw new InvalidOperationException("Import must start outside a dispatched callback on the owner thread");
            if (captured is null) EnsureSourceCaptureIdle();
            if (Volatile.Read(ref sourceArchiveBusy) != 0) throw new InvalidOperationException("Archive operation is active");
            if (captured is not null && viewpoint is not null) throw new ArgumentException("Captured history uses its saved character");
            // Copy the effective identity at admission, before suspended live callbacks can change globals.
            // Private captured metadata and omitted programmatic options stay exact; explicit
            // point-of-view choices follow ACT's UI-driven SetCharName rules.
            var selectedCharacter = !string.IsNullOrEmpty(character) ? character
                : viewpoint is not null ? viewpoint.Resolve(ActGlobals.charName) : ActGlobals.charName;
            var encoding = snippet is not null ? new ImportTextOptions().Resolve() : captured is { Length: > 0 } ? ImportTextOptions.Validate(captured[0].Encoding) : text?.Resolve() ?? ImportTextOptions.Validate(LogEncoding);
            var companionSnapshot = companions?.ToArray() ?? [];
            if (companionSnapshot.Length > 64) throw new ArgumentException("At most 64 companions can be imported");
            var names = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            foreach (var source in companionSnapshot)
            {
                ArgumentNullException.ThrowIfNull(source);
                CompanionLogCatalog.ValidateName(source.Name);
                if (!names.Add(source.Name)) throw new ArgumentException("Duplicate companion selection: " + source.Name);
                source.Text?.Validate();
            }
            companionSnapshot = companionSnapshot.OrderBy(source => source.Name, StringComparer.OrdinalIgnoreCase).ToArray();
            EnsureCanWaitForLogReader();
            lock (CombatGate)
            {
                EnsureCombatOpen();
                if (inCombat) throw new InvalidOperationException("End live combat before importing a log file");
                activeImport = new ImportOperation(this, path, range, encoding, text?.DetectBom ?? true, companionSnapshot, captured, selectedCharacter, snippet);
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
            if (operation.Advancing) throw new InvalidOperationException("Import advance cannot be reentered");
            operation.Advancing = true;
            try { return operation.Advance(cancel); }
            finally { operation.Advancing = false; if (operation.Completed) lock (CombatGate) activeImport = null; }
        }

        internal bool EnsureCapturedImportShutdownAllowed()
        {
            // Reports reuse the read-only table guard: reject before Host.Dispose marks closing or saves files.
            if (activeTableQuery) throw new InvalidOperationException("Shutdown is not permitted during read-only plugin callbacks");
            if (activeImport is not { HasCapturedSources: true } operation) return false;
            if (Environment.CurrentManagedThreadId != ownerThread || IsDispatchingCallback?.Invoke() == true || operation.Advancing)
                throw new InvalidOperationException("Captured import shutdown requires its owner outside callbacks/advance");
            EnsureCanWaitForCombatActions(); return true;
        }
        internal void CancelCapturedImportForShutdown()
        {
            if (!EnsureCapturedImportShutdownAllowed()) return;
            var errors = new List<Exception>();
            while (activeImport is not null)
            {
                try { AdvanceImport(true); }
                catch (OperationCanceledException) { }
                catch (Exception error) { errors.Add(error); }
                if (activeImport is not null)
                {
                    try { PumpOwnerCallbacks(); } catch (Exception error) { errors.Add(error); }
                    Thread.Sleep(1);
                }
            }
            if (errors.Count != 0) throw new AggregateException("Captured import ended with errors during shutdown", errors);
        }

        internal ImportResult ImportLog(string path, ImportRange? range = null, ImportTextOptions? text = null, IReadOnlyList<ImportCompanion>? companions = null, ImportCharacterOptions? viewpoint = null)
        {
            BeginImport(path, range, text, companions, viewpoint: viewpoint);
            while (true)
            {
                var before = CurrentImportProgress;
                if (AdvanceImport(false) is { } result) return result;
                PumpOwnerCallbacks();
                // The synchronous adapter pumps while waiting, but does not busy-spin on slow input or callbacks.
                if (CurrentImportProgress == before) Thread.Sleep(1);
            }
        }

        private sealed class ImportOperation(FormActMain root, string path, ImportRange? range, Encoding encoding, bool detectBom, ImportCompanion[] companions, CapturedImportSource[]? captured, string? character, string? snippet)
        {
            internal int OwnerThread { get; } = Environment.CurrentManagedThreadId;
            internal bool Exclusive, Advancing;
            internal bool HasCapturedSources => captured is not null;
            internal Exception? Failure;
            internal bool Completed;
            private bool canceled, ending, prepared;
            private LogSuspension? suspension;
            private LiveParserContext? live;
            private ZoneData? importZone;
            private int firstEncounter;
            private ulong snapshotBefore;
            private long lines, dispatched, bytesRead, totalBytes, companionHeadsExcluded;
            private readonly CancellationTokenSource cancellation = new();
            private readonly Channel<string[]> input = Channel.CreateBounded<string[]>(new BoundedChannelOptions(4)
            {
                SingleReader = true,
                SingleWriter = true,
                FullMode = BoundedChannelFullMode.Wait
            });
            private readonly Queue<string> buffered = new();
            private ImportSourceRecord? nextLine;
            private ImportSourceInput? sourceInput;
            private Task? reading;
            private readonly List<Exception> failures = new();
            internal ImportProgress Progress => new(lines, dispatched, sourceInput?.BytesRead ?? Interlocked.Read(ref bytesRead), sourceInput?.TotalBytes ?? Interlocked.Read(ref totalBytes), companionHeadsExcluded);
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
                    // ACT ImportClip lets a leading undated row inherit the pre-import known time.
                    if (snippet is null) root.lastKnownTime = DateTime.MinValue;
                    root.lastKnownStamp = Stopwatch.GetTimestamp();
                    ActGlobals.charName = character;
                    root.importingLogs = true;
                    root.ReadThreadLock = true;
                }
                root.NotifyImportFileChanged(true, path);
                // Only the input task owns the reader; parsing and ACT callbacks stay on the owner thread.
                if (companions.Length != 0)
                {
                    sourceInput = new(path, encoding, detectBom, companions, message => root.WriteHostWarning(message, "Companion import"), captured);
                    reading = sourceInput.Start(cancellation.Token);
                }
                else reading = Task.Run(async () =>
                {
                    var batch = new List<string>();
                    try
                    {
                        using IImportRecordReader reader = snippet is not null ? new ImportTextReader(snippet)
                            : captured is null ? new ImportLogReader(path, encoding, detectBom)
                            : new ImportLogReader(captured[0].Image.OpenReader(), captured[0].Encoding, captured[0].DetectBom);
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
                finally
                {
                    try { if (captured is not null) CapturedImportSource.DisposeAll(captured); }
                    finally { Exclusive = false; suspension?.Dispose(); cancellation.Dispose(); }
                }
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
                                if (sourceInput is { } group)
                                {
                                    if (!group.TryNext(root.GetDateTimeFromLog, out var record)) break;
                                    nextLine = record;
                                }
                                else
                                {
                                    if (buffered.Count == 0 && input.Reader.TryRead(out var batch))
                                        foreach (var record in batch) buffered.Enqueue(record);
                                    if (!buffered.TryDequeue(out var line)) break;
                                    nextLine = new(line, line.Length == 0 ? DateTime.MinValue : root.GetDateTimeFromLog(line), path, "", 0);
                                }
                            }
                            var next = nextLine.Value;
                            bool skipped;
                            if (next.BeyondMain)
                            {
                                lines++;
                                companionHeadsExcluded++;
                                skipped = true;
                            }
                            else skipped = SkipOutsideRange(next.Line, next.Time, next.Source == 0);
                            if (!skipped && !TryDispatch(next)) break;
                            // Only a rejected past-end head closes its source; an extended row must
                            // remain readable even when parsing that row changes the combat state.
                            sourceInput?.Consume(next.Source, next.BeyondMain || skipped && range is not null && next.Line.Length != 0 && next.Time > range.End);
                            nextLine = null;
                        }
                        if (!ending && nextLine is null && (sourceInput?.Completed ?? (buffered.Count == 0 && input.Reader.Completion.IsCompleted)))
                        {
                            if (sourceInput is null) input.Reader.Completion.GetAwaiter().GetResult();
                            ending = true;
                            cancellation.Cancel();
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
                if (sourceInput?.Failure is { } sourceError && !failures.Contains(sourceError)) failures.Add(sourceError);
                if (canceled && sourceInput is null && input.Reader.Completion.IsFaulted)
                {
                    // Explicit cancellation must not hide an already recorded producer error. Normal
                    // range completion still ignores decoder failures in unconsumed bytes after cutoff.
                    try { input.Reader.Completion.GetAwaiter().GetResult(); }
                    catch (OperationCanceledException) when (cancellation.IsCancellationRequested) { }
                    catch (Exception error) { if (!failures.Contains(error)) failures.Add(error); }
                }
                return Finish();
            }

            private bool SkipOutsideRange(string line, DateTime time, bool mainSource)
            {
                if (range is null || line.Length == 0 || time >= range.Start &&
                    (time <= range.End || range.ExtendToCombatEnd && root.InCombat)) return false;
                lines++;
                if (time > range.End)
                {
                    // ACT stops at the first later timestamp, even if subsequent records go backwards.
                    if (sourceInput is null) { ending = true; cancellation.Cancel(); }
                }
                else if (mainSource)
                {
                    // Recover preceding raw zone context without dispatching out-of-range plugin callbacks.
                    lock (root.CombatGate) root.UpdateRawZone(line);
                }
                return true;
            }

            private bool TryDispatch(ImportSourceRecord record)
            {
                var line = record.Line;
                var detectedTime = snippet is not null && record.Time == DateTime.MinValue ? root.lastKnownTime : record.Time;
                if (line.Length == 0) { lines++; return true; }
                int sorter;
                lock (root.CombatGate)
                {
                    var time = detectedTime == DateTime.MinValue ? root.lastKnownTime : detectedTime;
                    // Keep this record pending until the previous encounter can end without a synchronous action drain.
                    if (root.inCombat && time - root.lastHostileTime > TimeSpan.FromSeconds(6) &&
                        (!root.actionsIdle.IsSet || !root.combatTransitionsIdle.IsSet)) return false;
                    // ACT companion input may advance the shared clock but must not rewind it.
                    if (record.Source == 0 || detectedTime > root.lastKnownTime) root.LastKnownTime = detectedTime;
                    sorter = ++root.GlobalTimeSorter;
                    root.UpdateRawZone(line);
                }
                lines++;
                var wasInCombat = root.InCombat;
                root.CheckIdleEndCombat();
                var info = new LogLineEventArgs(line, 0, detectedTime, root.CurrentZone, root.InCombat, record.Name);
                dispatched++;
                root.PublishOriginalLogRecord(true, record.Path, line, detectedTime, sorter, record.Name);
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
                ImportResult result = new(lines, dispatched, 0, 0, 0, 0, companionHeadsExcluded);
                try
                {
                    try { root.EndCombat(false); root.WaitForCombatActions(); }
                    catch (Exception error) { failures.Add(error); }
                    try
                    {
                        lock (root.CombatGate)
                        {
                            // Stamp all retained clipboard encounters even if later aggregate arithmetic fails.
                            if (snippet is not null)
                                for (var index = firstEncounter; index < importZone!.Items.Count; index++) importZone.Items[index].ClipboardImport = true;
                            var encounters = 0; var combatants = 0; long damage = 0, healed = 0;
                            for (var index = firstEncounter; index < importZone!.Items.Count; index++)
                            {
                                var encounter = importZone.Items[index];
                                encounters++;
                                combatants = checked(combatants + encounter.NumCombatants);
                                damage = checked(damage + encounter.Damage);
                                healed = checked(healed + encounter.Healed);
                            }
                            result = new(lines, dispatched, encounters, combatants, damage, healed, companionHeadsExcluded);
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
                    if (captured is not null) foreach (var source in captured)
                        try { source.Image.Dispose(); } catch (Exception error) { failures.Add(error); }
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
