using System.Diagnostics;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.Bridge;

internal static class ExternalTimerChecks
{
    internal static void Run(PluginSession session, IActPluginV1 plugin)
    {
        if (!OperatingSystem.IsWindows()) throw new PlatformNotSupportedException("Repeater's native cancel button needs Windows");
        var root = ActGlobals.oFormActMain!;
        var timers = ActGlobals.oFormSpellTimers!;
        var tab = root.PluginGetSelfData(plugin)!.tpPluginSpace;
        var epoch = DateTime.UnixEpoch;
        root.LastKnownTime = epoch;
        var definition = new TimerData("Repeat fixture", "Fixture")
        { Tooltip = "repeat", OnlyMasterTicks = true, TimerValue = 3, WarningValue = 1 };
        timers.AddEditTimerDef(definition);
        timers.RebuildSpellTreeView();
        timers.NotifySpell("Caster", definition.Name, true, "", true);
        var frame = timers.GetTimerFrames().Single();
        root.LastKnownTime = epoch.AddSeconds(4);
        session.Pump();
        session.Pump();
        if (frame.SpellTimers.Count != 2 || frame.GetLargestVal(false) != 3)
            throw new Exception("Unmodified Repeater did not restart its expired frame");
        // Let the plugin's own one-second native Timer build the real cancellation control.
        var until = Stopwatch.StartNew();
        Button? cancel = null;
        while (until.Elapsed < TimeSpan.FromSeconds(2))
        {
            session.Pump();
            cancel = tab.Controls.OfType<FlowLayoutPanel>().SelectMany(panel => panel.Controls.OfType<Button>()).SingleOrDefault();
            if (cancel is not null) break;
            Thread.Sleep(10);
        }
        if (cancel is null) throw new Exception("Repeater's native timer never created a cancel button");
        cancel.PerformClick();
        session.Pump();
        if (timers.GetTimerFrames().Count != 0) throw new Exception("Repeater cancellation did not remove its frame");
        definition.Tooltip = "repeat combat";
        timers.NotifySpell("Caster", definition.Name, true, "", true);
        frame = timers.GetTimerFrames().Single();
        root.LastKnownTime = epoch.AddSeconds(8);
        session.Pump(); session.Pump();
        if (frame.SpellTimers.Count != 1) throw new Exception("Repeater restarted repeat-combat while idle");
        frame.SpellTimers.Clear();
        session.Pump();
        root.SetEncounter(root.LastKnownTime, "Caster", "Target");
        timers.NotifySpell("Caster", definition.Name, true, "", true);
        frame = timers.GetTimerFrames().Single();
        root.LastKnownTime = epoch.AddSeconds(12);
        session.Pump(); session.Pump();
        if (frame.SpellTimers.Count != 2) throw new Exception("Repeater did not restart repeat-combat during combat");
        root.EndCombat(true);
        Console.WriteLine("Unmodified Repeater restarted expired timers, honored combat-only repetition and cancelled through its native button.");

    }
}
