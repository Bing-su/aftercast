using System.Globalization;
using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Aftercast.PluginFixture;

// Synthetic callbacks only: no clipboard, external output or user file access.
public sealed class TextReportPlugin : IActPluginV1
{
    private readonly ImportPlugin parser = new();
    private int owner, calls;
    private readonly TextExportFormatOptions preset=new("{report-name}:{report-unicode}","report-name",false,false,"");
    private FormActMain root = null!;
    private void Own(CombatantData actor)
    {
        if (Environment.CurrentManagedThreadId != owner || !actor.Parent.Items.Values.Any(value => ReferenceEquals(value, actor)))
            throw new InvalidOperationException("Report callback lost original owner/object");
    }
    public void InitPlugin(TabPage page, Label label)
    {
        parser.InitPlugin(page, label); root = ActGlobals.oFormActMain!; owner = Environment.CurrentManagedThreadId;
        CombatantData.ColumnDefs["report-name"] = new("Displayed report name",true,"TEXT","name",actor=>actor.Name,actor=>actor.Name,
            (left,right)=>{Own(left);Own(right);return string.CompareOrdinal(left.Name,right.Name);});
        CombatantData.ExportVariables["report-name"] = new("name","","",(actor,_)=>{Own(actor);return actor.Name;});
        CombatantData.ExportVariables["report-unicode"] = new("unicode","","",(actor,_)=>{Own(actor);return "한글日本😀";});
        CombatantData.ExportVariables["report-count"] = new("count","","",(actor,_)=>{Own(actor);return (++calls).ToString(CultureInfo.InvariantCulture);});
        CombatantData.ExportVariables["report-fail"] = new("fail","","",(_,_)=>throw new InvalidOperationException("synthetic report failure"));
        CombatantData.ExportVariables["report-invalid"] = new("invalid","","",(_,_)=>"\ud800");
        CombatantData.ExportVariables["report-limit"] = new("limit","","",(_,_)=>new string('x',1024*1024));
        CombatantData.ExportVariables["report-remove"] = new("remove","","",(actor,_)=>
        {
            foreach(var zone in root.ZoneList) zone.Items.Remove(actor.Parent);
            return "removed";
        });
        CombatantData.ExportVariables["report-mutate"] = new("mutate","","",(_,_)=>{CombatantData.ExportVariables["report-added"]=new("added","","",(_,_)=>"added");return "mutated";});
        root.AddTextFormat(preset);
        EncounterData.ExportVariables["report-group"] = new("group","","",(encounter,allies,_)=>
        {
            if(Environment.CurrentManagedThreadId!=owner || allies.Any(actor=>!encounter.Items.Values.Any(value=>ReferenceEquals(value,actor))))
                throw new InvalidOperationException("Report group lost original owner/object");
            return "group";
        });
    }
    public void DeInitPlugin()
    {
        root.TextExportFormats.RemoveAll(value=>ReferenceEquals(value,preset));
        foreach(var key in new[]{"report-name","report-unicode","report-count","report-fail","report-invalid","report-limit","report-mutate","report-added","report-remove"}) CombatantData.ExportVariables.Remove(key);
        CombatantData.ColumnDefs.Remove("report-name"); EncounterData.ExportVariables.Remove("report-group"); parser.DeInitPlugin();
    }
}
