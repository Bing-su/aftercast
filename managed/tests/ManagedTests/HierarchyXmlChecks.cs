using System.Globalization;
using System.Text;
using System.Xml;
using System.Xml.Linq;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class HierarchyXmlChecks
{
    private static void Check(bool value,string message){if(!value)throw new Exception("Hierarchy XML: "+message);}
    private static T Throws<T>(Action action)where T:Exception
    {try{action();}catch(T error){return error;}throw new Exception("Expected "+typeof(T).Name);}
    internal static void Run()
    {
        HostLifetimeChecks.RunBounded(DepthsAndReferenceVector);
        HostLifetimeChecks.RunBounded(OwnershipAndRecovery);
        HostLifetimeChecks.RunBounded(BoundsAndEmpty);
        CaptionsAndId();
        HostLifetimeChecks.RunBounded(RepresentativeScale);
        HostLifetimeChecks.RunBounded(WholeOutputBounds);
        Console.WriteLine("Hierarchy XML depths, nested SQL values/EncId override, original caption, ownership, empty/stale/failure/bounds and independent current-table capture passed.");
    }
    private static HierarchyXmlCatalog Capture(TableReportChecks.Fixture f,HierarchyXmlDepth depth=HierarchyXmlDepth.AllTables)=>f.Host.CaptureHierarchyXml(f.Id,depth)??throw new Exception("scope absent");
    private static void DepthsAndReferenceVector()
    {
        using var f=new TableReportChecks.Fixture();var current=f.Catalog();
        foreach(var depth in Enum.GetValues<HierarchyXmlDepth>())
        {
            var catalog=Capture(f,depth);var report=f.Host.ReadHierarchyXml(catalog.Revision);var xml=XDocument.Parse(report.Content);
            Check(xml.Root!.Name=="EncounterTable"&&xml.Root.Attribute("Name")!.Value==f.Encounter.ToString(),"root name");
            Check(xml.Root.Elements("Row").Count()==3,"all actor rows in original order");
            Check(xml.Descendants("CombatantTable").Any()==(depth!=HierarchyXmlDepth.MainTable),"combatant table depth");
            Check(xml.Descendants("DamageTypeTable").Any()==(depth!=HierarchyXmlDepth.MainTable),"middle depth includes attack summaries");
            Check(xml.Descendants("AttackTypeTable").Any()==(depth==HierarchyXmlDepth.AllTables),"full depth includes actions");
            Check(xml.Descendants("Row").Count()==catalog.Rows&&xml.Root.Descendants().Count(node=>node.Name.LocalName.EndsWith("Table",StringComparison.Ordinal))+1==catalog.Tables,"counts");
            Check(xml.Root.Element("Row")!.Elements().Take(2).Select(element=>element.Name.LocalName).SequenceEqual(["sql_name","hidden"]),"all SQL metadata including hidden, no cell formatter");
            Check(f.CellCalls==0,"never display/color callbacks");
            var expected=Reference(f,depth);
            Check(report.Content.Replace("\r\n","\n",StringComparison.Ordinal)==expected.Replace("\r\n","\n",StringComparison.Ordinal),"independent original-style writer vector");
            f.Xml(current); // Hierarchical captures do not consume or replace the separate current-table selection.
        }
        MasterSwing.ColumnDefs["action-key"].GetSqlData=_=>throw new Exception("overwritten first action field must not execute");
        var full=Capture(f);var fullXml=XDocument.Parse(f.Host.ReadHierarchyXml(full.Revision).Content);
        Check(fullXml.Descendants("AttackTypeTable").Elements("Row").All(row=>row.Element("action_sql")!.Value==f.Encounter.EncId),"first action field uses encounter EncId, independent of registered SQL name");
        var action=f.Catalog(f.Scope(TableLevel.Action));Throws<Exception>(()=>f.Xml(action));
        MasterSwing.ColumnDefs["later"]=new("later",false,"TEXT","later",_=>"cell",_=>"later",(_,_)=>0);
        var multi=Capture(f);Throws<Exception>(()=>f.Host.ReadHierarchyXml(multi.Revision));
        MasterSwing.ColumnDefs["action-key"].GetSqlData=_=>"ignored\u0001";
        multi=Capture(f);var overwritten=XDocument.Parse(f.Host.ReadHierarchyXml(multi.Revision).Content);
        Check(overwritten.Descendants("AttackTypeTable").Elements("Row").All(row=>row.Element("action_sql")!.Value==f.Encounter.EncId&&row.Element("later")!.Value=="later"),"unused first value ignored, but first callback failure retained when later columns exist");
    }
    private static string Reference(TableReportChecks.Fixture f,HierarchyXmlDepth depth)
    {
        using var stream=new MemoryStream();
        using(var output=new StreamWriter(stream,new UTF8Encoding(false),4096,true){NewLine="\r\n"})
        using(var writer=new XmlTextWriter(output){Formatting=Formatting.Indented,Indentation=4,Namespaces=false})
        {
            void Start(string name,string caption){writer.WriteStartElement(name);writer.WriteAttributeString("Name",caption);}
            writer.WriteStartDocument();var title=f.Encounter.ToString();Start("EncounterTable",title);
            foreach(var actor in f.Encounter.Items.Values)
            {
                writer.WriteStartElement("Row");writer.WriteElementString("sql_name","sql-"+actor.Name);writer.WriteElementString("hidden","hidden-sql");
                if(depth!=HierarchyXmlDepth.MainTable)
                {
                    Start("CombatantTable",title+" | "+actor.Name);
                    foreach(var category in actor.Items.Values)
                    {
                        writer.WriteStartElement("Row");writer.WriteElementString("category_sql","sql-cat");Start("DamageTypeTable",title+" | "+actor.Name+" | "+category.Type);
                        foreach(var attack in category.Items.Values)
                        {
                            writer.WriteStartElement("Row");writer.WriteElementString("attack_sql","sql-attack");
                            if(depth==HierarchyXmlDepth.AllTables)
                            {
                                Start("AttackTypeTable",title+" | "+actor.Name+" | "+category.Type+" | "+attack.Type);
                                foreach(var action in attack.Items){writer.WriteStartElement("Row");writer.WriteElementString("action_sql",f.Encounter.EncId);writer.WriteEndElement();}
                                writer.WriteEndElement();
                            }
                            writer.WriteEndElement();
                        }
                        writer.WriteEndElement();writer.WriteEndElement();
                    }
                    writer.WriteEndElement();
                }
                writer.WriteEndElement();
            }
            writer.WriteEndElement();writer.WriteEndDocument();
        }
        return Encoding.UTF8.GetString(stream.ToArray());
    }
    private static void OwnershipAndRecovery()
    {
        using var f=new TableReportChecks.Fixture();var capture=Capture(f);
        var other=Capture(f);Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyXml(capture.Revision));
        Check(f.Host.ReadHierarchyXml(other.Revision).Rows==other.Rows,"repeat capture");
        var foreign=Task.Run(()=>Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyXml(other.Revision)));foreign.GetAwaiter().GetResult();
        f.Callback=()=>f.Encounter.Items["C"].Items["Category"].Items.Clear();
        Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyXml(other.Revision));f.Callback=null;
        capture=Capture(f);Check(f.Host.ReadHierarchyXml(capture.Revision).Rows==capture.Rows,"recover after cross-node mutation");
        f.Callback=()=>f.Host.Dispose();Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyXml(capture.Revision));Check(!f.Root.IsActClosing,"unsafe Dispose rejected before mutation");f.Callback=null;
        f.Callback=()=>{f.Root.WriteExceptionLog(new Exception("reported"),"hierarchy fixture");throw new Exception("body failed");};
        var both=Throws<AggregateException>(()=>f.Host.ReadHierarchyXml(capture.Revision));Check(both.ToString().Contains("body failed",StringComparison.Ordinal)&&both.ToString().Contains("owner-thread",StringComparison.Ordinal),"body and validation causes retained");f.Callback=null;
        f.Callback=()=>f.Host.CaptureHierarchyXml(f.Id,HierarchyXmlDepth.MainTable);Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyXml(capture.Revision));f.Callback=null;
        Check(f.Host.ReadHierarchyXml(capture.Revision).Rows==capture.Rows,"guard released after callback failure");
        Check(f.Host.CaptureHierarchyXml(ulong.MaxValue,HierarchyXmlDepth.MainTable) is null,"missing capture");Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyXml(capture.Revision));
    }
    private static void BoundsAndEmpty()
    {
        using var f=new TableReportChecks.Fixture();Throws<ArgumentException>(()=>f.Host.CaptureHierarchyXml(f.Id,(HierarchyXmlDepth)0));
        var capture=Capture(f);CombatantData.ColumnDefs["display-key"].GetSqlData=_=>new string('x',4097);capture=Capture(f);Throws<InvalidDataException>(()=>f.Host.ReadHierarchyXml(capture.Revision));
        CombatantData.ColumnDefs["display-key"].GetSqlData=_=>"bad\u0001";capture=Capture(f);Throws<XmlException>(()=>f.Host.ReadHierarchyXml(capture.Revision));
        CombatantData.ColumnDefs["display-key"].GetSqlData=actor=>"sql-"+actor.Name;
        var actor=f.Encounter.Items["A"];var saved=actor.Items;actor.Items=new(saved);capture=Capture(f,HierarchyXmlDepth.DamageTypeTables);actor.Items=new(saved);Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyXml(capture.Revision));actor.Items=saved;
        f.Encounter.Items.Clear();capture=Capture(f);var empty=XDocument.Parse(f.Host.ReadHierarchyXml(capture.Revision).Content);Check(capture.Tables==1&&capture.Rows==0&&!empty.Descendants("Row").Any(),"actual empty hierarchy");
        var source=new CombatantData("bad\u0001",f.Encounter);f.Encounter.Items.Add("key",source);Throws<XmlException>(()=>Capture(f,HierarchyXmlDepth.DamageTypeTables));f.Encounter.Items.Clear();
        for(var index=0;index<1024;index++)f.Encounter.Items.Add(index.ToString(CultureInfo.InvariantCulture),new CombatantData(index.ToString(CultureInfo.InvariantCulture),f.Encounter));
        Throws<InvalidDataException>(()=>Capture(f,HierarchyXmlDepth.DamageTypeTables));
    }
    private static void RepresentativeScale()
    {
        using var f=new TableReportChecks.Fixture();var actor=f.Encounter.Items["A"];
        f.Encounter.Items.Remove("B");f.Encounter.Items.Remove("C");
        var category=actor.Items["Category"];var attack=category.Items["Attack"];attack.Items.Clear();
        for(var index=0;index<2000;index++){var action=new MasterSwing(1,false,"None",new Dnum(10),DateTime.UnixEpoch.AddSeconds(index),index,"Attack","A","Physical","Target");attack.Items.Add(action);f.Known.Add(action);}
        MasterSwing.ColumnDefs["second"]=new("second",false,"TEXT","second",value=>f.Cell(value,"cell"),value=>f.Sql(value,"sql"),(_,_)=>0);
        MasterSwing.ColumnDefs["third"]=new("third",false,"TEXT","third",value=>f.Cell(value,"cell"),value=>f.Sql(value,"sql"),(_,_)=>0);
        var capture=Capture(f);var clock=System.Diagnostics.Stopwatch.StartNew();
        try{var result=f.Host.ReadHierarchyXml(capture.Revision);Check(result.Rows>=2000,"representative row count");Console.WriteLine($"Hierarchy representative: 2000 action rows, 3 SQL columns, success in {clock.Elapsed.TotalMilliseconds:F0} ms");}
        catch(TimeoutException error){Check(error.Message.Contains("exceeded",StringComparison.Ordinal),"explicit budget error");Console.WriteLine($"Hierarchy representative: explicit cooperative budget failure in {clock.Elapsed.TotalMilliseconds:F0} ms");}
        var recovered=Capture(f,HierarchyXmlDepth.MainTable);Check(f.Host.ReadHierarchyXml(recovered.Revision).Rows==1,"guard recovers after representative work");
    }
    private static void WholeOutputBounds()
    {
        using var f=new TableReportChecks.Fixture();
        foreach(var actor in f.Encounter.Items.Values)
        {
            var attack=actor.Items["Category"].Items["Attack"];attack.Items.Clear();
            for(var index=0;index<300;index++){var row=new MasterSwing(1,false,"None",new Dnum(10),DateTime.UnixEpoch,index,"Attack",actor.Name,"Physical","Target");attack.Items.Add(row);f.Known.Add(row);}
        }
        MasterSwing.ColumnDefs["expanded"]=new("expanded",false,"TEXT","expanded",_=>"cell",row=>f.Sql(row,new string('&',4096)),(_,_)=>0);
        var capture=Capture(f);var expansion=Throws<InvalidDataException>(()=>f.Host.ReadHierarchyXml(capture.Revision));
        Check(expansion.Message.Contains("Serialized table report exceeds 16 MiB",StringComparison.Ordinal),"nested markup/escaping output limit");
        MasterSwing.ColumnDefs["expanded"].GetSqlData=row=>f.Sql(row,new string('x',4096));
        foreach(var key in new[]{"second","third"})MasterSwing.ColumnDefs[key]=new(key,false,"TEXT",key,_=>"cell",row=>f.Sql(row,new string('x',4096)),(_,_)=>0);
        capture=Capture(f);var aggregate=Throws<InvalidDataException>(()=>f.Host.ReadHierarchyXml(capture.Revision));
        Check(aggregate.Message.Contains("Hierarchy SQL text exceeds 8 MiB",StringComparison.Ordinal),"aggregate raw SQL bound across nodes");
        var once=true;f.Callback=()=>{if(once){once=false;Thread.Sleep(2100);}};capture=Capture(f,HierarchyXmlDepth.MainTable);
        Throws<TimeoutException>(()=>f.Host.ReadHierarchyXml(capture.Revision));f.Callback=null;
        Check(f.Host.ReadHierarchyXml(capture.Revision).Rows==3,"cooperative timeout releases admission");
    }
    private sealed class FixedHashEncounter(string character,string zone,ZoneData parent):EncounterData(character,zone,parent)
    {internal int Hash=0x1234;public override int GetHashCode()=>Hash;}
    private static void CaptionsAndId()
    {
        using var f=new TableReportChecks.Fixture();var actor=f.Encounter.Items["A"];var empty=new FixedHashEncounter("A","Zone",f.Root.ZoneList[0]){Title="日本 <&>"};
        Check(empty.EncId=="00001234","object hash x8");empty.Hash=1;Check(empty.EncId=="00001234","cached across later hash changes");
        Check(empty.ToString()=="日本 <&> - [00:00]","unset caption");
        var original=CultureInfo.CurrentCulture;
        try
        {
            foreach(var culture in new[]{"en-US","ko-KR","ja-JP"})
            {
                CultureInfo.CurrentCulture=CultureInfo.GetCultureInfo(culture);
                f.Encounter.StartTimes=[DateTime.UnixEpoch,DateTime.UnixEpoch];f.Encounter.EndTimes=[DateTime.UnixEpoch.AddHours(26).AddMinutes(3).AddSeconds(4),DateTime.UnixEpoch];
                Check(f.Encounter.DurationS=="02:03:04","ACT hours component, not total hours");
                var start=f.Encounter.StartTime;var expected=start==DateTime.MaxValue?$"{f.Encounter.Title} - [{f.Encounter.DurationS}]":$"{f.Encounter.Title} - [{f.Encounter.DurationS}] ({start.ToShortDateString()}) {start.ToLongTimeString()}";
                Check(f.Encounter.ToString()==expected,"old encounter caption/culture");
            }
        }
        finally{CultureInfo.CurrentCulture=original;}
    }
}
