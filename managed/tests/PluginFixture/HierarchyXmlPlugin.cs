using System.Globalization;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.Testing;
namespace Aftercast.PluginFixture;

// Original synthetic objects only; no user data, clipboard, network or exporter file access.
public class HierarchyXmlPlugin:IActPluginV1
{
    protected virtual bool Html=>false;
    protected virtual bool ExpandedXml=>false;
    private const string Source="Source日本😀",Target="Target한글😀\r\n<&>";
    private FormActMain root=null!;private int owner;private bool delayed,expanded,htmlColumns;
    private FormActMain.DateTimeLogParser? parser;private string? previousCharacter;
    private readonly HashSet<object> actions=new(ReferenceEqualityComparer.Instance);
    private void Own(object value)
    {
        if(Environment.CurrentManagedThreadId!=owner)throw new InvalidOperationException("Hierarchy callback left owner thread");
        var valid=value switch
        {
            CombatantData actor=>actor.Parent.Items.Values.Any(row=>ReferenceEquals(row,actor)),
            DamageTypeData category=>category.Parent!.Items.Values.Any(row=>ReferenceEquals(row,category)),
            AttackType attack=>attack.Parent!.Items.Values.Any(row=>ReferenceEquals(row,attack)),
            MasterSwing swing=>actions.Contains(swing),_=>false
        };
        if(!valid)throw new InvalidOperationException("Hierarchy callback copied original row");
    }
    private string Actor(CombatantData actor)
    {
        Own(actor);
        if(actor.Damage==444&&!delayed){delayed=true;Thread.Sleep(2100);}
        if(actor.Damage==555)foreach(var zone in root.ZoneList)zone.Items.Remove(actor.Parent);
        if(actor.Damage==666)throw new InvalidOperationException(Html?"synthetic hierarchy display failure":"synthetic hierarchy SQL failure");
        if(actor.Damage==777)return "\ud800";
        if(actor.Damage==888)return new string('x',4097);
        return actor.Name;
    }
    private string Action(MasterSwing row)
    {Own(row);return row.Damage.Number==999?new string('&',3500):"action:"+row.Damage.Number.ToString(CultureInfo.InvariantCulture);}
    public void InitPlugin(TabPage page,Label status)
    {
        root=ActGlobals.oFormActMain!;owner=Environment.CurrentManagedThreadId;parser=root.GetDateTimeFromLog;previousCharacter=ActGlobals.charName;ActGlobals.charName=Source;
        root.TimeStampLen=0;root.GetDateTimeFromLog=line=>DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0],CultureInfo.InvariantCulture));
        CombatFixture.ConfigureCategories();root.BeforeLogLineRead+=Read;root.OnCombatEnd+=End;
        CombatantData.ColumnDefs.Clear();DamageTypeData.ColumnDefs.Clear();AttackType.ColumnDefs.Clear();MasterSwing.ColumnDefs.Clear();
        CombatantData.ColumnDefs["registered-a"]=new("Label",false,"TEXT","first",_=>throw new Exception("display callback"),row=>{Own(row);return row.Parent.EncId;},(_,_)=>0);
        CombatantData.ColumnDefs["registered-b"]=new("Other",true,"TEXT","second",_=>throw new Exception("display callback"),Actor,(_,_)=>0);
        DamageTypeData.ColumnDefs["registered-a"]=new("Label",false,"TEXT","first",_=>"display",row=>{Own(row);return row.Type!;});
        DamageTypeData.ColumnDefs["registered-b"]=new("Other",true,"TEXT","second",_=>"display",row=>{Own(row);return "category";});
        AttackType.ColumnDefs["registered-a"]=new("Label",false,"TEXT","first",_=>"display",row=>{Own(row);return row.Type!;},(_,_)=>0);
        AttackType.ColumnDefs["registered-b"]=new("Other",true,"TEXT","second",_=>"display",row=>{Own(row);return "attack";},(_,_)=>0);
        MasterSwing.ColumnDefs["registered-a"]=new("Label",false,"TEXT","first",_=>"display",row=>{Own(row);if(row.Damage.Number==333)throw new Exception("synthetic first action SQL failure");return "unused\u0001";},(_,_)=>0);
        MasterSwing.ColumnDefs["registered-b"]=new("Other",true,"TEXT","second",_=>"display",Action,(_,_)=>0);
        if(Html)ConfigureHtml();
    }
    private void ConfigureHtml()
    {
        CombatantData.ColumnDefs["simple-numeric"]=new("Simple numeric",false,"FLOAT","simple",Actor,row=>{Own(row);return row.Damage.ToString(CultureInfo.InvariantCulture);},(left,right)=>{Own(left);Own(right);return string.CompareOrdinal(left.Name,right.Name);});
        CombatantData.ColumnDefs["registered-a"].GetCellData=Actor;
        CombatantData.ColumnDefs["registered-b"].GetCellData=row=>{Own(row);return "<script>alert('x')</script><img src=x onerror=boom>&\""+row.Name;};
        CombatantData.ColumnDefs["registered-a"].SortComparer=(left,right)=>{Own(left);Own(right);if(left.Damage==222||right.Damage==222)throw new Exception("synthetic hierarchy sort failure");return string.CompareOrdinal(left.Name,right.Name);};
        DamageTypeData.ColumnDefs["registered-a"].GetCellData=row=>{Own(row);return row.Type!;};
        DamageTypeData.ColumnDefs["registered-b"].GetCellData=row=>{Own(row);return "category";};
        AttackType.ColumnDefs["registered-a"].GetCellData=row=>{Own(row);return row.Type!;};
        AttackType.ColumnDefs["registered-b"].GetCellData=row=>{Own(row);return "attack";};
        MasterSwing.ColumnDefs["registered-a"].GetCellData=row=>{Own(row);if(row.Damage.Number==333)throw new Exception("synthetic hierarchy action display failure");return row.Damage.Number.ToString(CultureInfo.InvariantCulture);};
        MasterSwing.ColumnDefs["registered-b"].GetCellData=Action;
    }
    private void Read(bool import,LogLineEventArgs info)
    {
        if(!import)throw new InvalidOperationException("Hierarchy fixture requires synthetic import");
        var amount=long.Parse(info.logLine.Split('|')[1],CultureInfo.InvariantCulture);
        if(Html&&amount==1111)CombatantData.ColumnDefs.Remove("registered-b");
        if(Html&&amount==1113)CombatantData.OutgoingDamageTypeDataObjects["New Synthetic Category"]=new("New Synthetic Category",0,System.Drawing.Color.Red);
        if(Html&&amount==1114)
        {
            var previous=root.ZoneList.SelectMany(zone=>zone.Items).FirstOrDefault(encounter=>encounter.Items.Count>0);
            if(previous is not null){var key=previous.Items.Keys[0];previous.Items[key]=new CombatantData(previous.Items[key].Name,previous);}
        }
        if(Html&&amount==1112)CombatantData.ColumnDefs["registered-b"]=new("Re-registered label",true,"TEXT","second",row=>{Own(row);return "re-registered "+row.Name;},Actor,(_,_)=>0);
        if((Html||ExpandedXml)&&amount==999&&!htmlColumns)
        {
            htmlColumns=true;
            for(var index=0;index<32;index++)
            {
                var key="expanded-"+index.ToString(CultureInfo.InvariantCulture);
                MasterSwing.ColumnDefs[key]=new("expanded label",false,"TEXT",key,Action,Action,(_,_)=>0);
            }
        }
        root.SetEncounter(info.detectedTime,Source,Target);
        var row=new MasterSwing(1,false,"None",new Dnum(amount),info.detectedTime,root.GlobalTimeSorter,"Strike",Source,"Physical",Target);actions.Add(row);root.AddCombatAction(row);
    }
    private void End(bool import,CombatToggleEventArgs info)
    {
        if(!import||info.encounter is null)return;info.encounter.Title=Target;
        if(Html&&!expanded&&info.encounter.Items.Values.Any(actor=>actor.Damage==6400))
        {
            expanded=true;var zone=root.ZoneList[0];
            for(var index=0;index<63;index++)zone.Items.Add(new EncounterData(Source,"Synthetic empty",zone){Title="Empty "+index.ToString(CultureInfo.InvariantCulture)});
        }
    }
    public void DeInitPlugin(){root.OnCombatEnd-=End;root.BeforeLogLineRead-=Read;root.GetDateTimeFromLog=parser!;ActGlobals.charName=previousCharacter;}
}

public sealed class HierarchyHtmlPlugin:HierarchyXmlPlugin
{protected override bool Html=>true;}

// Keep normal XML behavior unchanged; isolate serialized-size checks from large-row timing.
public sealed class HierarchyXmlExpandedPlugin:HierarchyXmlPlugin
{protected override bool ExpandedXml=>true;}

public sealed class OdbcPreviewPlugin : HierarchyXmlPlugin, IActPluginV1
{
    public new void InitPlugin(TabPage page, Label status)
    {
        base.InitPlugin(page,status);
        EncounterData.ColumnDefs["odbc-synthetic"] = new("Synthetic",false,"TEXT","enc_sql",_=>"display",_=>"encounter");
        CombatantData.ColumnDefs["odbc-encid"] = new("Synthetic",false,"TEXT","encid",_=>"display",_=>"nonempty-encid",(_,_)=>0);
    }
    public new void DeInitPlugin() { EncounterData.ColumnDefs.Remove("odbc-synthetic"); base.DeInitPlugin(); }
}
