using System.Reflection;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

namespace Aftercast.PluginFixture;

// Synthetic worker-only probe: invoke the existing private fault seam, never real settings.
public sealed class MixedConfigFaultPlugin : IActPluginV1
{
    public void InitPlugin(TabPage tab, Label status)
    {
        var bridge = AppDomain.CurrentDomain.GetAssemblies().Single(assembly => assembly.GetName().Name == "Aftercast.Bridge");
        var entry = bridge.GetType("Aftercast.Bridge.EntryPoint", throwOnError:true)!;
        var lazy = entry.GetField("plugins", BindingFlags.Static | BindingFlags.NonPublic)!.GetValue(null)!;
        var session = lazy.GetType().GetProperty("Value")!.GetValue(lazy)!;
        var host = (HostEnvironment)session.GetType().GetField("environment", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(session)!;
        const string xml = "<Config><CustomTriggers><Trigger Category='FaultFixture' Regex='fault'/></CustomTriggers><SpellTimers><Spell Name='Fault' Category='FaultFixture'/></SpellTimers></Config>";
        var preview = host.PreviewMixedConfig(xml);
        Action<string> fail = stage => { if (stage == "triggers-published") throw new IOException("synthetic publication fault"); };
        try
        {
            typeof(HostEnvironment).GetMethod("ImportMixedConfigCore", BindingFlags.Instance | BindingFlags.NonPublic)!
                .Invoke(host, [xml, preview.Triggers.Revision, preview.Timers.Revision, fail]);
            throw new InvalidOperationException("Synthetic fault was not reached");
        }
        catch (TargetInvocationException error) when (error.InnerException is IOException && host.IsFaulted) { }
        status.Text = "Synthetic fault injected";
    }
    public void DeInitPlugin() { }
}
