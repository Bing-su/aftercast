using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.Loader;
using System.Text.Json;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.Bridge;

// Load only this test assembly into the Rust worker after initializing the three original plugins.
public sealed class OverlayProcessProbe : IActPluginV1
{
    public OverlayProcessProbe()
    {
        var bridge = AppDomain.CurrentDomain.GetAssemblies().Single(assembly => assembly.GetName().Name == "Aftercast.Bridge");
        var context = AssemblyLoadContext.GetLoadContext(GetType().Assembly)!;
        // Reuse the embedded bridge's session; copying a second Bridge.dll would create another set of globals.
        Assembly? Resolve(AssemblyLoadContext _, AssemblyName name) => name.Name == "Aftercast.Bridge" ? bridge : null;
        context.Resolving += Resolve;
        try { Run(); }
        finally { context.Resolving -= Resolve; }
    }

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static void Run()
    {
        // Resolve bridge types only after the actual child bridge has been supplied to this test load context.
        var lazy = (Lazy<PluginSession>)typeof(EntryPoint).GetField("plugins", BindingFlags.Static | BindingFlags.NonPublic)!.GetValue(null)!;
        var session = lazy.Value;
        session.Pump();
        var root = ActGlobals.oFormActMain!;
        var state = root.AppDataFolder.FullName;
        File.WriteAllText(Path.Combine(state, "process-probe-start.json"), JsonSerializer.Serialize(new {
            pid = Environment.ProcessId, thread = Environment.CurrentManagedThreadId,
            apartment = Thread.CurrentThread.GetApartmentState().ToString(),
            plugins = root.ActPlugins.Select(plugin => plugin.pluginObj.GetType().FullName).ToArray()
        }));
        // Drive the synchronous probe outside native dispatch and DeInit's log-reading suspension.
        OverlayIntegrationChecks.Run(session, stopSession: false);
        var statePath = Path.Combine(state, "process-probe-result.json");
        File.WriteAllText(statePath, JsonSerializer.Serialize(new { pid = Environment.ProcessId, passed = true }));
        root.Disposed += (_, _) => {
            // Host disposal follows event-source shutdown, so no cactbot source may retain the ACT log hook.
            var handlers = (Delegate?)typeof(FormActMain).GetField("OnLogLineRead", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(root);
            var removed = handlers?.GetInvocationList().Any(handler => handler.Target?.GetType().FullName == "Cactbot.CactbotEventSource") != true;
            if (!removed) throw new Exception("Cactbot log hook survived Rust-owned shutdown");
            File.WriteAllText(Path.Combine(state, "process-teardown.json"), JsonSerializer.Serialize(new {
                pid = Environment.ProcessId, cactbotLogHookRemoved = removed
            }));
        };
    }

    public void InitPlugin(TabPage pluginScreenSpace, Label pluginStatusText) => pluginStatusText.Text = "Child CEF/Edge integration passed";
    public void DeInitPlugin() { }
}
