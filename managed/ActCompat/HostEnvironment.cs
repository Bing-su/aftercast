using System.Windows.Forms;

namespace Advanced_Combat_Tracker
{
    // Fields, rather than properties, satisfy the original plugin's MemberRefs directly.
    public class ActPluginData
    {
        public FileInfo pluginFile = null!;
        public IActPluginV1 pluginObj = null!;
        public TabPage tpPluginSpace = null!;
        public Label lblPluginStatus = null!;
        public Label lblPluginTitle = null!;
        public CheckBox cbEnabled = null!;
    }

    public partial class FormActMain(DirectoryInfo appDataFolder) : Form
    {
        private readonly Dictionary<object, ActPluginData> plugins = new(ReferenceEqualityComparer.Instance);
        private Control? pluginTabs;
        private readonly object errorGate = new();
        private readonly int ownerThread = Environment.CurrentManagedThreadId;
        private long errorCount, ownerErrorCount;
        private string lastError = "", lastOwnerError = "";
        private readonly Queue<string> recentDiagnostics = new();
        public DirectoryInfo AppDataFolder => appDataFolder;
        public List<ActPluginData> ActPlugins { get; set; } = new();
        public bool IsActClosing { get; private set; }
        public bool InitActDone { get; private set; }
        public Version GetVersion() => typeof(FormActMain).Assembly.GetName().Version!;
        internal void SetInitializationComplete(bool complete) => InitActDone = complete && !IsActClosing;
        internal void BeginClosing() { IsActClosing = true; InitActDone = false; ConfigureSpeech(false); ConfigureSound(false); }
        public ActPluginData? PluginGetSelfData(IActPluginV1? plugin) =>
            plugin is not null && plugins.TryGetValue(plugin, out var data) ? data : null;
        public void WriteExceptionLog(Exception? error, string context) => RecordDiagnostic(error, context, failOwner: true);
        internal void WriteHostWarning(string message, string context) =>
            RecordDiagnostic(new InvalidOperationException(message), context, failOwner: false);
        private void RecordDiagnostic(Exception? error, string context, bool failOwner)
        {
            string summary, detail;
            // Plugin-defined exception formatting must not break the host's diagnostic/error path.
            try { summary = error?.Message ?? "Unknown exception"; }
            catch { summary = "Exception message formatting failed"; }
            try { detail = error?.ToString() ?? "Unknown exception"; }
            catch { detail = summary; }
            // Capture caught initializer failures; attribute only logs from the owning thread.
            lock (errorGate)
            {
                errorCount++;
                var message = $"{context}: {summary}";
                lastError = message.Length > 2048 ? message[..2048] : message;
                // Keep bounded stack evidence for worker failures that would otherwise overwrite each other.
                var evidence = $"{context}: {detail}";
                recentDiagnostics.Enqueue(evidence.Length > 8192 ? evidence[..8192] : evidence);
                if (recentDiagnostics.Count > 32) recentDiagnostics.Dequeue();
                if (failOwner && Environment.CurrentManagedThreadId == ownerThread)
                {
                    ownerErrorCount++;
                    lastOwnerError = lastError;
                }
            }
        }
        public void RestartACT(bool update, string reason) =>
            throw new PlatformNotSupportedException("RestartACT is not supported by an embedded host");
        internal void Register(ActPluginData data)
        {
            plugins.Add(data.pluginObj, data);
            ActPlugins.Add(data);
            if (OperatingSystem.IsWindows()) AttachNativePluginTab(data.tpPluginSpace);
        }
        // Native add-ons add/remove their own pages through TabPage.Parent, e.g. cactbot hides its settings page.
        internal void InitializeNativePluginTabs() { pluginTabs = new TabControl(); Controls.Add(pluginTabs); }
        private void AttachNativePluginTab(TabPage tab) => ((TabControl)pluginTabs!).TabPages.Add(tab);
        internal void Unregister(IActPluginV1 plugin)
        {
            // Retain ownership separately because a plugin can replace or edit the public ACT list.
            if (plugins.Remove(plugin, out var data)) ActPlugins.Remove(data);
        }
        internal Tuple<long, string> Errors { get { lock (errorGate) return Tuple.Create(errorCount, lastError); } }
        internal Tuple<long, string> OwnerErrors { get { lock (errorGate) return Tuple.Create(ownerErrorCount, lastOwnerError); } }
        internal string[] RecentDiagnostics { get { lock (errorGate) return recentDiagnostics.ToArray(); } }
        internal Tuple<long, string[]> DiagnosticsSnapshot
        {
            get { lock (errorGate) return Tuple.Create(errorCount, recentDiagnostics.ToArray()); }
        }
    }

    public static partial class ActGlobals
    {
        public static FormActMain? oFormActMain;
        public static FormSpellTimers? oFormSpellTimers;
        public static FormEncounterLogs? oFormEncounterLogs;
        public static FormImportProgress? oFormImportProgress;
        public static string? charName = "YOU";
    }
}

namespace Aftercast.ActCompat
{
    using Advanced_Combat_Tracker;

    // Keep lifecycle operations on the creating thread; background plugin threads own only their action submissions.
    public class HostEnvironment : IDisposable
    {
        private readonly FormActMain main;
        private readonly FormSpellTimers timers;
        private readonly FormEncounterLogs encounterLogs;
        private bool disposed;
        public HostEnvironment(string directory, Action? pumpCallbacks = null, Func<bool>? isDispatchingCallback = null)
        {
            main = new(new DirectoryInfo(directory));
            main.InitializeAudio();
            main.PumpCallbacks = pumpCallbacks;
            main.IsDispatchingCallback = isDispatchingCallback;
            if (ActGlobals.oFormActMain is not null)
            {
                main.Dispose();
                throw new InvalidOperationException("ACT globals are already owned by another environment");
            }
            try
            {
                main.LoadCustomTriggers(Path.Combine(directory, "CustomTriggers.xml"));
                main.LoadDataCorrections(Path.Combine(directory, "DataCorrections.xml"));
                main.LoadHistoryIndex();
                main.Controls.Add(main.OptionsTreeView);
                main.OptionsTreeView.Nodes.Add("Data Correction");
                if (OperatingSystem.IsWindows()) main.InitializeNativePluginTabs();
                timers = new();
                timers.LoadSpellTimers(Path.Combine(directory,"SpellTimers.xml"));
                encounterLogs = new(false);
            }
            catch { timers?.Dispose(); main.Dispose(); throw; }
            ActGlobals.oFormActMain = main;
            ActGlobals.oFormSpellTimers = timers;
            ActGlobals.oFormEncounterLogs = encounterLogs;
            // The original parser substitutes this ACT alias for the current player's actor name.
            ActGlobals.charName = "YOU";
            RegistrationLifetime.Reset();
            RegistrationLifetime.InitializeHostCategoryNames();
            DefaultColumns.Initialize();
            main.CurrentZone = "Unknown Zone";
            main.InitializeCombatState(DateTime.Now);
        }
        public void Register(ActPluginData data) => main.Register(data);
        public void BeginClosing() => main.BeginClosing();
        public void ConfigureSpeech(bool enabled) => main.ConfigureSpeech(enabled);
        public byte[] DrainSpeech() => main.DrainSpeech();
        public void ConfigureSound(bool enabled) => main.ConfigureSound(enabled);
        public (byte[] Bytes, int Volume) DrainSound() => main.DrainSound();
        public int TtsVolume => main.TtsVolume;
        public void ConfigureAudio(int soundVolume, int ttsVolume, string[] timerSounds)
        {
            if (soundVolume is < 0 or > 100 || ttsVolume is < 0 or > 100 || timerSounds.Length != 3)
                throw new ArgumentException("Invalid audio settings");
            main.SoundVolume = soundVolume; main.TtsVolume = ttsVolume;
            (main.TimerStartSound, main.TimerWarningSound, main.TimerExpireSound) = (timerSounds[0], timerSounds[1], timerSounds[2]);
        }
        public void SetInitializationComplete(bool complete) => main.SetInitializationComplete(complete);
        public void ReportLogWriterWarning(string message) => main.WriteHostWarning(message, "Vendor disk writer");
        public void Unregister(IActPluginV1 plugin) => main.Unregister(plugin);
        public event Action<OriginalLogRecord> OriginalLogRecordRead
        {
            add => main.OriginalLogRecordRead += value;
            remove => main.OriginalLogRecordRead -= value;
        }
        public Tuple<long, string> Errors => main.OwnerErrors;
        public Tuple<long, string> Diagnostics => main.Errors;
        public string[] RecentDiagnostics => main.RecentDiagnostics;
        public Tuple<long, string[]> DiagnosticsSnapshot => main.DiagnosticsSnapshot;
        public ImportResult ImportLog(string path, ImportRange? range = null, ImportTextOptions? text = null, IReadOnlyList<ImportCompanion>? companions = null) => main.ImportLog(path, range, text, companions);
        public void BeginImport(string path, ImportRange? range = null, ImportTextOptions? text = null, IReadOnlyList<ImportCompanion>? companions = null) => main.BeginImport(path, range, text, companions);
        public void OpenLogWithCompanions(IReadOnlyList<ImportCompanion> companions, bool getCurrentZone = false, TimeSpan? maxClockWait = null, ImportTextOptions? mainText = null) => main.OpenLogWithCompanions(getCurrentZone, companions, maxClockWait, mainText);
        public long LiveLogGeneration => main.LiveLogGeneration;
        public long OpenLiveLog(string path, IReadOnlyList<ImportCompanion> companions, bool getCurrentZone, TimeSpan maxClockWait, ImportTextOptions? text)
        {
            main.EnsureCanWaitForLogReader();
            main.LogFilePath = path;
            main.OpenLogWithCompanions(getCurrentZone, companions, maxClockWait, text);
            return main.LiveLogGeneration;
        }
        public void CloseLiveLog(long generation) => main.CloseLiveLog(generation);
        public LiveLogSourceStatus[] ReadLiveLogSources() => main.ReadLiveLogSources();
        // Proposals only: no reader is opened and admission must resolve/validate again.
        public static CompanionCandidate[] ReadCompanionCandidates(string path) => CompanionLogCatalog.Discover(path)
            .Select(source => new CompanionCandidate(source.Name, source.Path)).ToArray();
        public ImportProgress ImportProgress => main.CurrentImportProgress;
        public ImportResult? AdvanceImport(bool cancel) => main.AdvanceImport(cancel);
        public TablePageData? ReadTable(TableQuery query) => main.ReadTable(query);
        public TableColumnCatalog ReadTableColumns(TableLevel level) => main.ReadTableColumns(level);
        public HistorySourceSnapshot ReadHistorySources(HistorySelection selection, HistorySourceQuery query, HistorySourceSnapshot? previous = null) =>
            main.ReadHistorySourcesAsync(selection, query, previous, false).GetAwaiter().GetResult();
        public Task<HistorySourceSnapshot> ReadHistorySourcesAsync(HistorySelection selection, HistorySourceQuery query, HistorySourceSnapshot? previous = null, CancellationToken cancellation = default) =>
            main.ReadHistorySourcesAsync(selection, query, previous, true, cancellation);
        public HistoryPage ReadHistory(HistoryQuery query) => main.ReadHistoryAsync(query,false).GetAwaiter().GetResult();
        public Task<HistoryPage> ReadHistoryAsync(HistoryQuery query) => main.ReadHistoryAsync(query);
        public int RestoreSelectedHistory(HistorySelection selection) => main.RestoreSelectedHistoryAsync(selection,false).GetAwaiter().GetResult();
        public Task<int> RestoreSelectedHistoryAsync(HistorySelection selection) => main.RestoreSelectedHistoryAsync(selection);
        public int ArchiveOperation(int command, string text, ulong id, bool compressed) => main.ArchiveOperation(command, text, id, compressed);
        public Task<int> ArchiveOperationAsync(int command, string text, ulong id, bool compressed) => main.ArchiveOperationAsync(command, text, id, compressed);
        public CombatSnapshotData? ReadCombatSnapshot(ulong afterRevision) => main.ReadCombatSnapshot(afterRevision);
        public CombatPage<EncounterSummaryData> ListEncounters(int offset, int limit) => main.ListEncounters(offset, limit);
        public EncounterDetailData? ReadEncounter(ulong id, bool live) => main.ReadEncounter(id, live);
        public CombatPage<AttackSummaryData>? ReadEncounterAttacks(ulong id, int offset, int limit) => main.ReadEncounterAttacks(id, offset, limit);
        public CombatPage<EncounterActionData>? ReadEncounterActions(ulong id, int offset, int limit) => main.ReadEncounterActions(id, offset, limit);
        public EncounterLogPageData? ReadEncounterLogs(ulong id, int offset, int limit) => main.ReadEncounterLogs(id, offset, limit);
        public void ConfigureRecording(bool enabled, bool original)
        {
            main.RecordEncounterLogs = enabled;
            main.OriginalLoglineViewLogs = original;
        }
        public bool HasPluginSettings(TabPage tab) => main.HasPluginSettings(tab);
        public void ShowPluginSettings(TabPage tab) => main.ShowPluginSettings(tab);
        public void EnsureCanStop() => main.EnsureCanWaitForLogReader();
        // Quiesce file callbacks before the boundary so an admitted line cannot race encounter finalization.
        public NotificationPage ReadNotifications(ulong after, int limit) => main.ReadNotifications(after, limit);
        public NotificationItem ShowNotification(ulong id) => main.ShowNotification(id);
        public NotificationStatus ReadNotificationStatus() => main.ReadNotificationStatus();
        public NotificationChange MarkNotificationsRead(ulong through) => main.MarkNotificationsRead(through);
        public NotificationChange ClearReadNotifications(ulong through) => main.ClearReadNotifications(through);
        public object TimerOperation(int command,ulong revision,string? previousKey,TimerDefinition? definition,
            string? key,int offset,int limit)
        {
            var path = Path.Combine(main.AppDataFolder.FullName,"SpellTimers.xml");
            return command switch
            {
                1 => timers.ReadTimerDefinitions(offset,limit),
                2 => SaveTimer(path,revision,previousKey,definition ?? throw new ArgumentException("Missing timer definition")),
                3 => timers.DeleteTimerDefinition(path,revision,key ?? throw new ArgumentException("Missing timer key")),
                4 => timers.ReloadTimerDefinitions(path,revision),
                _ => throw new ArgumentException("Unknown timer operation")
            };
        }
        private SavedTimer SaveTimer(string path,ulong revision,string? previousKey,TimerDefinition definition)
        {
            var next = timers.SaveTimerDefinition(path,revision,previousKey,definition);
            // Return the managed identity: ACT key casing follows this worker's culture, not Rust's lowercase rules.
            var key = definition.ToData().Key;
            return new(next,new TimerItem(key,TimerDefinition.FromData(timers.TimerDefs[key])));
        }
        public object TriggerOperation(int command, ulong revision, string? previousKey, TriggerDefinition? definition,
            string? key, int offset, int limit, bool originalLines, bool processImports, ulong after) => command switch
        {
            1 => main.ReadTriggers(offset, limit),
            2 => main.SaveTrigger(revision, previousKey, definition ?? throw new ArgumentException("Missing trigger definition")),
            3 => main.DeleteTrigger(revision, key ?? throw new ArgumentException("Missing trigger key")),
            4 => main.SaveTriggerOptions(revision, originalLines, processImports),
            5 => main.ReloadTriggers(revision),
            6 => main.ReadTriggerResults(after, limit),
            7 => ClearResults(),
            _ => throw new ArgumentException("Unknown trigger operation")
        };
        private int ClearResults() { main.ClearTriggerResults(); return 0; }
        public void EndCombat() => main.RunWithLogReadingSuspended(() => main.EndCombat(true));
        public void StopLogReading() => main.StopLogFollower();
        public void AdvanceTimers() => timers.Advance();
        public void StopProcessing()
        {
            EnsureCanStop();
            main.BeginCombatShutdown();
            var errors = new List<Exception>();
            // Drain both producers while their plugin callbacks and ACT globals are still valid.
            try { main.StopLogFollower(); } catch (Exception error) { errors.Add(error); }
            try { main.StopCombatProcessing(); } catch (Exception error) { errors.Add(error); }
            if (errors.Count != 0) throw new AggregateException("Host processing shutdown failed", errors);
        }
        public void RunWithLogReadingSuspended(Action action)
        {
            EnsureCanStop();
            main.RunWithLogReadingSuspended(() => main.RunWithCombatActionsSuspended(action));
        }
        public void Dispose()
        {
            // A previous environment may be disposed again after another host has acquired the globals.
            if (disposed) return;
            EnsureCanStop();
            disposed = true;
            main.BeginClosing();
            var errors = new List<Exception>();
            try
            {
                try { StopProcessing(); } catch (Exception error) { errors.Add(error); }
                try { main.SaveCustomTriggers(Path.Combine(main.AppDataFolder.FullName, "CustomTriggers.xml")); } catch (Exception error) { errors.Add(error); }
                try { timers.SaveSpellTimers(Path.Combine(main.AppDataFolder.FullName,"SpellTimers.xml")); } catch (Exception error) { errors.Add(error); }
                try { main.Dispose(); } catch (Exception error) { errors.Add(error); }
                try { timers.Dispose(); } catch (Exception error) { errors.Add(error); }
                try { encounterLogs.Dispose(); } catch (Exception error) { errors.Add(error); }
            }
            finally
            {
                RegistrationLifetime.Reset();
                ActGlobals.oFormActMain = null;
                ActGlobals.oFormSpellTimers = null;
                ActGlobals.oFormEncounterLogs = null;
                ActGlobals.charName = null;
            }
            if (errors.Count != 0) throw new AggregateException("Host environment disposal failed", errors);
        }
    }
}
