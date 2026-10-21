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
    internal (TabPage Tab, Label Label)? Controls;
}

// EntryPoint serializes calls; mark states before callbacks so failures are never implicitly retried.
internal sealed class PluginSession
{
    private readonly Dictionary<long, PluginRecord> plugins = new();
    private readonly List<long> initialized = new();
    private long nextId;
    private string? directory;
    private HostEnvironment? environment;
    private OwnerContext? context;
    private readonly LogBuffer logs = new();
    internal Tuple<long, string[]> DiagnosticsSnapshot => environment?.DiagnosticsSnapshot ?? Tuple.Create(0L, Array.Empty<string>());
    internal T OnOwner<T>(Func<T> action) => context is null ? action() : context.Run(action);
    internal int Pump() => context?.Pump() ?? 0;
    internal void EndCombat() => OnOwner(() => { environment?.EndCombat(); return 0; });
    internal void EnsureCanStop() => environment?.EnsureCanStop();
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
            context is null ? null : () => context.IsDispatching);
        host.OriginalLogRecordRead += logs.Capture;
        return environment = host;
    }
    private void Cleanup(PluginRecord plugin)
    {
        if (plugin.Controls is not { } controls) return;
        plugin.Controls = null;
        environment?.Unregister(plugin.Plugin);
        var errors = new List<Exception>();
        foreach (var control in new Control[] { controls.Tab, controls.Label })
            try { control.Dispose(); } catch (Exception error) { errors.Add(error); }
        if (errors.Count != 0) throw new AggregateException("Host control disposal failed", errors);
    }
    internal void Configure(string path)
    {
        if (directory is not null) throw new InvalidOperationException("Host environment is already configured");
        if (!Path.IsPathFullyQualified(path) || !Directory.Exists(path))
            throw new ArgumentException("Host data directory must be an existing absolute directory", nameof(path));
        // ACT plugins save settings beneath AppDataFolder/Config, e.g. FFXIV_ACT_Plugin.config.xml.
        Directory.CreateDirectory(Path.Combine(path, "Config"));
        directory = path;
        // Both platforms dispatch on the caller's thread; Windows also pumps its native messages.
        context = new(error => ActGlobals.oFormActMain?.WriteExceptionLog(error, "Posted owner callback"));
    }
    internal long Add(string path, object instance)
    {
        plugins.Add(++nextId, new(path, (IActPluginV1)instance));
        return nextId;
    }
    internal object Construct(string path, string? typeName) => OnOwner(() =>
    {
        // Plugins read ACT globals in constructors, e.g. the official sample's AppDataFolder settings path.
        _ = Host();
        return PluginLoader.Create(path, typeName);
    });
    internal string Initialize(long id) => OnOwner(() => InitializeCore(id));
    private string InitializeCore(long id)
    {
        var plugin = Find(id);
        if (plugin.State != PluginState.Constructed)
            throw new InvalidOperationException("Plugin initialization can only be attempted once per instance");
        plugin.State = PluginState.Initializing;
        try
        {
            var host = Host();
            var data = new ActPluginData { pluginObj = plugin.Plugin, pluginFile = new(plugin.Path), tpPluginSpace = new() };
            try { data.lblPluginStatus = new(); } catch { data.tpPluginSpace.Dispose(); throw; }
            // Capture owned controls separately: plugin code can mutate the public metadata fields.
            var tab = data.tpPluginSpace;
            var label = data.lblPluginStatus;
            plugin.Controls = (tab, label);
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
            var (after, message) = host.Errors;
            if (before != after) throw new InvalidOperationException("InitPlugin reported an exception: " + message);
            plugin.State = PluginState.Initialized;
            initialized.Add(id);
            return label.Text;
        }
        catch (Exception error)
        {
            plugin.State = PluginState.Failed;
            Exception failure = error;
            try { Cleanup(plugin); } catch (Exception cleanupError) { failure = new AggregateException(error, cleanupError); }
            throw new InvalidOperationException($"InitPlugin failed for {plugin.Path}: {failure.GetBaseException().Message}", failure);
        }
    }
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
        var errors = new List<Exception>();
        try { plugin.Plugin.DeInitPlugin(); } catch (Exception error) { errors.Add(error); }
        try { Cleanup(plugin); } catch (Exception error) { errors.Add(error); }
        if (errors.Count != 0) throw new AggregateException("DeInitPlugin failed for " + plugin.Path, errors);
    }
    internal void Stop()
    {
        // Reject a reader's own dispatched callback before forgetting any session-owned resources.
        EnsureCanStop();
        var errors = new List<Exception>();
        // Drain file callbacks while plugin state is still valid, before invoking any DeInitPlugin.
        try { environment?.StopProcessing(); } catch (Exception error) { errors.Add(error); }
        for (var index = initialized.Count - 1; index >= 0; index--)
            if (Find(initialized[index]).State == PluginState.Initialized)
                try { Deinitialize(initialized[index]); } catch (Exception error) { errors.Add(error); }
        plugins.Clear();
        initialized.Clear();
        directory = null;
        var previous = environment;
        environment = null;
        try { previous?.Dispose(); } catch (Exception error) { errors.Add(error); }
        logs.Clear();
        try { context?.Dispose(); } catch (Exception error) { errors.Add(error); }
        context = null;
        if (errors.Count != 0) throw new AggregateException("Plugin shutdown failed; the session was closed", errors);
    }
}
