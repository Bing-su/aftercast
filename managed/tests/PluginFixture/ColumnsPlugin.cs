using System.Drawing;
using System.Globalization;
using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Aftercast.PluginFixture;

public class ColumnsPlugin : IActPluginV1
{
    private readonly ImportPlugin parser = new();
    private int owner, reads;
    protected FormActMain Root = null!;
    private void Own()
    {
        if (Environment.CurrentManagedThreadId != owner) throw new InvalidOperationException("Table callback left plugin owner thread");
    }
    protected virtual string Format(CombatantData value)
    {
        Own();
        if (!value.Parent.Items.Values.Any(actor => ReferenceEquals(actor, value)))
            throw new InvalidOperationException("Table callback received a copied actor");
        return value.Name + ":" + value.Damage.ToString(CultureInfo.InvariantCulture);
    }
    public void InitPlugin(TabPage page, Label status)
    {
        parser.InitPlugin(page, status);
        Root = ActGlobals.oFormActMain!;
        owner = Environment.CurrentManagedThreadId;
        EncounterData.ColumnDefs["fixture"] = new("Encounter fixture", true, "TEXT", "value", value => { Own(); return value.Title; }, _ => "sql");
        CombatantData.ColumnDefs["fixture"] = new("Actor fixture", true, "TEXT", "value", Format, _ => "sql",
            (left, right) => { Own(); return left.Damage.CompareTo(right.Damage); }) { GetCellForeColor = _ => { Own(); return Color.Red; } };
        CombatantData.ColumnDefs["bad"] = new("Bad fixture", false, "TEXT", "value", _ => throw new InvalidOperationException("synthetic table callback failure"), _ => "sql", (_, _) => throw new InvalidOperationException("synthetic sort failure"));
        CombatantData.ColumnDefs["null"] = new("Null fixture", false, "TEXT", "value", _ => null!, _ => "sql", (_, _) => 0);
        CombatantData.ColumnDefs["probe"] = new("Actor fixture", false, "TEXT", "value", _ => { Own(); return (++reads).ToString(CultureInfo.InvariantCulture); }, _ => "sql", (_, _) => 0);
        CombatantData.ColumnDefs["mutate"] = new("Mutate fixture", false, "TEXT", "value", _ => { Own(); CombatantData.ColumnDefs.Remove("mutate"); return "removed"; }, _ => "sql", (_, _) => 0);
        DamageTypeData.ColumnDefs["fixture"] = new("Category fixture", true, "TEXT", "value", value => { Own(); return value.Damage.ToString(CultureInfo.InvariantCulture); }, _ => "sql");
        AttackType.ColumnDefs["fixture"] = new("Attack fixture", true, "TEXT", "value", value => { Own(); return value.Damage.ToString(CultureInfo.InvariantCulture); }, _ => "sql", (left, right) => left.Damage.CompareTo(right.Damage));
        MasterSwing.ColumnDefs["fixture"] = new("Action fixture", true, "TEXT", "value", value => { Own(); return value.Damage.Number.ToString(CultureInfo.InvariantCulture); }, _ => "sql", (left, right) => left.TimeSorter.CompareTo(right.TimeSorter));
    }
    public void DeInitPlugin()
    {
        foreach (var key in new[] { "fixture", "bad", "null", "probe", "mutate" }) CombatantData.ColumnDefs.Remove(key);
        EncounterData.ColumnDefs.Remove("fixture"); DamageTypeData.ColumnDefs.Remove("fixture");
        AttackType.ColumnDefs.Remove("fixture"); MasterSwing.ColumnDefs.Remove("fixture");
        parser.DeInitPlugin();
    }
}
// A managed synthetic blockage tests the existing process shutdown boundary, not arbitrary native-hang recovery.
public sealed class StuckColumnsPlugin : ColumnsPlugin
{
    protected override string Format(CombatantData value)
    {
        File.WriteAllText(Path.Combine(Root.AppDataFolder.FullName, "columns.entered"), "synthetic formatter entered");
        Thread.Sleep(Timeout.Infinite);
        return "unreachable";
    }
}

// Synthetic callback observes ACT globals from the isolated worker.
public sealed class SortKeysPlugin : IActPluginV1
{
    private readonly ColumnsPlugin inner = new();
    public void InitPlugin(TabPage page, Label status)
    {
        inner.InitPlugin(page, status);
        CombatantData.ColumnDefs["sort-keys"] = new("Sort keys", false, "TEXT", "keys",
            _ => ActGlobals.eDSort + "|" + ActGlobals.eDSort2, _ => "sql", (_, _) => 0);
    }
    public void DeInitPlugin() { CombatantData.ColumnDefs.Remove("sort-keys"); inner.DeInitPlugin(); }
}

// No live game or user files: expose an ACT self/ally classification through the owned table query.
public sealed class ClassificationPlugin : IActPluginV1
{
    private readonly ColumnsPlugin inner = new();
    private string? previous;
    public void InitPlugin(TabPage page, Label status) { previous = ActGlobals.charName; inner.InitPlugin(page, status); ActGlobals.charName = "Source"; }
    public void DeInitPlugin() { inner.DeInitPlugin(); ActGlobals.charName = previous; }
}
