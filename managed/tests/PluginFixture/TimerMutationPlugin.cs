using Advanced_Combat_Tracker;
using System.Windows.Forms;

namespace Aftercast.PluginFixture;

// Own only synthetic timers and parser input. No capture, external I/O or real user settings.
public sealed class TimerMutationPlugin : IActPluginV1
{
    private readonly ImportPlugin parser = new();
    private FormActMain? root;
    private int changes;
    public void InitPlugin(TabPage tab,Label status)
    {
        parser.InitPlugin(tab,status);
        root = ActGlobals.oFormActMain!;
        var timers = ActGlobals.oFormSpellTimers!;
        var definition = new TimerData("Fixture timer","Fixture") { TimerValue = 3600, StartSoundData = "none", WarningSoundData = "none" };
        if (!timers.TimerDefs.ContainsKey(definition.Key)) timers.AddEditTimerDef(definition);
        timers.RebuildSpellTreeView();
        root.LastKnownTime = DateTime.UnixEpoch;
        timers.NotifySpell("Source","Fixture timer",true,"Target",true);
        root.OnLogLineRead += Change;
    }
    private void Change(bool import,LogLineEventArgs info)
    {
        if (!import || !info.originalLogLine.EndsWith("|edit-timer",StringComparison.Ordinal)) return;
        var tooltip = "external-" + (++changes);
        foreach (var definition in ActGlobals.oFormSpellTimers!.TimerDefs.Values) definition.Tooltip = tooltip;
    }
    public void DeInitPlugin() { root!.OnLogLineRead -= Change; parser.DeInitPlugin(); }
}
