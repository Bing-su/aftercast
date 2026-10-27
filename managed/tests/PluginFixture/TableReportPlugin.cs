using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Aftercast.PluginFixture;

// Synthetic owned-object callbacks only; no clipboard, network or exporter file access.
public sealed class TableReportPlugin : IActPluginV1
{
    private readonly ImportPlugin parser = new();
    private int owner, calls;
    private static FormActMain Root => ActGlobals.oFormActMain!;
    private void Own(object row)
    {
        if (owner != Environment.CurrentManagedThreadId) throw new InvalidOperationException("Report left owner thread");
        var actors=Root.ZoneList.SelectMany(zone=>zone.Items).SelectMany(encounter=>encounter.Items.Values).ToArray();
        if (!actors.Any(actor=>ReferenceEquals(actor,row) || actor.Items.Values.Any(category=>ReferenceEquals(category,row) ||
            category.Items.Values.Any(attack=>ReferenceEquals(attack,row) || attack.Items.Any(action=>ReferenceEquals(action,row))))))
            throw new InvalidOperationException("Report copied original row");
    }
    private string Sql(object row)
    {
        Own(row);
        if (row is CombatantData changed && changed.Damage == 555) CombatantData.ColumnDefs.Remove("bad-sort");
        if (row is CombatantData actor && actor.Damage == 666) throw new InvalidOperationException("synthetic SQL failure");
        if (row is CombatantData invalid && invalid.Damage == 777) return "\ud800";
        if (row is CombatantData large && large.Damage == 888) return new string('x',4097);
        return "sql:한글日本😀\r\n" + (row is CombatantData named ? named.Name : row is MasterSwing swing ? swing.Damage.Number.ToString(System.Globalization.CultureInfo.InvariantCulture) : "");
    }
    private string Display(object row) { Own(row); return "<script>&\"한글😀:"+(++calls); }
    public void InitPlugin(TabPage page, Label label)
    {
        parser.InitPlugin(page,label); owner=Environment.CurrentManagedThreadId;
        CombatantData.ColumnDefs.Clear(); DamageTypeData.ColumnDefs.Clear(); AttackType.ColumnDefs.Clear(); MasterSwing.ColumnDefs.Clear();
        CombatantData.ColumnDefs["fixture"] = new("Label",false,"TEXT","sql_name",row=>Display(row),row=>Sql(row),(left,right)=>{Own(left);Own(right);return string.CompareOrdinal(left.Name,right.Name);});
        DamageTypeData.ColumnDefs["fixture"] = new("Label",false,"TEXT","sql_name",row=>Display(row),row=>Sql(row));
        AttackType.ColumnDefs["fixture"] = new("Label",false,"TEXT","sql_name",row=>Display(row),row=>Sql(row),(_,_)=>0);
        MasterSwing.ColumnDefs["fixture"] = new("Label",false,"TEXT","sql_name",row=>Display(row),row=>row.Damage.Number==999 ? new string('&',4096) : Sql(row),(left,right)=>left.TimeSorter.CompareTo(right.TimeSorter));
        CombatantData.ColumnDefs["bad-sort"] = new("Label",false,"TEXT","other",row=>Display(row),row=>Sql(row),(_,_)=>throw new InvalidOperationException("synthetic report sort failure"));
    }
    public void DeInitPlugin()
    {
        CombatantData.ColumnDefs.Clear(); DamageTypeData.ColumnDefs.Clear(); AttackType.ColumnDefs.Clear(); MasterSwing.ColumnDefs.Clear(); parser.DeInitPlugin();
    }
}
