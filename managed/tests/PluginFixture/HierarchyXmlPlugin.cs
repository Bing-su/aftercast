using System.Globalization;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.Testing;
namespace Aftercast.PluginFixture;

// Original synthetic objects only; no user data, clipboard, network or exporter file access.
public sealed class HierarchyXmlPlugin:IActPluginV1
{
    private const string Source="Source日本😀",Target="Target한글😀\r\n<&>";
    private FormActMain root=null!;private int owner;private bool delayed;
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
        if(actor.Damage==666)throw new InvalidOperationException("synthetic hierarchy SQL failure");
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
    }
    private void Read(bool import,LogLineEventArgs info)
    {
        if(!import)throw new InvalidOperationException("Hierarchy fixture requires synthetic import");
        var amount=long.Parse(info.logLine.Split('|')[1],CultureInfo.InvariantCulture);root.SetEncounter(info.detectedTime,Source,Target);
        var row=new MasterSwing(1,false,"None",new Dnum(amount),info.detectedTime,root.GlobalTimeSorter,"Strike",Source,"Physical",Target);actions.Add(row);root.AddCombatAction(row);
    }
    private static void End(bool import,CombatToggleEventArgs info){if(import&&info.encounter is not null)info.encounter.Title=Target;}
    public void DeInitPlugin(){root.OnCombatEnd-=End;root.BeforeLogLineRead-=Read;root.GetDateTimeFromLog=parser!;ActGlobals.charName=previousCharacter;}
}
