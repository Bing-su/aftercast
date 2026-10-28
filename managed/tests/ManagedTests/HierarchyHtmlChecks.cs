using System.Net;
using System.Text.RegularExpressions;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class HierarchyHtmlChecks
{
    private static void Check(bool value,string message){if(!value)throw new Exception("Hierarchy HTML: "+message);}
    private static T Throws<T>(Action action)where T:Exception
    {try{action();}catch(T error){return error;}throw new Exception("Expected "+typeof(T).Name);}
    private static HierarchyHtmlLevel[] Options(TableLevel last=TableLevel.Action)=>Enumerable.Range(1,(int)last).Select(level=>new HierarchyHtmlLevel((TableLevel)level,[level switch{1=>"display-key",2=>"category-key",3=>"attack-key",_=>"action-key"}])).ToArray();
    private static HierarchyHtmlCatalog Capture(TableReportChecks.Fixture f,TableLevel last=TableLevel.Action,ulong[]? ids=null)=>f.Host.CaptureHierarchyHtml(ids??[f.Id],last)??throw new Exception("missing HTML scope");
    internal static void Run()
    {
        HostLifetimeChecks.RunBounded(SimpleOwnedGraph);
        HostLifetimeChecks.RunBounded(SimpleFailureGuard);
        HostLifetimeChecks.RunBounded(GraphsFromOwnedActions);
        HostLifetimeChecks.RunBounded(GraphSelectiveNames);
        HostLifetimeChecks.RunBounded(DepthsColumnsAndSourceOrder);
        HostLifetimeChecks.RunBounded(SelectedTreeOrderAndIndependence);
        HostLifetimeChecks.RunBounded(ErrorsMutationAndRecovery);
        HostLifetimeChecks.RunBounded(BoundsEmptyAndBudgets);
        HostLifetimeChecks.RunBounded(WholeOutputBounds);
        HostLifetimeChecks.RunBounded(EmptyDescendantRegistryChanges);
        HostLifetimeChecks.RunBounded(VirtualNamesAreGuarded);
        HostLifetimeChecks.RunBounded(StaleVirtualNameIsNotInvoked);
        HostLifetimeChecks.RunBounded(PrivateNodeIdentityDoesNotCallPluginHash);
        Console.WriteLine("Owned hierarchy HTML: checked-set tree order, four table levels, display keys/sort, escaped static source, revisions, bounds and failure cleanup passed.");
    }
    private static void SimpleOwnedGraph()
    {
        using var f=new TableReportChecks.Fixture();
        CombatantData.ColumnDefs["display-key"].GetSqlData=actor=>f.Sql(actor,actor.Name switch {"A"=>"10.2","B"=>"0.5",_=>"0"});
        var settings=new HierarchyHtmlGraphOptions(1,true,"Resist","unused","All","Warded/",Levels:[TableLevel.Combatant],Simple:true,SimpleColumn:"display-key",OnlyAllies:false);
        var capture=Capture(f);var report=f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Combatant),settings);
        Check(Regex.Matches(report.Content,"<rect").Count==2 && report.Content.Contains("12.1",StringComparison.Ordinal),"Simple SQL graph and axis");
        Check(report.Content.Contains("Total: 11",StringComparison.Ordinal),"Simple totals displayed");
        var calls=f.CellCalls;Throws<ArgumentException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Combatant),settings with{SimpleColumn="missing"}));
        Check(f.CellCalls==calls,"Simple key validated before cells");
        _=f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Combatant),settings with{OnlyAllies=true});
        _=f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Combatant),settings with{OnlyAllies=false});
        CombatantData.ColumnDefs["display-key"].GetSqlData=actor=>actor.Name=="C" ? throw new IOException("synthetic row failure") : f.Sql(actor,"5");
        capture=Capture(f);report=f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Combatant),settings);
        Check(Regex.Matches(report.Content,"<rect").Count==2,"Simple skips SQL row failures");
        CombatantData.ColumnDefs["display-key"].GetSqlData=actor=>f.Sql(actor,"0");
        capture=Capture(f);report=f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Combatant),settings);
        Check(report.Content.Contains("No positive values",StringComparison.Ordinal),"Simple empty values explicit");
        f.Encounter.Items.Clear();capture=Capture(f);report=f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Combatant),settings);
        Check(report.Content.Contains("No actors",StringComparison.Ordinal),"Simple empty actors explicit");
    }
    private static void SimpleFailureGuard()
    {
        using var f=new TableReportChecks.Fixture();
        var settings=new HierarchyHtmlGraphOptions(1,true,"Resist","unused","All","Warded/",Levels:[TableLevel.Combatant],Simple:true,SimpleColumn:"display-key",OnlyAllies:false);
        CombatantData.ColumnDefs["display-key"].GetSqlData=actor=>f.Sql(actor,"NaN");
        var capture=Capture(f);Throws<InvalidDataException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Combatant),settings));
        CombatantData.ColumnDefs["display-key"].GetSqlData=actor=>{f.Own(actor);f.Encounter.Items.Clear();throw new IOException("synthetic mutation");};
        capture=Capture(f);Throws<AggregateException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Combatant),settings));
    }
    private static void GraphsFromOwnedActions()
    {
        using var f=new TableReportChecks.Fixture();
        f.Encounter.Items.Remove("B"); f.Encounter.Items.Remove("C");
        var actor=f.Encounter.Items["A"];
        foreach(var time in new[]{0,3}) actor.AddCombatAction(new MasterSwing(1,false,"None",new Dnum(10),DateTime.UnixEpoch.AddSeconds(time),time,"Strike<&", "A", "Fire", "Target"));
        foreach(var category in actor.Items.Values)
        {
            f.Known.Add(category);
            foreach(var attack in category.Items.Values){f.Known.Add(attack);foreach(var action in attack.Items)f.Known.Add(action);}
        }
        AttackType.ColumnDefs["attack-key"].GetSqlData=attack=>f.Sql(attack,"10");
        var settings=new HierarchyHtmlGraphOptions(1,true,"attack-key","action-key","All","Warded/");
        var capture=Capture(f);var cellCalls=f.CellCalls;
        Throws<ArgumentException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options(),settings with{DistributionColumn="missing"}));
        Check(f.CellCalls==cellCalls,"invalid graph column rejected before value callbacks");
        var html=f.Host.ReadHierarchyHtml(capture.Revision,Options(),settings).Content;
        Check(html.Contains("<polyline",StringComparison.Ordinal)&&html.Contains("<rect",StringComparison.Ordinal)&&html.Contains("A100 100",StringComparison.Ordinal),"owned line, pie and bars rendered");
        Check(html.Contains("Strike&lt;&amp;",StringComparison.Ordinal)&&!html.Contains("<script",StringComparison.OrdinalIgnoreCase),"graph text escaped");
        Check(f.SqlCalls>0,"distribution uses original SQL callbacks");
        var shallow=f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Combatant),settings with{Levels=[TableLevel.Combatant]});
        Check(Regex.Matches(shallow.Content,"<table>").Count==1 && shallow.Content.Contains("<polyline",StringComparison.Ordinal) && !shallow.Content.Contains("<rect",StringComparison.Ordinal) && !shallow.Content.Contains("A100 100",StringComparison.Ordinal),"independent shallow table and graph depths");
        Throws<ArgumentException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Combatant),settings with{Levels=[TableLevel.Action]}));
        var onlyBars=f.Host.ReadHierarchyHtml(capture.Revision,Options(),settings with{Levels=[TableLevel.Action]});
        Check(onlyBars.Content.Contains("<rect",StringComparison.Ordinal)&&!onlyBars.Content.Contains("<polyline",StringComparison.Ordinal),"independent action graph");
        var stale=Capture(f,TableLevel.Combatant);Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options(),settings));
        Throws<ArgumentException>(()=>f.Host.ReadHierarchyHtml(stale.Revision,Options(TableLevel.Combatant),settings));
        capture=Capture(f);AttackType.ColumnDefs["attack-key"].GetSqlData=attack=>{f.Own(attack);throw new IOException("synthetic graph SQL");};
        capture=Capture(f);Throws<IOException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options(),settings));
        AttackType.ColumnDefs["attack-key"].GetSqlData=attack=>attack.Type == "All" ? f.Sql(attack,"10") : throw new IOException("non-All row failure");
        capture=Capture(f);Check(f.Host.ReadHierarchyHtml(capture.Revision,Options(),settings).Content.Contains("<svg",StringComparison.Ordinal),"non-All SQL exception drops row");
        AttackType.ColumnDefs["attack-key"].GetSqlData=attack=>{f.Own(attack);if(attack.Type=="All"){attack.Items.Clear();throw new IOException("mutating SQL");}return "10";};
        capture=Capture(f);Throws<AggregateException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options(),settings));
        AttackType.ColumnDefs["attack-key"].GetSqlData=attack=>f.Sql(attack,"10");
        capture=Capture(f);Check(f.Host.ReadHierarchyHtml(capture.Revision,Options(),settings).Content.Contains("<svg",StringComparison.Ordinal),"recovery after callback failure");
        var selected=new[]{"Damage"};var selecting=settings with{SelectedCombatants=["A"],SoloCategories=selected};
        capture=Capture(f);f.Callback=()=>selected[0]="Healing";
        var selectedHtml=f.Host.ReadHierarchyHtml(capture.Revision,Options(),selecting).Content;f.Callback=null;
        Check(selectedHtml.Contains(">Damage</text>",StringComparison.Ordinal)&&!selectedHtml.Contains(">Healing</text>",StringComparison.Ordinal),"solo selection snapshot respected");
        Throws<ArgumentException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options(),settings with{SoloCategories=["Damage","Damage"]}));
        Throws<ArgumentException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options(),settings with{DistributionColumn="Resist", WardPrefix="", Levels=[TableLevel.Attack]}));
        MasterSwing.ColumnDefs["action-key"].SortComparer=(left,right)=>{f.Own(left);f.Own(right);Capture(f);return 0;};
        capture=Capture(f);Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options(),settings));
        MasterSwing.ColumnDefs["action-key"].SortComparer=(_,_)=>0;
        capture=Capture(f);Check(f.Host.ReadHierarchyHtml(capture.Revision,Options(),settings).Content.Contains("<svg",StringComparison.Ordinal),"sort reentry releases guard");
    }
    private static void GraphSelectiveNames()
    {
        using var f=new TableReportChecks.Fixture();
        var first=f.Encounter.Items["A"];f.Encounter.Items.Clear();f.Encounter.Items.Add("alias",first);
        var second=new CombatantData("A",f.Encounter);f.Encounter.Items.Add("other",second);f.Known.Add(second);
        foreach(var actor in new[]{first,second})
            foreach(var time in new[]{0,3}) actor.AddCombatAction(new MasterSwing(1,false,"None",new Dnum(10),DateTime.UnixEpoch.AddSeconds(time),time,"Strike","A","Fire","Target"));
        foreach(var actor in f.Encounter.Items.Values)foreach(var category in actor.Items.Values)
        {f.Known.Add(category);foreach(var attack in category.Items.Values){f.Known.Add(attack);foreach(var action in attack.Items)f.Known.Add(action);}}
        var custom=first.Items["Category"];var customAll=new AttackType("All",custom);custom.Items["All"]=customAll;f.Known.Add(customAll);
        foreach(var action in first.Items["Damage"].Items["All"].Items)customAll.Items.Add(action);
        CombatantData.ColumnDefs["display-key"].GetSqlData=actor=>f.Sql(actor,"10");
        var capture=Capture(f);var settings=new HierarchyHtmlGraphOptions(1,true,"Resist","unused","All","Warded/",SelectedCombatants:["alias"],Levels:[TableLevel.Combatant]);
        var report=f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Combatant),settings);
        Check(Regex.Matches(report.Content,"<polyline").Count==2,"ACT selective name matches aliases with same uppercase name");
        var simple=f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Combatant),settings with{Simple=true,SimpleColumn="display-key",OnlyAllies=false});
        Check(Regex.Matches(simple.Content,"<rect").Count==2,"Simple selective name matches aliases");
        var solo=f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Category),settings with{SelectedCombatants=null,Levels=[TableLevel.Category]});
        Check(!solo.Content.Contains(">Category</text>",StringComparison.Ordinal),"unregistered custom category excluded from ACT all selection");
        var calls=f.CellCalls;
        Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Combatant),settings with{SelectedCombatants=["missing"]}));
        Check(f.CellCalls==calls,"missing actor rejected before display callbacks");
        Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Category),settings with{SelectedCombatants=null,SoloCategories=["missing"],Levels=[TableLevel.Category]}));
    }
    private static void DepthsColumnsAndSourceOrder()
    {
        using var f=new TableReportChecks.Fixture();
        foreach(var last in new[]{TableLevel.Combatant,TableLevel.Category,TableLevel.Attack,TableLevel.Action})
        {
            var capture=Capture(f,last);Check(f.CellCalls==0&&f.SqlCalls==0,"capture has no value callbacks");
            var options=Options(last);options[0]=options[0] with{Sort=new("display-key",Descending:true)};
            var report=f.Host.ReadHierarchyHtml(capture.Revision,options);var html=report.Content;
            Check(report.LastLevel==last&&report.Encounters.SequenceEqual([f.Id]),"captured shape");
            Check(Regex.Matches(html,"<table>").Count==capture.Tables&&Regex.Matches(html,"<section id=").Count==capture.Tables,"one nested section/table per included scope");
            var table=html.Split("</table>",StringSplitOptions.None)[0];
            Check(table.IndexOf("cell-C",StringComparison.Ordinal)<table.IndexOf("cell-A",StringComparison.Ordinal),"requested actor table order");
            Check(html.Contains("<th>display-key</th>",StringComparison.Ordinal)&&!html.Contains("Different label",StringComparison.Ordinal)&&!html.Contains("sql-A",StringComparison.Ordinal),"registered keys and display values, not label/SQL");
            if(last>=TableLevel.Category)
            {
                var captions=Regex.Matches(html,"<caption>(.*?)</caption>",RegexOptions.Singleline).Select(match=>WebUtility.HtmlDecode(match.Groups[1].Value)).ToArray();
                Check(captions[1].EndsWith(" | A",StringComparison.Ordinal),"child navigation stays original A first despite root sort");
                Check(html.Contains("cell-cat",StringComparison.Ordinal),"category values");
            }
            Check(html.Contains("cell-attack",StringComparison.Ordinal)==(last>=TableLevel.Attack),"attack depth");
            Check(html.Contains("cell-action",StringComparison.Ordinal)==(last==TableLevel.Action),"action depth");
            Check(f.SqlCalls==0,"no SQL or EncId override in HTML");f.CellCalls=0;
        }
        var value="<script>alert('x')</script><img src=x onerror=boom>&\"한글日本😀";
        f.Encounter.Title=value;
        CombatantData.ColumnDefs["<key>&\""]=new("ignored",false,"TEXT","unused",actor=>f.Cell(actor,value),_=>throw new Exception("SQL must not execute"),(_,_)=>0);
        var selected=Capture(f,TableLevel.Combatant);var safe=f.Host.ReadHierarchyHtml(selected.Revision,[new(TableLevel.Combatant,["<key>&\"","hidden-key"])]).Content;
        Check(safe.Contains("&lt;script&gt;",StringComparison.Ordinal)&&safe.Contains("&quot;",StringComparison.Ordinal)&&safe.Contains("&lt;key&gt;&amp;",StringComparison.Ordinal),"escaped values, caption and exact header keys");
        Check(!safe.Contains("<script",StringComparison.OrdinalIgnoreCase)&&!safe.Contains("<img",StringComparison.OrdinalIgnoreCase)&&!safe.Contains(" src=\"",StringComparison.OrdinalIgnoreCase)&&!safe.Contains("javascript:",StringComparison.OrdinalIgnoreCase),"no executable/resources from data");
        Check(safe.Contains("default-src 'none'",StringComparison.Ordinal)&&safe.Contains("hidden-cell",StringComparison.Ordinal),"CSP and explicitly selected hidden column");
        Check(!Directory.GetFiles(f.Root.AppDataFolder.FullName).Any(),"no exported files");
    }
    private static void SelectedTreeOrderAndIndependence()
    {
        using var f=new TableReportChecks.Fixture();var zone=f.Root.ZoneList[0];
        var second=new EncounterData("A","Second",zone){Title="Second <&>"};zone.Items.Add(second);
        var ids=f.Host.ListEncounters(0,16).Items.Select(row=>row.Id).ToArray();var secondId=ids.Single(id=>id!=f.Id);
        var caller=new[]{secondId,f.Id};var capture=Capture(f,TableLevel.Action,caller);caller[0]=0;
        Check(capture.Encounters.SequenceEqual([f.Id,secondId]),"tree order, not selection argument order");
        capture.Encounters[0]=0;
        var xml=f.Host.CaptureHierarchyXml(f.Id,HierarchyXmlDepth.MainTable)!;var table=f.Catalog();
        var report=f.Host.ReadHierarchyHtml(capture.Revision,Options());
        Check(report.Encounters.SequenceEqual([f.Id,secondId]),"caller cannot mutate internal captured ID array");
        Check(report.Content.IndexOf("Synthetic",StringComparison.Ordinal)<report.Content.IndexOf("Second &lt;&amp;&gt;",StringComparison.Ordinal),"encounter source order");
        Check(f.Host.ReadHierarchyXml(xml.Revision).Rows==3&&f.Xml(table).Rows==3,"independent XML/current table selections");
        f.Callback=()=>second.Title="changed";Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options()));f.Callback=null;
        second.Title="Second <&>";
        report.Encounters[0]=0;Check(f.Host.ReadHierarchyHtml(capture.Revision,Options()).Encounters[0]==f.Id,"owned response array");
        var replacement=Capture(f,TableLevel.Combatant);Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options()));
        Check(f.Host.CaptureHierarchyHtml([ulong.MaxValue],TableLevel.Combatant) is null,"missing selection");Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyHtml(replacement.Revision,Options(TableLevel.Combatant)));
    }
    private static void ErrorsMutationAndRecovery()
    {
        using var f=new TableReportChecks.Fixture();var capture=Capture(f);
        Throws<InvalidOperationException>(()=>Task.Run(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options())).GetAwaiter().GetResult());
        var bad=Options();bad[3]=new(TableLevel.Action,["missing"]);var calls=f.CellCalls;
        Throws<ArgumentException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,bad));Check(f.CellCalls==calls,"all keys validated before callbacks");
        bad=Options();bad[1]=bad[1] with{Sort=new("category-key")};Throws<ArgumentException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,bad));
        bad=Options();bad[0]=bad[0] with{Sort=new("missing")};Throws<ArgumentException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,bad));
        var options=Options();f.Callback=()=>{options[3].Columns[0]="missing";};
        Check(f.Host.ReadHierarchyHtml(capture.Revision,options).Content.Contains("cell-action",StringComparison.Ordinal),"option arrays snapshotted before callback mutation");f.Callback=null;
        f.Callback=()=>{f.Root.WriteExceptionLog(new Exception("reported"),"HTML synthetic");throw new Exception("body failed");};
        var aggregate=Throws<AggregateException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options()));Check(aggregate.ToString().Contains("body failed",StringComparison.Ordinal)&&aggregate.ToString().Contains("owner-thread",StringComparison.Ordinal),"both causes retained");f.Callback=null;
        f.Callback=()=>f.Host.Dispose();Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options()));Check(!f.Root.IsActClosing,"unsafe Dispose rejected");f.Callback=null;
        f.Callback=()=>Capture(f);Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options()));f.Callback=null;
        Check(f.Host.ReadHierarchyHtml(capture.Revision,Options()).Rows==capture.Rows,"guard released");
        f.Callback=()=>f.Encounter.Items["C"].Items["Category"].Items.Clear();Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options()));f.Callback=null;
        capture=Capture(f);Check(f.Host.ReadHierarchyHtml(capture.Revision,Options()).Tables==capture.Tables,"explicit recapture after scope mutation");
        CombatantData.ColumnDefs["display-key"].SortComparer=(_,_)=>throw new Exception("sort failure");capture=Capture(f);
        var sorting=Options();sorting[0]=sorting[0] with{Sort=new("display-key")};Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,sorting));
        MasterSwing.ColumnDefs["action-key"].GetCellData=_=>throw new Exception("cell failure");capture=Capture(f);
        Throws<Exception>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options()));
    }
    private sealed class NamedCategory(CombatantData actor):DamageTypeData(true,"Category",actor)
    {
        internal Action? Callback;
        public override string ToString(){Callback?.Invoke();return "Category";}
    }
    private sealed class HashRegistry:Dictionary<string,CombatantData.ColumnDef>
    {
        internal int Calls;
        public override int GetHashCode(){Calls++;throw new Exception("registry hash callback must not run");}
    }
    private static void PrivateNodeIdentityDoesNotCallPluginHash()
    {
        using var f=new TableReportChecks.Fixture();var original=CombatantData.ColumnDefs;var registry=new HashRegistry();
        foreach(var pair in original)registry.Add(pair.Key,pair.Value);CombatantData.ColumnDefs=registry;
        try
        {
            var capture=Capture(f);Check(f.Host.ReadHierarchyHtml(capture.Revision,Options()).Rows>0&&registry.Calls==0,"private node dictionaries use reference identity, not plugin registry virtual hashes");
        }
        finally{CombatantData.ColumnDefs=original;}
    }
    private static void StaleVirtualNameIsNotInvoked()
    {
        foreach(var xml in new[]{false,true})
        {
            using var f=new TableReportChecks.Fixture();var categories=new List<NamedCategory>();
            foreach(var name in new[]{"A","C"})
            {
                var actor=f.Encounter.Items[name];var original=actor.Items["Category"].Items["Attack"];
                var category=new NamedCategory(actor);actor.Items["Category"]=category;f.Known.Add(category);categories.Add(category);
                var attack=new AttackType("Attack",category);category.Items.Add("Attack",attack);f.Known.Add(attack);attack.Items.AddRange(original.Items);
            }
            var revision=xml?f.Host.CaptureHierarchyXml(f.Id,HierarchyXmlDepth.AllTables)!.Revision:Capture(f).Revision;
            var calls=0;categories[0].Callback=()=>f.Encounter.Items["C"].Items.Remove("Category");categories[1].Callback=()=>calls++;
            var error=Throws<Exception>(()=>{if(xml)f.Host.ReadHierarchyXml(revision);else f.Host.ReadHierarchyHtml(revision,Options());});
            Check(error.ToString().Contains("changed",StringComparison.Ordinal)&&calls==0,"removed later scope fails before its virtual getter is called");
            categories[0].Callback=null;categories[1].Callback=null;f.Encounter.Items["C"].Items["Category"]=categories[1];
            var recovered=Capture(f);Check(f.Host.ReadHierarchyHtml(recovered.Revision,Options()).Rows>0,"stale name failure releases guard");
        }
    }
    private static void VirtualNamesAreGuarded()
    {
        foreach(var xml in new[]{false,true})foreach(var duringRender in new[]{false,true})
        {
            using var f=new TableReportChecks.Fixture();var actor=f.Encounter.Items["A"];
            var original=actor.Items["Category"].Items["Attack"];
            var category=new NamedCategory(actor);actor.Items["Category"]=category;f.Known.Add(category);
            var attack=new AttackType("Attack",category);category.Items.Add("Attack",attack);f.Known.Add(attack);attack.Items.AddRange(original.Items);
            void CaptureAndRender(Action callback)
            {
                ulong revision=0;
                if(duringRender)revision=xml?f.Host.CaptureHierarchyXml(f.Id,HierarchyXmlDepth.AllTables)!.Revision:Capture(f).Revision;
                category.Callback=callback;
                if(duringRender){if(xml)f.Host.ReadHierarchyXml(revision);else f.Host.ReadHierarchyHtml(revision,Options());}
                else {if(xml)f.Host.CaptureHierarchyXml(f.Id,HierarchyXmlDepth.AllTables);else Capture(f);}
            }
            Throws<InvalidOperationException>(()=>CaptureAndRender(()=>f.Host.Dispose()));category.Callback=null;
            Check(!f.Root.IsActClosing,"virtual name Dispose rejected before shutdown mutation");
            Throws<InvalidOperationException>(()=>CaptureAndRender(()=>Capture(f)));category.Callback=null;
            var reported=Throws<Exception>(()=>CaptureAndRender(()=>f.Root.WriteExceptionLog(new Exception("name reported"),"synthetic name")));category.Callback=null;
            Check(reported.ToString().Contains("owner-thread",StringComparison.Ordinal),"name error baseline precedes first virtual getter");
            var both=Throws<AggregateException>(()=>CaptureAndRender(()=>{category.Items.Clear();throw new Exception("name body failed");}));category.Callback=null;
            Check(both.ToString().Contains("name body failed",StringComparison.Ordinal)&&both.ToString().Contains("changed",StringComparison.Ordinal),"name failure and structural mutation both retained without recursive names");
            category.Items.Add("Attack",attack);
            var recovered=Capture(f);Check(f.Host.ReadHierarchyHtml(recovered.Revision,Options()).Rows>0,"name guard released after failure");
        }
    }
    private static void EmptyDescendantRegistryChanges()
    {
        using var f=new TableReportChecks.Fixture();
        foreach(var actor in f.Encounter.Items.Values)actor.Items["Category"].Items.Clear();
        var capture=Capture(f);var action=MasterSwing.ColumnDefs["action-key"];
        f.Callback=()=>MasterSwing.ColumnDefs.Clear();
        Throws<InvalidOperationException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options()));
        f.Callback=null;MasterSwing.ColumnDefs["action-key"]=action;
        // All requested registries stay guarded even if no row can produce a table at that depth.
        Check(f.Host.ReadHierarchyHtml(capture.Revision,Options()).Tables==capture.Tables,"recovery after empty descendant registry mutation");
    }
    private static void WholeOutputBounds()
    {
        using var f=new TableReportChecks.Fixture();
        foreach(var actor in f.Encounter.Items.Values)
        {
            var attack=actor.Items["Category"].Items["Attack"];attack.Items.Clear();
            for(var index=0;index<24;index++){var row=new MasterSwing(1,false,"None",new Dnum(10),DateTime.UnixEpoch,index,"Attack",actor.Name,"Physical","Target");attack.Items.Add(row);f.Known.Add(row);}
        }
        var keys=Enumerable.Range(0,32).Select(index=>"column-"+index).ToArray();
        MasterSwing.ColumnDefs.Clear();
        foreach(var key in keys)MasterSwing.ColumnDefs[key]=new(key,false,"TEXT",key,row=>f.Cell(row,new string('&',4096)),_=>throw new Exception("SQL must not run"),(_,_)=>0);
        var options=Options();options[3]=new(TableLevel.Action,keys.Take(13).ToArray());
        var capture=Capture(f);var expansion=Throws<InvalidDataException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,options));
        Check(expansion.Message.Contains("Serialized table report exceeds 16 MiB",StringComparison.Ordinal),"HTML escaping expansion respects serialized output bound");
        foreach(var key in keys)MasterSwing.ColumnDefs[key].GetCellData=row=>f.Cell(row,new string('x',4096));
        capture=Capture(f);options[3]=new(TableLevel.Action,keys);
        var aggregate=Throws<InvalidDataException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,options));
        Check(aggregate.Message.Contains("Hierarchy HTML text exceeds 8 MiB",StringComparison.Ordinal),"aggregate raw bound across scopes");
        var shallow=Capture(f,TableLevel.Combatant);Check(f.Host.ReadHierarchyHtml(shallow.Revision,Options(TableLevel.Combatant)).Rows==3,"explicit recovery after budget failures");
    }
    private static void BoundsEmptyAndBudgets()
    {
        using var f=new TableReportChecks.Fixture();
        Throws<ArgumentException>(()=>Capture(f,(TableLevel)0));Throws<ArgumentException>(()=>Capture(f,ids:[]));Throws<ArgumentException>(()=>Capture(f,ids:[f.Id,f.Id]));
        Throws<ArgumentException>(()=>Capture(f,ids:Enumerable.Range(0,65).Select(_=>f.Id).ToArray()));
        var capture=Capture(f);Throws<ArgumentException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,[]));
        CombatantData.ColumnDefs["display-key"].GetCellData=_=>new string('x',4097);capture=Capture(f);Throws<InvalidDataException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options()));
        CombatantData.ColumnDefs["display-key"].GetCellData=actor=>f.Cell(actor,"value");
        var once=true;f.Callback=()=>{if(once){once=false;Thread.Sleep(2100);}};capture=Capture(f,TableLevel.Combatant);
        Throws<TimeoutException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Combatant)));f.Callback=null;
        Check(f.Host.ReadHierarchyHtml(capture.Revision,Options(TableLevel.Combatant)).Rows==3,"timeout explicit same-selection recovery");
        f.Encounter.Items.Clear();capture=Capture(f);
        var empty=f.Host.ReadHierarchyHtml(capture.Revision,Options());Check(empty.Tables==1&&empty.Rows==0&&!empty.Content.Contains("<td>",StringComparison.Ordinal),"empty encounter, mandatory main table");
        var invalid=Options();invalid[3]=new(TableLevel.Action,["missing"]);Throws<ArgumentException>(()=>f.Host.ReadHierarchyHtml(capture.Revision,invalid));
    }
}
