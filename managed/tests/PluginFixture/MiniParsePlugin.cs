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

// Fixed numeric column and original objects; this fixture never accesses user data.
public sealed class MiniGraphFixturePlugin : IActPluginV1
{
    private readonly MiniParsePlugin inner = new();
    public void InitPlugin(TabPage page, Label label)
    {
        inner.InitPlugin(page, label);
        var root = ActGlobals.oFormActMain!;
        root.AddCombatAction(new MasterSwing(1, false, "", new Dnum(50), DateTime.Now, root.GlobalTimeSorter, "Strike", "Second Source", "Physical", "Synthetic Target"));
        CombatantData.ColumnDefs["mini-graph-value"] = new("Mini graph value", true, "REAL", "value",
            actor => actor.Damage.ToString(System.Globalization.CultureInfo.InvariantCulture),
            actor => actor.Damage.ToString(System.Globalization.CultureInfo.InvariantCulture),
            (left, right) => left.Damage.CompareTo(right.Damage));
        CombatantData.ColumnDefs["mini-graph-scaled"] = new("Scaled", true, "REAL", "scaled", _ => "", actor => (actor.Damage * 20).ToString(System.Globalization.CultureInfo.InvariantCulture), (left, right) => left.Damage.CompareTo(right.Damage));
        CombatantData.ColumnDefs["mini-graph-zero"] = new("Zero", true, "REAL", "zero", _ => "0", _ => "0", (_, _) => 0);
        CombatantData.ColumnDefs["mini-graph-negative"] = new("Negative", true, "REAL", "negative", _ => "-1", _ => "-1", (_, _) => 0);
        CombatantData.ColumnDefs["mini-graph-infinite"] = new("Infinite", true, "REAL", "infinite", _ => "∞", _ => System.Globalization.CultureInfo.GetCultureInfo("en-US").NumberFormat.PositiveInfinitySymbol, (_, _) => 0);
        CombatantData.ColumnDefs["mini-graph-fail"] = new("Fail", true, "REAL", "fail", _ => "", _ => throw new InvalidOperationException("synthetic SQL failure"), (_, _) => 0);
        CombatantData.ColumnDefs["mini-graph-partial"] = new("Partial", true, "REAL", "partial", _ => "", actor => actor.Name == "Synthetic Target" ? throw new InvalidOperationException("synthetic row failure") : actor.Damage.ToString(System.Globalization.CultureInfo.InvariantCulture), (left, right) => left.Damage.CompareTo(right.Damage));
        CombatantData.ColumnDefs["mini-graph-badsort"] = new("Bad sort", true, "REAL", "bad", _ => "1", _ => "1", (_, _) => throw new InvalidOperationException("synthetic sort failure"));
    }
    public void DeInitPlugin()
    {
        foreach (var key in new[] { "mini-graph-value", "mini-graph-scaled", "mini-graph-zero", "mini-graph-negative", "mini-graph-infinite", "mini-graph-fail", "mini-graph-partial", "mini-graph-badsort" }) CombatantData.ColumnDefs.Remove(key);
        inner.DeInitPlugin();
    }
}
