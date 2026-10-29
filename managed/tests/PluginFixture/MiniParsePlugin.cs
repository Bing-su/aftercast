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

public sealed class MiniParseEndPlugin : IActPluginV1
{
    public void InitPlugin(TabPage page, Label label) { ActGlobals.oFormActMain!.EndCombat(true); }
    public void DeInitPlugin() { }
}

public sealed class MiniParseNextPlugin : IActPluginV1
{
    public void InitPlugin(TabPage page, Label label)
    {
        var root = ActGlobals.oFormActMain!; var now = DateTime.Now.AddSeconds(1);
        root.ChangeZone("Synthetic Next Zone");
        if (!root.SetEncounter(now, "Next Source", "Next Target")) throw new InvalidOperationException("Synthetic encounter refused");
        root.AddCombatAction(new MasterSwing(1, false, "", new Dnum(20), now, root.GlobalTimeSorter, "Strike", "Next Source", "Physical", "Next Target"));
    }
    public void DeInitPlugin() { }
}

public sealed class MiniParseRegistryPlugin : IActPluginV1
{
    public void InitPlugin(TabPage page, Label label)
    {
        CombatantData.ColumnDefs["mini-extra"] = new("Mini extra", true, "TEXT", "extra", _ => "x", _ => "x", (_, _) => 0);
    }
    public void DeInitPlugin() => CombatantData.ColumnDefs.Remove("mini-extra");
}

public sealed class MiniParseEmptyPlugin : IActPluginV1
{
    public void InitPlugin(TabPage page, Label label)
    {
        var root = ActGlobals.oFormActMain!;
        if (!root.SetEncounter(DateTime.Now, "Empty Source", "Empty Target")) throw new InvalidOperationException("Synthetic empty encounter refused");
        root.EndCombat(true);
    }
    public void DeInitPlugin() { }
}
