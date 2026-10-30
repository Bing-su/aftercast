using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

namespace Aftercast.Bridge;

internal enum PluginState { Constructed, Initializing, Initialized, Failed, Stopped }
internal sealed class PluginRecord(string path, IActPluginV1 plugin)
{
    internal readonly string Path = path;
    internal readonly IActPluginV1 Plugin = plugin;
    internal PluginState State;
    internal bool WriterCaptureAttempted;
    internal VendorWriterObserver? Writer;
    internal (TabPage Tab, Label Label, Label Title, CheckBox Enabled)? Controls;
}

// EntryPoint serializes calls; mark states before callbacks so failures are never implicitly retried.
internal sealed partial class PluginSession
{
    private readonly Dictionary<long, PluginRecord> plugins = new();
    private readonly List<long> initialized = new();
    private long nextId;
    private readonly WorkerReusePolicy workerReuse = new();
    private string? directory;
    private HostEnvironment? environment;
    private OwnerContext? context;
    private bool recordEncounterLogs = true, originalLoglineViewLogs;
    private bool idleEndEnabled = true, idleTimerEnabled = true, mainTableCommas = true, countZeroDamageAsHit = true;
    private int idleEndSeconds = 6, silenceCutSeconds;
    private SelectiveParsingModeEnum selectiveMode;
    private string[] selectedCombatants = [];
    private bool speechEnabled;
    private bool soundEnabled;
    private int soundVolume = 100, ttsVolume = 100;
    private string[] timerSounds = ["none", "none", "none"];
    private string miscMode = "none", miscText = "";
    private bool colorInvert; private int colorLuminosity, colorSaturation;
    private System.Drawing.Color colorAlly = System.Drawing.Color.ForestGreen, colorPersonal = System.Drawing.Color.FromArgb(200,255,200);
    private readonly LogBuffer logs = new();
    internal Tuple<long, string[]> DiagnosticsSnapshot => environment?.DiagnosticsSnapshot ?? Tuple.Create(0L, Array.Empty<string>());
    internal T OnOwner<T>(Func<T> action)
    {
        T Checked()
        {
            if (environment?.IsFaulted == true)
            {
                workerReuse.MarkUnsafe();
                throw new InvalidOperationException("Host retired after mixed Config publication failure; restart the worker");
            }
            return action();
        }
        return context is null ? Checked() : context.Run(Checked);
    }
    internal byte[] TextReportOperation(TextReportRequest request) => OnOwner(() =>
    {
        var host = Host();
        return HostJson.Encode(request.Options is null ? host.ReadTextReportCatalog() :
            (object?)host.ReadTextReport(request.EncounterId, request.Revision, request.Options)!);
    });
    internal byte[] HierarchyHtmlOperation(int command, TableReportRequest request) => OnOwner(() => HierarchyHtmlEncoding.Encode(Host(),command,request));
    internal byte[] HierarchyXmlOperation(int command, TableReportRequest request) => OnOwner(() => HierarchyXmlEncoding.Encode(Host(),command,request));
    internal byte[] OdbcOperation(int command, TableReportRequest request, int capacity) => OnOwner(() => OdbcEncoding.Encode(Host(), command, request, capacity));
    internal byte[] TableReportOperation(TableReportRequest request, int capacity) => OnOwner(() => TableReportEncoding.Encode(Host(), request, capacity));
    internal byte[] TableOperation(TableRequest request, int capacity) => OnOwner(() => TableEncoding.Encode(Host(), request, capacity));
    internal ArraySegment<byte> QueryCombat(int mode, ulong id, int offset, int limit, string? incomingCombatant) =>
        OnOwner(() => CombatQueryEncoding.Encode(environment, mode, id, offset, limit, incomingCombatant));
    private ulong notifiedCombatRevision;
    internal int Pump()
    {
        // Pump is the caller's startup boundary; pending plugin instances keep deferred integrations paused.
        OnOwner(() => { environment?.SetInitializationComplete(initialized.Count != 0 &&
            plugins.Values.All(plugin => plugin.State is not (PluginState.Constructed or PluginState.Initializing))); return 0; });
        var callbacks = context?.Pump() ?? 0;
        OnOwner(() => { environment?.AdvanceTimers(); environment?.AdvanceIdleTimer(); return 0; });
        var revision = environment?.CombatSnapshotRevision ?? 0;
        if (revision != notifiedCombatRevision)
        {
            notifiedCombatRevision = revision;
            return callbacks | int.MinValue;
        }
        return callbacks;
    }
    internal byte[] TriggerOperation(int command, TriggerRequest request) => OnOwner(() =>
    {
        var host = Host();
        object result;
        if (command == 14)
        {
            host.ConfigureSelectiveParsing((SelectiveParsingModeEnum)request.SelectiveMode, request.Selected ?? throw new ArgumentException("Missing selection"));
            result = true;
        }
        else if (command == 12)
        {
            var preview = host.PreviewMixedConfig(request.Snippet ?? throw new ArgumentException("Missing Config"));
            result = new { triggers = preview.Triggers, timers = preview.Timers };
        }
        else if (command == 13)
        {
            try
            {
                host.ImportMixedConfig(request.Snippet ?? throw new ArgumentException("Missing Config"), request.Revision, request.TimerRevision);
                result = new { faulted = false, error = "" };
            }
            catch (Exception error) when (host.IsFaulted)
            {
                workerReuse.MarkUnsafe();
                var message = error.Message;
                if (string.IsNullOrEmpty(message)) message = "Mixed Config publication failed";
                if (message.Length > 512) message = message[..512];
                result = new { faulted = true, error = message };
            }
        }
        else if (command == 10) result = host.PreviewTriggerConfig(request.Snippet ?? throw new ArgumentException("Missing Config"));
        else if (command == 11) { object? batch = null; host.RunWithLogReadingSuspended(() => batch = host.SaveTriggerBatch(request.Revision, request.Definitions ?? throw new ArgumentException("Missing definitions"))); result = batch!; }
        else if (command == 9) result = host.PreviewTriggerSnippet(request.Snippet ?? throw new ArgumentException("Missing trigger snippet"));
        else if (command == 8) result = host.BenchmarkTriggers(request.Revision, request.Snapshot, request.Keys ?? throw new ArgumentException("Missing benchmark selection"));
        else if (command is 1 or 6) result = host.TriggerOperation(command, request.Revision, request.PreviousKey,
            request.Definition, request.Key, request.Offset, request.Limit, request.OriginalLines, request.ProcessImports, request.After);
        else
        {
            object? changed = null;
            host.RunWithLogReadingSuspended(() => changed = host.TriggerOperation(command, request.Revision, request.PreviousKey,
                request.Definition, request.Key, request.Offset, request.Limit, request.OriginalLines, request.ProcessImports, request.After));
            result = changed!;
        }
        return HostJson.Encode(result);
    });
    internal byte[] CorrectionOperation(int command, CorrectionRequest request) => OnOwner(() =>
    {
        var host = Host();
        return HostJson.Encode(command switch {
            1 => (object)host.ReadCorrections(),
            2 => host.SaveCorrections(request.Revision, request.Renames ?? throw new ArgumentException("Missing rename rules"), request.Redirects ?? throw new ArgumentException("Missing redirect rules")),
            3 => host.ReadSpeechRules(),
            4 => host.SaveSpeechRules(request.Revision, request.SpeechRules ?? throw new ArgumentException("Missing speech rules")),
            _ => throw new ArgumentException("Unknown correction operation"),
        });
    });
    internal byte[] TimerOperation(int command, TimerRequest request) => OnOwner(() =>
    {
        var host = Host();
        object? result = null;
        void Execute() => result = command switch {
            5 => host.PreviewTimerSnippet(request.Snippet ?? throw new ArgumentException("Missing timer snippet")),
            6 => host.PreviewTimerConfig(request.Snippet ?? throw new ArgumentException("Missing Config")),
            7 => host.SaveTimerBatch(request.Revision, request.Definitions ?? throw new ArgumentException("Missing definitions")),
            _ => host.TimerOperation(command,request.Revision,request.PreviousKey,request.Definition,request.Key,request.Offset,request.Limit),
        };
        if (command is 1 or 5 or 6) Execute(); else host.RunWithLogReadingSuspended(Execute);
        return HostJson.Encode(result!);
    });
    internal byte[] NotificationOperation(int command, ulong id, int limit) => OnOwner(() =>
        HostJson.Encode(command switch
        {
            1 => (object)Host().ReadNotifications(id, limit),
            2 => Host().ShowNotification(id),
            3 => Host().MarkNotificationsRead(id),
            4 => Host().ClearReadNotifications(id),
            5 => Host().ReadNotificationStatus(),
            _ => throw new ArgumentOutOfRangeException(nameof(command)),
        }));
    internal void EndCombat() => OnOwner(() => { environment?.EndCombat(); return 0; });
    internal int ArchiveOperation(int command, string text, ulong id, bool compressed, bool includeLogs = true, bool processTriggers = false) => OnOwner(() =>
    {
        if (command is 1 or 4 && !plugins.Values.Any(plugin => plugin.State == PluginState.Initialized))
            throw new InvalidOperationException("Archive reconstruction requires initialized parser categories");
        return Host().ArchiveOperation(command, text, id, compressed, includeLogs, processTriggers);
    });
    internal void EnsureCanStop()
    {
        if (archive is not null || history is not null) throw new InvalidOperationException("Complete the archive/history operation before stopping the session");
        environment?.EnsureCanStop();
    }
    internal ulong SubscribeLogs(int maxRecords, int maxBytes) => OnOwner(() => logs.Subscribe(maxRecords, maxBytes));
    internal void UnsubscribeLogs(ulong id) => OnOwner(() => { logs.Unsubscribe(id); return 0; });
    internal LogDrain DrainLogs(ulong id, int capacity) => OnOwner(() => logs.Drain(id, capacity));
    internal CombatSnapshotData? ReadCombatSnapshot(ulong afterRevision) => OnOwner(() =>
    {
        if (environment is not null) return environment.ReadCombatSnapshot(afterRevision);
        if (afterRevision != 0) throw new ArgumentOutOfRangeException(nameof(afterRevision));
        return null;
    });
    internal ImportResult ImportLog(string path) => OnOwner(() =>
    {
        if (!plugins.Values.Any(plugin => plugin.State == PluginState.Initialized))
            throw new InvalidOperationException("File import requires an initialized plugin");
        return Host().ImportLog(path);
    });
    internal void BeginImport(string path, ImportRange? range = null, ImportTextOptions? text = null) => OnOwner(() =>
    {
        if (!plugins.Values.Any(plugin => plugin.State == PluginState.Initialized))
            throw new InvalidOperationException("File import requires an initialized plugin");
        Host().BeginImport(path, range, text);
        return 0;
    });
    internal void BeginClipboardImport(string content, ImportCharacterOptions? viewpoint = null) => OnOwner(() =>
    {
        if (!plugins.Values.Any(plugin => plugin.State == PluginState.Initialized))
            throw new InvalidOperationException("Clipboard import requires an initialized plugin");
        Host().BeginClipboardImport(content, viewpoint);
        return 0;
    });
    internal ImportResult? AdvanceImport(bool cancel) => OnOwner(() =>
    {
        if (sourceImportOwner != 0) throw new InvalidOperationException("Source import requires its owned continuation");
        return Host().AdvanceImport(cancel);
    });
    internal ImportProgress ImportProgress => OnOwner(() => Host().ImportProgress);
    private Task<int>? archive;
    internal void BeginArchive(int command, string text, ulong id, bool compressed, bool includeLogs = true, bool processTriggers = false) => OnOwner(() =>
    {
        if (archive is not null || history is not null) throw new InvalidOperationException("Archive/history operation is already active");
        if (command is 1 or 4 && !plugins.Values.Any(plugin => plugin.State == PluginState.Initialized))
            throw new InvalidOperationException("Archive reconstruction requires an initialized plugin");
        // Construct controls under the normal plugin context, never the private continuation context.
        var host = Host();
        archive = context!.RunOperation(() => host.ArchiveOperationAsync(command, text, id, compressed, includeLogs, processTriggers));
        return 0;
    });
    internal int? AdvanceArchive()
    {
        var operation = archive ?? throw new InvalidOperationException("No active archive operation");
        if (!operation.IsCompleted) return null;
        try { return operation.GetAwaiter().GetResult(); }
        finally { archive = null; }
    }
    private Task<byte[]>? history;
    internal void BeginHistory(int command,HistoryRequest request) => OnOwner(() =>
    {
        if (history is not null || archive is not null) throw new InvalidOperationException("Archive/history operation is already active");
        if (command == 2 && !plugins.Values.Any(plugin => plugin.State == PluginState.Initialized))
            throw new InvalidOperationException("History reconstruction requires an initialized plugin");
        var host = Host(); // Construct controls in the normal owner context, before private continuations.
        history = context!.RunOperation(async () => command == 1
            ? HostJson.Encode(await host.ReadHistoryAsync(request.Query!))
            : HostJson.Encode(await host.RestoreSelectedHistoryAsync(request.Selection!)));
        return 0;
    });
    internal byte[]? AdvanceHistory()
    {
        var operation = history ?? throw new InvalidOperationException("No active history operation");
        if (!operation.IsCompleted) return null;
        try { return operation.GetAwaiter().GetResult(); }
        finally { history = null; }
    }
    private PluginRecord Find(long id) => plugins.TryGetValue(id, out var plugin)
        ? plugin : throw new ArgumentException("Unknown plugin instance", nameof(id));
    private HostEnvironment Host()
    {
        if (environment is not null) return environment;
        if (OperatingSystem.IsWindows() && Thread.CurrentThread.GetApartmentState() != ApartmentState.STA)
            throw new InvalidOperationException("Windows plugin initialization requires a caller-owned STA thread and UI message loop");
        var host = new HostEnvironment(directory
            ?? throw new InvalidOperationException("Host data directory has not been configured"),
            context is null ? null : () => context.Pump(),
            context is null ? null : () => context.IsDispatching, selectiveMode, selectedCombatants);
        host.ConfigureRecording(recordEncounterLogs, originalLoglineViewLogs);
        host.ConfigureIdleEnd(idleEndEnabled, idleEndSeconds, idleTimerEnabled);
        host.ConfigureSilenceCut(silenceCutSeconds);
        host.ConfigureMainTableCommas(mainTableCommas);
        host.ConfigureZeroDamageHits(countZeroDamageAsHit);
        host.OriginalLogRecordRead += logs.Capture;
        host.ConfigureSpeech(speechEnabled);
        host.ConfigureSound(soundEnabled);
        host.ConfigureAudio(soundVolume, ttsVolume, timerSounds, miscMode, miscText);
        host.ApplyColorScalars(colorInvert, colorLuminosity / 100m, colorSaturation / 100m);
        host.ApplyColorControls(colorAlly,colorPersonal);
        return environment = host;
    }
    private void RequireReusableWorker() => workerReuse.RequireReusable();
    private void CaptureWriter(PluginRecord plugin)
    {
        if (plugin.WriterCaptureAttempted) return;
        plugin.WriterCaptureAttempted = true;
        try { plugin.Writer = FfxivWriterLifetime.Capture(plugin.Plugin, Host().ReportLogWriterWarning); }
        catch { workerReuse.MarkUnsafe(); throw; }
    }
    private void FinishWriter(PluginRecord plugin)
    {
        if (plugin.Writer is not { } writer) return;
        plugin.Writer = null;
        WriterStopObservation observation;
        try { observation = writer.Finish(() => context?.Pump()); }
        catch { workerReuse.MarkUnsafe(); throw; }
        workerReuse.Observe(observation);
        if (observation.Failed) throw new InvalidOperationException(observation.Describe());
    }
    private void Cleanup(PluginRecord plugin)
    {
        if (plugin.Controls is not { } controls) return;
        plugin.Controls = null;
        environment?.Unregister(plugin.Plugin);
        var errors = new List<Exception>();
        foreach (var control in new Control[] { controls.Tab, controls.Label, controls.Title, controls.Enabled })
            try { control.Dispose(); } catch (Exception error) { errors.Add(error); }
        if (plugin.Plugin is IDisposable resource && plugin.Plugin is not Control { IsDisposed: true })
            try { resource.Dispose(); } catch (Exception error) { errors.Add(error); }
        // Plugin/container/TraceListener disposal may enqueue a final record, so inspect afterward.
        try { FinishWriter(plugin); } catch (Exception error) { errors.Add(error); }
        if (errors.Count != 0) throw new AggregateException("Host control disposal failed", errors);
    }
    internal void Configure(string path, bool recordLogs = true, bool originalLogs = false, bool idleEnabled = true, int idleSeconds = 6, bool timerEnabled = true, int silenceSeconds = 0, bool commas = true, SelectiveParsingModeEnum selection = SelectiveParsingModeEnum.None, string[]? selected = null, bool countZero = true)
    {
        RequireReusableWorker();
        HostEnvironment.ValidateSelectiveParsing(selection, selected ?? []);
        if (silenceSeconds is < 0 or > 60) throw new ArgumentOutOfRangeException(nameof(silenceSeconds));
        if (idleSeconds is < 2 or > 600) throw new ArgumentOutOfRangeException(nameof(idleSeconds));
        if (directory is not null) throw new InvalidOperationException("Host environment is already configured");
        if (!Path.IsPathFullyQualified(path) || !Directory.Exists(path))
            throw new ArgumentException("Host data directory must be an existing absolute directory", nameof(path));
        // ACT plugins save settings beneath AppDataFolder/Config, e.g. FFXIV_ACT_Plugin.config.xml.
        Directory.CreateDirectory(Path.Combine(path, "Config"));
        directory = path;
        recordEncounterLogs = recordLogs;
        originalLoglineViewLogs = originalLogs;
        idleEndEnabled = idleEnabled;
        idleTimerEnabled = timerEnabled;
        silenceCutSeconds = silenceSeconds;
        mainTableCommas = commas;
        countZeroDamageAsHit = countZero;
        selectiveMode = selection;
        selectedCombatants = (selected ?? []).ToArray();
        idleEndSeconds = idleSeconds;
        // Both platforms dispatch on the caller's thread; Windows also pumps its native messages.
        context = new(error => ActGlobals.oFormActMain?.WriteExceptionLog(error, "Posted owner callback"));
    }
    internal long Add(string path, object instance)
    {
        environment?.SetInitializationComplete(false);
        plugins.Add(++nextId, new(path, (IActPluginV1)instance));
        return nextId;
    }
    internal object Construct(string path, string? typeName) => OnOwner(() =>
    {
        RequireReusableWorker();
        // Plugins read ACT globals in constructors, e.g. the official sample's AppDataFolder settings path.
        _ = Host();
        environment!.SetInitializationComplete(false);
        return PluginLoader.Create(path, typeName);
    });
    internal void ConfigureSpeech(bool enabled) => OnOwner(() =>
    {
        speechEnabled = enabled;
        environment?.ConfigureSpeech(enabled);
        return 0;
    });
    internal byte[] DrainSpeech() => OnOwner(() => environment?.DrainSpeech() ?? Array.Empty<byte>());
    internal int TtsVolume => ttsVolume;
    internal void ConfigureSound(bool enabled) => OnOwner(() => { soundEnabled = enabled; environment?.ConfigureSound(enabled); return 0; });
    internal (byte[] Bytes, int Volume) DrainSound() => OnOwner(() => environment?.DrainSound() ?? (Array.Empty<byte>(), 0));
    internal (bool Invert, float Luminosity, float Saturation, System.Drawing.Color Ally, System.Drawing.Color Personal) ReadRuntimeColors() => OnOwner(() =>
        environment?.ReadRuntimeColors() ?? (colorInvert, colorLuminosity / 100f, colorSaturation / 100f, colorAlly, colorPersonal));
    internal void ConfigureColorControls(System.Drawing.Color ally, System.Drawing.Color personal) => OnOwner(() =>
    {
        if ((!ally.IsEmpty && ally.A != 255) || (!personal.IsEmpty && personal.A != 255)) throw new ArgumentException("Colors must be opaque or empty");
        environment?.ApplyColorControls(ally,personal);
        colorAlly=ally; colorPersonal=personal; return 0;
    });
    internal void ConfigureColors(bool invert, int luminosity, int saturation) => OnOwner(() =>
    {
        if (luminosity is < -100 or > 100 || saturation is < -100 or > 100) throw new ArgumentOutOfRangeException(nameof(luminosity));
        // Preserve native sequential event semantics: an exception can leave controls partially changed.
        environment?.ApplyColorScalars(invert, luminosity / 100m, saturation / 100m);
        colorInvert = invert; colorLuminosity = luminosity; colorSaturation = saturation;
        return 0;
    });
    internal void ConfigureAudio(int sound, int tts, string[] timers, string mode, string text) => OnOwner(() =>
    {
        if (sound is < 0 or > 100 || tts is < 0 or > 100 || timers.Length != 3 || mode is not ("none" or "beep" or "wav" or "tts") || text.Any(char.IsControl)) throw new ArgumentException("Invalid audio settings");
        soundVolume = sound; ttsVolume = tts; timerSounds = timers; miscMode = mode; miscText = text;
        environment?.ConfigureAudio(sound, tts, timers, mode, text);
        return 0;
    });
    internal string Initialize(long id) => OnOwner(() => InitializeCore(id));
    private string InitializeCore(long id)
    {
        RequireReusableWorker();
        var plugin = Find(id);
        if (plugin.State != PluginState.Constructed)
            throw new InvalidOperationException("Plugin initialization can only be attempted once per instance");
        plugin.State = PluginState.Initializing;
        try
        {
            var host = Host();
            var data = new ActPluginData { pluginObj = plugin.Plugin, pluginFile = new(plugin.Path), tpPluginSpace = new() };
            try { data.lblPluginStatus = new(); data.lblPluginTitle = new() { Text = Path.GetFileName(plugin.Path) }; data.cbEnabled = new() { Checked = true }; }
            catch { data.lblPluginTitle?.Dispose(); data.lblPluginStatus?.Dispose(); data.tpPluginSpace.Dispose(); throw; }
            // Capture owned controls separately: plugin code can mutate the public metadata fields.
            var tab = data.tpPluginSpace;
            var label = data.lblPluginStatus;
            plugin.Controls = (tab, label, data.lblPluginTitle, data.cbEnabled);
            // ACT supplies usable UI handles before InitPlugin; worker Invoke calls must not create them.
            if (OperatingSystem.IsWindows())
            {
                _ = ActGlobals.oFormActMain!.Handle;
                _ = tab.Handle;
                _ = label.Handle;
            }
            host.Register(data);
            var before = host.Errors.Item1;
            plugin.Plugin.InitPlugin(tab, label);
            CaptureWriter(plugin);
            var (after, message) = host.Errors;
            if (before != after) throw new InvalidOperationException("InitPlugin reported an exception: " + message);
            plugin.State = PluginState.Initialized;
            initialized.Add(id);
            return label.Text;
        }
        catch (Exception error)
        {
            plugin.State = PluginState.Failed;
            var failures = new List<Exception> { error };
            try { CaptureWriter(plugin); } catch (Exception captureError) { failures.Add(captureError); }
            try { Cleanup(plugin); } catch (Exception cleanupError) { failures.Add(cleanupError); }
            Exception failure = failures.Count == 1 ? error : new AggregateException(failures);
            throw new InvalidOperationException($"InitPlugin failed for {plugin.Path}: {failure.GetBaseException().Message}", failure);
        }
    }
    internal (int State, int Settings) Status(long id) => OnOwner(() =>
    {
        var plugin = Find(id);
        var settings = !OperatingSystem.IsWindows() ? 0 :
            plugin.State == PluginState.Initialized && plugin.Controls is { } controls && Host().HasPluginSettings(controls.Tab) ? 2 : 1;
        return ((int)plugin.State, settings);
    });

    internal void ShowSettings(long id) => OnOwner(() =>
    {
        var plugin = Find(id);
        if (plugin.State != PluginState.Initialized || plugin.Controls is not { } controls)
            throw new InvalidOperationException("Only an initialized plugin can open settings");
        Host().ShowPluginSettings(controls.Tab);
        return 0;
    });

    internal void Deinitialize(long id) => OnOwner(() =>
    {
        if (environment is null) DeinitializeCore(id);
        else environment.RunWithLogReadingSuspended(() => DeinitializeCore(id));
        return 0;
    });
    private void DeinitializeCore(long id)
    {
        var plugin = Find(id);
        if (plugin.State != PluginState.Initialized)
            throw new InvalidOperationException("Only an initialized plugin can be deinitialized");
        plugin.State = PluginState.Stopped;
        if (plugin.Controls is { } owned) owned.Enabled.Checked = false;
        var errors = new List<Exception>();
        try { OverlayPluginLifetime.StopEventSources(plugin.Plugin); } catch (Exception error) { errors.Add(error); }
        try { plugin.Plugin.DeInitPlugin(); } catch (Exception error) { errors.Add(error); }
        try { Cleanup(plugin); } catch (Exception error) { errors.Add(error); }
        if (errors.Count != 0) throw new AggregateException("DeInitPlugin failed for " + plugin.Path, errors);
    }
    internal void Stop()
    {
        // Reject a reader's own dispatched callback before forgetting any session-owned resources.
        EnsureCanStop();
        if (environment?.IsFaulted == true) workerReuse.MarkUnsafe();
        if (environment?.IsFaulted != true) OnOwner(() => { environment?.BeginClosing(); return 0; });
        var errors = new List<Exception>();
        // Cancel bounded source I/O before plugin callbacks and retain every release failure.
        try { ReleaseCapture(); } catch (Exception error) { errors.Add(error); }
        try { ReleaseProposal(); } catch (Exception error) { errors.Add(error); }
        // Drain file callbacks while plugin state is still valid, before invoking any DeInitPlugin.
        try { environment?.StopProcessing(); } catch (Exception error) { errors.Add(error); }
        for (var index = initialized.Count - 1; index >= 0; index--)
            if (Find(initialized[index]).State == PluginState.Initialized)
                try { Deinitialize(initialized[index]); } catch (Exception error) { errors.Add(error); }
        plugins.Clear();
        initialized.Clear();
        directory = null;
        sourceImportOwner = liveSourceOwner = 0; liveRootGeneration = 0; liveRequest = null;
        speechEnabled = false;
        soundEnabled = false;
        soundVolume = ttsVolume = 100;
        timerSounds = ["none", "none", "none"]; miscMode = "none"; miscText = "";
        var previous = environment;
        environment = null;
        try { previous?.Dispose(); } catch (Exception error) { errors.Add(error); }
        logs.Clear();
        try { context?.Dispose(); } catch (Exception error) { errors.Add(error); }
        context = null;
        if (errors.Count != 0) throw new AggregateException("Plugin shutdown failed; the session was closed", errors);
    }
}
