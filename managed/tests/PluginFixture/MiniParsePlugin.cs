using System.Windows.Forms;
using Advanced_Combat_Tracker;
namespace Aftercast.PluginFixture;

// Synthetic live combat only; no input file, audio or external output.
public sealed class MiniParsePlugin : IActPluginV1
{
    private readonly TextReportPlugin inner = new();
    public void InitPlugin(TabPage page, Label label)
    {
        inner.InitPlugin(page, label);
        var root = ActGlobals.oFormActMain!;
        var now = DateTime.Now;
        if (!root.SetEncounter(now, "Synthetic Source", "Synthetic Target")) throw new InvalidOperationException("Synthetic encounter refused");
        root.AddCombatAction(new MasterSwing(1, false, "", new Dnum(100), now, root.GlobalTimeSorter, "Strike", "Synthetic Source", "Physical", "Synthetic Target"));
    }
    public void DeInitPlugin() => inner.DeInitPlugin();
}
