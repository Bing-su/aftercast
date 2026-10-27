using System.Drawing;
using System.Xml.Linq;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Testing;

internal static class TableReportChecks
{
    private static void Check(bool value,string message){if(!value)throw new Exception("Table reports: "+message);}
    private static T Throws<T>(Action action)where T:Exception
    {try{action();}catch(T error){return error;}throw new Exception("Expected "+typeof(T).Name);}
    private sealed class Fixture:IDisposable
    {
        private readonly DirectoryInfo directory=Directory.CreateTempSubdirectory("aftercast-table-report-");
        internal readonly HostEnvironment Host;internal readonly FormActMain Root;internal readonly EncounterData Encounter;
        internal readonly ulong Id;internal readonly HashSet<object> Known=new(ReferenceEqualityComparer.Instance);
        internal readonly int Owner=Environment.CurrentManagedThreadId;internal int SqlCalls,CellCalls;internal Action? Callback;
        internal Fixture()
        {
            Host=new(directory.FullName);Root=ActGlobals.oFormActMain!;CombatFixture.ConfigureCategories();
            var zone=Root.ZoneList[0];Encounter=new("A","Synthetic <&> 日本😀",zone);zone.Items.Add(Encounter);
            foreach(var name in new[]{"A","B","C"})
            {
                var actor=new CombatantData(name,Encounter);Encounter.Items.Add(name,actor);Known.Add(actor);
                var category=new DamageTypeData(true,"Category",actor);actor.Items["Category"]=category;Known.Add(category);
                var attack=new AttackType("Attack",category);category.Items.Add("Attack",attack);Known.Add(attack);
                for(var index=0;index<2;index++) {var swing=new MasterSwing(1,false,"None",new Dnum(10),DateTime.UnixEpoch,index,"Attack",name,"Physical","Target");attack.Items.Add(swing);Known.Add(swing);}
            }
            foreach(var actor in Encounter.Items.Values)
                foreach(var category in actor.Items.Values)
                {
                    Known.Add(category);
                    foreach(var attack in category.Items.Values){Known.Add(attack);foreach(var action in attack.Items)Known.Add(action);}
                }
            CombatantData.ColumnDefs.Clear();DamageTypeData.ColumnDefs.Clear();AttackType.ColumnDefs.Clear();MasterSwing.ColumnDefs.Clear();
            CombatantData.ColumnDefs["display-key"]=new("Different label",true,"TEXT","sql_name",actor=>Cell(actor,"cell-"+actor.Name),actor=>Sql(actor,"sql-"+actor.Name),
                (left,right)=>{Own(left);Own(right);return string.CompareOrdinal(left.Name,right.Name);}){GetCellForeColor=_=>throw new Exception("Color callback must not run")};
            CombatantData.ColumnDefs["hidden-key"]=new("Hidden label",false,"TEXT","hidden",actor=>Cell(actor,"hidden-cell"),actor=>Sql(actor,"hidden-sql"),(_,_)=>0);
            DamageTypeData.ColumnDefs["category-key"]=new("category label",true,"TEXT","category_sql",value=>Cell(value,"cell-cat"),value=>Sql(value,"sql-cat"));
            AttackType.ColumnDefs["attack-key"]=new("attack label",true,"TEXT","attack_sql",value=>Cell(value,"cell-attack"),value=>Sql(value,"sql-attack"),(_,_)=>0);
            MasterSwing.ColumnDefs["action-key"]=new("action label",true,"TEXT","action_sql",value=>Cell(value,"cell-action"),value=>Sql(value,"sql-action"),(_,_)=>0);
            Id=Host.ListEncounters(0,16).Items.Single().Id;
        }
        internal void Own(object value){Check(Environment.CurrentManagedThreadId==Owner&&Known.Contains(value),"original callback owner/object");}
        internal string Sql(object value,string text){Own(value);SqlCalls++;Callback?.Invoke();return text;}
        internal string Cell(object value,string text){Own(value);CellCalls++;Callback?.Invoke();return text;}
        internal TableScope Scope(TableLevel level=TableLevel.Combatant)=>new(level,Id,level>=TableLevel.Category?"A":null,level>=TableLevel.Attack?"Category":null,level>=TableLevel.Action?"Attack":null);
        internal TableReportCatalog Catalog(TableScope? scope=null)=>Host.ReadTableReportCatalog(scope??Scope())??throw new Exception("missing catalog");
        internal TableReport Xml(TableReportCatalog catalog,int[]? selected=null,TableSort? sort=null,string name="Caption <&> 日本😀")=>Host.ReadTableReport(catalog.Revision,new(TableReportFormat.TableXml,name,selected,Sort:sort));
        public void Dispose(){try{Host.Dispose();}finally{directory.Delete(true);}}
    }
    internal static void Run()
    {
        HostLifetimeChecks.RunBounded(XmlAndSelection);
        HostLifetimeChecks.RunBounded(XmlCharacters);
        HostLifetimeChecks.RunBounded(HtmlEscaping);
        HostLifetimeChecks.RunBounded(OwnershipAndFailure);
        HostLifetimeChecks.RunBounded(BoundsAndEmpty);
        HostLifetimeChecks.RunBounded(StrictFailuresAndSerialization);
        Console.WriteLine("Owned current-table XML SQL values/roots/selection, escaped static HTML, dynamic columns, sorting and failure boundaries passed.");
    }
    private static void XmlAndSelection()
    {
        using var f=new Fixture();var catalog=f.Catalog();
        catalog.Rows[0]=new(99,99,"caller",null,null,null);catalog.Columns[0]=catalog.Columns[0] with{SqlDataName="caller"};
        var report=f.Xml(catalog);var xml=XDocument.Parse(report.Content);
        Check(report.Rows==3&&xml.Root!.Name=="EncounterTable"&&xml.Root.Attribute("Name")!.Value=="Caption <&> 日本😀","XML envelope");
        Check(xml.Root!.Elements("Row").First().Elements().Select(element=>element.Name.LocalName).SequenceEqual(["sql_name","hidden"]),"all SQL columns, registration order, no labels or visible filter");
        Check(xml.Root!.Elements("Row").First().Element("sql_name")!.Value=="sql-A"&&f.CellCalls==0&&f.SqlCalls==6,"SQL values once, not cell/color callbacks");
        using(var memory=new MemoryStream())
        {
            using(var writer=new System.Xml.XmlTextWriter(memory,new System.Text.UTF8Encoding(false))){writer.Formatting=System.Xml.Formatting.Indented;writer.Indentation=4;writer.Namespaces=false;
                writer.WriteStartDocument();writer.WriteStartElement("","EncounterTable","");writer.WriteAttributeString("","Name","","Caption <&> 日本😀");
                foreach(var actor in new[]{"A","B","C"}){writer.WriteStartElement("","Row","");writer.WriteElementString("sql_name","sql-"+actor);writer.WriteElementString("hidden","hidden-sql");writer.WriteEndElement();}
                writer.WriteEndElement();writer.WriteEndDocument();writer.Flush();
                var expected=System.Text.Encoding.UTF8.GetString(memory.ToArray());
                Check(report.Content.Replace("\r\n","\n",StringComparison.Ordinal)==expected.Replace("\r\n","\n",StringComparison.Ordinal),"XML vector follows original XmlTextWriter calls/indentation");
            }
        }

        Check(f.Xml(catalog,[1]).Rows==3,"ACT single selection still exports all rows");
        var selected=XDocument.Parse(f.Xml(catalog,[2,0],new("display-key",Descending:true)).Content);
        Check(selected.Root!.Elements("Row").Select(row=>row.Element("sql_name")!.Value).SequenceEqual(["sql-C","sql-A"]),"selected snapshot indices and requested sort");
        foreach(var (level,root) in new[]{(TableLevel.Category,"CombatantTable"),(TableLevel.Attack,"DamageTypeTable"),(TableLevel.Action,"AttackTypeTable")})
            Check(XDocument.Parse(f.Xml(f.Catalog(f.Scope(level))).Content).Root!.Name==root,"ACT XML root mapping");
        Throws<InvalidOperationException>(()=>f.Xml(catalog));
    }
    private static void XmlCharacters()
    {
        using var f=new Fixture();var caption="A\r\nB\tC<&>\"日本😀";var value="x\r\ny\rz\n\t<&>";
        CombatantData.ColumnDefs.Clear();CombatantData.ColumnDefs["key"]=new("label",true,"TEXT","data",_=>"cell",actor=>f.Sql(actor,value),(_,_)=>0);
        var actual=XDocument.Parse(f.Xml(f.Catalog(),name:caption).Content);
        using var stream=new MemoryStream();using(var writer=new System.Xml.XmlTextWriter(stream,new System.Text.UTF8Encoding(false))){writer.Formatting=System.Xml.Formatting.Indented;writer.Indentation=4;writer.Namespaces=false;
            writer.WriteStartDocument();writer.WriteStartElement("EncounterTable");writer.WriteAttributeString("Name",caption);
            for(var index=0;index<3;index++){writer.WriteStartElement("Row");writer.WriteElementString("data",value);writer.WriteEndElement();}
            writer.WriteEndElement();writer.WriteEndDocument();writer.Flush();
            var expected=XDocument.Parse(System.Text.Encoding.UTF8.GetString(stream.ToArray()));
            Check(actual.Root!.Attribute("Name")!.Value==expected.Root!.Attribute("Name")!.Value,"original caption CR/LF/tab semantics");
            Check(actual.Root.Element("Row")!.Element("data")!.Value==expected.Root.Element("Row")!.Element("data")!.Value,"original SQL CR/LF/tab semantics");
        }
        CombatantData.ColumnDefs["key"].GetSqlData=_=>"bad\u0001character";
        Throws<System.Xml.XmlException>(()=>f.Xml(f.Catalog()));
    }
    private static void HtmlEscaping()
    {
        using var f=new Fixture();
        var value="<script>alert('x')</script>&\"<img src=x onerror=boom> 한글日本😀";
        CombatantData.ColumnDefs["<key>&\""]=new("ignored label",true,"TEXT","valid",actor=>f.Cell(actor,value),actor=>f.Sql(actor,"not HTML"),(_,_)=>0);
        var catalog=f.Catalog();var html=f.Host.ReadTableReport(catalog.Revision,new(TableReportFormat.StaticHtml,value,Columns:["<key>&\""]));
        Check(html.Rows==3&&f.SqlCalls==0&&f.CellCalls==3,"HTML cell-only callbacks");
        Check(html.Content.Contains("<meta charset=\"utf-8\">",StringComparison.Ordinal)&&html.Content.Contains("Content-Security-Policy",StringComparison.Ordinal),"static document encoding/policy");
        Check(!html.Content.Contains("<script",StringComparison.OrdinalIgnoreCase)&&!html.Content.Contains("<img",StringComparison.OrdinalIgnoreCase)&&!html.Content.Contains(" href=",StringComparison.OrdinalIgnoreCase)&&!html.Content.Contains(" src=\"",StringComparison.OrdinalIgnoreCase),"no generated script/link/resource attributes");
        Check(html.Content.Contains("&lt;script&gt;",StringComparison.Ordinal)&&html.Content.Contains("&amp;",StringComparison.Ordinal)&&html.Content.Contains("&quot;",StringComparison.Ordinal),"escaped plugin/title/header text");
        Throws<ArgumentException>(()=>f.Host.ReadTableReport(catalog.Revision,new(TableReportFormat.StaticHtml,"name",[0],Columns:["display-key"])));
        Check(!Directory.GetFiles(f.Root.AppDataFolder.FullName).Any(),"rendering creates no files");
    }
    private static void OwnershipAndFailure()
    {
        using var f=new Fixture();var catalog=f.Catalog();
        Throws<InvalidOperationException>(()=>Task.Run(()=>f.Xml(catalog)).GetAwaiter().GetResult());
        f.Callback=()=>f.Host.ReadTableReportCatalog(f.Scope());Throws<InvalidOperationException>(()=>f.Xml(catalog));f.Callback=null;
        Check(f.Xml(catalog).Rows==3,"guard released after failure");
        f.Callback=()=>f.Encounter.Items["B"]=new CombatantData("B",f.Encounter);
        Throws<InvalidOperationException>(()=>f.Xml(catalog));f.Callback=null;
        var replacement=f.Encounter.Items["B"];f.Known.Add(replacement);
        Throws<InvalidOperationException>(()=>f.Xml(catalog));catalog=f.Catalog();
        var old=CombatantData.ColumnDefs["display-key"].GetSqlData;
        CombatantData.ColumnDefs["display-key"].GetSqlData=actor=>throw new IOException("synthetic SQL failure");
        Throws<InvalidOperationException>(()=>f.Xml(catalog));catalog=f.Catalog();
        Throws<IOException>(()=>f.Xml(catalog));CombatantData.ColumnDefs["display-key"].GetSqlData=old;
        catalog=f.Catalog();f.Callback=()=>{f.Encounter.Items.Clear();throw new IOException("body failure");};
        var both=Throws<AggregateException>(()=>f.Xml(catalog));Check(both.Flatten().InnerExceptions.Any(error=>error.Message=="body failure"),"body error retained with scope failure");f.Callback=null;
    }
    private static void BoundsAndEmpty()
    {
        using var f=new Fixture();var catalog=f.Catalog();
        foreach(var rows in new[]{new[]{-1},new[]{3},new[]{0,0}})Throws<ArgumentException>(()=>f.Xml(catalog,rows));
        Throws<ArgumentException>(()=>f.Host.ReadTableReport(catalog.Revision,new(TableReportFormat.TableXml,"name",Columns:["display-key"])));
        CombatantData.ColumnDefs["display-key"]=new("label",true,"TEXT","bad name",_=>"cell",_=>"sql",(_,_)=>0);catalog=f.Catalog();Throws<System.Xml.XmlException>(()=>f.Xml(catalog));
        CombatantData.ColumnDefs["display-key"]=new("label",true,"TEXT","valid",_=>"cell",_=>new string('x',4097),(_,_)=>0);catalog=f.Catalog();Throws<InvalidDataException>(()=>f.Xml(catalog));
        f.Encounter.Items.Clear();catalog=f.Catalog();Check(f.Xml(catalog).Rows==0&&!XDocument.Parse(f.Xml(catalog).Content).Root!.Elements().Any(),"empty table document");
        Throws<ArgumentException>(()=>f.Host.ReadTableReportCatalog(new(TableLevel.Encounter)));
        var absent=f.Host.ReadTableReportCatalog(new(TableLevel.Combatant,ulong.MaxValue));Check(absent is null,"missing scope");Throws<InvalidOperationException>(()=>f.Xml(catalog));
    }
    private static void StrictFailuresAndSerialization()
    {
        using var f=new Fixture();var catalog=f.Catalog();
        f.Callback=()=>f.Host.Dispose();Throws<InvalidOperationException>(()=>f.Xml(catalog));f.Callback=null;
        Check(!f.Root.IsActClosing&&ReferenceEquals(ActGlobals.oFormActMain,f.Root),"Dispose rejected before side effects");
        f.Callback=()=>f.Root.WriteExceptionLog(new IOException("reported failure"),"synthetic report");
        Throws<InvalidOperationException>(()=>f.Xml(catalog));f.Callback=null;Check(f.Xml(catalog).Rows==3,"recover after reported owner error");
        CombatantData.ColumnDefs["display-key"].SortComparer=(_,_)=>throw new IOException("sort body");catalog=f.Catalog();
        var sort=Throws<InvalidOperationException>(()=>f.Xml(catalog,sort:new("display-key")));
        Check(sort.ToString().Contains("sort body",StringComparison.Ordinal),"sort callback detail retained");
        CombatantData.ColumnDefs["display-key"].SortComparer=(_,_)=>1;catalog=f.Catalog();
        Throws<InvalidOperationException>(()=>f.Xml(catalog,sort:new("display-key")));
        CombatantData.ColumnDefs["display-key"].GetSqlData=_=>"\ud800";catalog=f.Catalog();Throws<System.Text.EncoderFallbackException>(()=>f.Xml(catalog));
        var serialization=typeof(HostEnvironment).Assembly.GetType("Aftercast.ActCompat.TableReportSerialization")!.GetMethod("Serialize",System.Reflection.BindingFlags.Static|System.Reflection.BindingFlags.NonPublic)!;
        var column=new TableColumnData("key","label",true,"TEXT","sql",true,false);
        var matrix=Enumerable.Range(0,1024).Select(_=>new[]{new string('&',4096)}).ToArray();
        try{serialization.Invoke(null,[TableReportFormat.StaticHtml,TableLevel.Combatant,"name",new[]{column},matrix]);throw new Exception("Escaped output budget missing");}
        catch(System.Reflection.TargetInvocationException error){Check(error.InnerException is InvalidDataException,"serialized output byte budget");}
        var actor=f.Encounter.Items["A"];actor.Items.Clear();var emptyCategory=new DamageTypeData(true,"Empty",actor);actor.Items["Empty"]=emptyCategory;
        var empty=f.Catalog(new(TableLevel.Attack,f.Id,"A","Empty"));
        actor.Items=new(){{"Empty",new DamageTypeData(true,"Empty",actor)}};
        Throws<InvalidOperationException>(()=>f.Xml(empty));
    }

}
