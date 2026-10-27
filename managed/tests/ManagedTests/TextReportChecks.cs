using System.Globalization;
using System.Text;
using System.Text.RegularExpressions;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Testing;

internal static class TextReportChecks
{
    private static void Check(bool value,string message) {if(!value)throw new Exception("Owned text report: "+message);}
    private static T Throws<T>(Action action) where T:Exception
    {try{action();}catch(T error){return error;}throw new Exception("Expected "+typeof(T).Name);}
    private sealed class Fixture:IDisposable
    {
        private readonly DirectoryInfo directory=Directory.CreateTempSubdirectory("aftercast-text-report-");
        internal readonly HostEnvironment Host;
        internal readonly FormActMain Root;
        internal readonly EncounterData Encounter;
        internal readonly ulong Id;
        internal int Calls;
        private readonly int owner=Environment.CurrentManagedThreadId;
        internal Fixture()
        {
            Host=new(directory.FullName);Root=ActGlobals.oFormActMain!;CombatFixture.ConfigureCategories();
            var zone=Root.ZoneList[0];Encounter=new("Alpha","Fixture",zone);zone.Items.Add(Encounter);
            foreach(var name in new[]{"Alpha","Unknown","Zed"})Encounter.Items.Add(name.ToUpperInvariant(),new CombatantData(name,Encounter));
            Encounter.SetAllies([Encounter.Items["ALPHA"]]);
            CombatantData.ColumnDefs["Name"]=new("Displayed name",true,"TEXT","SqlName",actor=>actor.Name,actor=>actor.Name,
                (left,right)=>{Own(left);Own(right);return string.CompareOrdinal(left.Name,right.Name);});
            CombatantData.ExportVariables["name"]=new("name","","",(actor,_)=>{Own(actor);return actor.Name;});
            CombatantData.ExportVariables["extra"]=new("extra","","",(actor,extra)=>{Own(actor);return extra;});
            CombatantData.ExportVariables["n"]=new("n","","",(_,_)=>"\n");
            EncounterData.ExportVariables["group"]=new("group","","",(encounter,allies,_)=>{Check(ReferenceEquals(encounter,Encounter)&&allies.All(actor=>Encounter.Items.Values.Any(known=>ReferenceEquals(known,actor))),"original encounter/allies");return allies.Count.ToString(CultureInfo.InvariantCulture);});
            Id=Host.ListEncounters(0,16).Items.Single().Id;
        }
        private void Own(CombatantData actor) {Check(Environment.CurrentManagedThreadId==owner&&Encounter.Items.Values.Any(known=>ReferenceEquals(known,actor)),"owner thread/original object");Calls++;}
        internal TextReport Read(TextReportOptions options) =>Host.ReadTextReport(Id,Host.ReadTextReportCatalog().Revision,options)??throw new Exception("missing selected encounter");
        public void Dispose(){try{Host.Dispose();}finally{directory.Delete(true);}}
    }
    internal static void Run()
    {
        HostLifetimeChecks.RunBounded(FormattingAndSelection);
        HostLifetimeChecks.RunBounded(CallbacksAndRevision);
        HostLifetimeChecks.RunBounded(BudgetsAndParser);
        HostLifetimeChecks.RunBounded(ScopeChanges);
        HostLifetimeChecks.RunBounded(AllyScopeChanges);
        Console.WriteLine("Owned text reports: ACT unaligned framing/sort/allies, selected identity, owner callbacks, revisions, limits and failure cleanup passed.");
    }
    private static TextReportOptions Options(string player="{name}",bool allies=false,bool info=false,string group="")=>new(player,"Name",allies,info,group);
    private static void FormattingAndSelection()
    {
        using var f=new Fixture();
        Check(f.Read(Options("[{name}:{extra:X}:{missing:discard}]")).Text=="[Zed:X:{missing}][Alpha:X:{missing}]\n","ACT descending sort/Unknown/extra/unknown variable");
        Check(f.Read(Options("{name}{n}",true)).Text=="Alpha\n\n","allies-only and trailing empty LF");
        Check(f.Read(Options("{name}",true,true,"group:{group};")).Text=="group:1;Alpha\n","group formatter and actor scope");
        f.Encounter.SetAllies([]);
        Check(f.Read(Options("{name}{n}",true)).Text=="Zed\nAlpha\n\n","ACT empty-allies fallback keeps all actors");
        foreach(var text in new[]{"","a","\n","a\r\n\nb","한글日本語😀",string.Join('\n',Enumerable.Repeat("line",300))})
        {
            var option=Options(text);var actual=f.Read(option);
            Check(actual.Text==f.Root.GetTextExport(f.Encounter,new TextExportFormatOptions(option.PlayerFormat,option.Sorting,option.ShowOnlyAllies,option.ShowAlliedInfo,option.AlliesFormat)),"shared ACT framing: "+text.Length);
            Check(actual.EncounterId==f.Id&&actual.Revision!=0,"owned origin/revision");
        }
        var catalog=f.Host.ReadTextReportCatalog();catalog.PlayerVariables[0]="caller mutation";
        Check(!f.Host.ReadTextReportCatalog().PlayerVariables.Contains("caller mutation")&&f.Host.ReadTextReportCatalog().Revision==catalog.Revision,"caller-owned catalog arrays");
        f.Calls=0;Check(f.Host.ReadTextReport(ulong.MaxValue,catalog.Revision,Options()) is null&&f.Calls==0,"missing encounter never rebinds");
        Throws<ArgumentException>(()=>f.Host.ReadTextReport(0,catalog.Revision,Options()));
        f.Encounter.Items.Clear();f.Encounter.SetAllies([]);Check(f.Read(Options()).Text=="\n","empty encounter final LF");
        f.Root.ZoneList[0].Items.Clear();Check(f.Host.ReadTextReport(f.Id,catalog.Revision,Options()) is null,"removed selection is unavailable");
        var zone=f.Root.ZoneList[0];var selective=new EncounterData("Alpha","Selective",true,zone);zone.Items.Add(selective);
        selective.Items.Add("ALPHA",new CombatantData("Alpha",selective));selective.Items.Add("ZED",new CombatantData("Zed",selective));
        selective.SetAllies([selective.Items["ALPHA"]]);
        var owner=Environment.CurrentManagedThreadId;
        CombatantData.ColumnDefs["Name"].SortComparer=(left,right)=>{Check(Environment.CurrentManagedThreadId==owner&&selective.Items.Values.Any(known=>ReferenceEquals(known,left))&&selective.Items.Values.Any(known=>ReferenceEquals(known,right)),"selective original owner objects");return string.CompareOrdinal(left.Name,right.Name);};
        CombatantData.ExportVariables["name"].GetExportString=(actor,_)=>{Check(selective.Items.Values.Any(known=>ReferenceEquals(known,actor)),"selected original actor");return actor.Name;};
        var selected=f.Host.ListEncounters(0,16).Items.Single().Id;
        var options=Options("{name}",true,true,"group:{group};");
        EncounterData.ExportVariables["group"].GetExportString=(_,allies,_)=>allies.Count.ToString(CultureInfo.InvariantCulture);
        Check(f.Host.ReadTextReport(selected,f.Host.ReadTextReportCatalog().Revision,options)!.Text=="group:2;ZedAlpha\n","selective IgnoreEnemies bypasses manual allies as ACT does");
    }
    private static void CallbacksAndRevision()
    {
        using var f=new Fixture();var initial=f.Host.ReadTextReportCatalog();
        CombatantData.ExportVariables["added"]=new("added","","",(_,_)=>"new");
        Throws<InvalidOperationException>(()=>f.Host.ReadTextReport(f.Id,initial.Revision,Options()));
        var current=f.Host.ReadTextReportCatalog();Check(current.Revision>initial.Revision,"catalog changes invalidate stale request");
        CombatantData.ExportVariables["fail"]=new("fail","","",(_,_)=>throw new InvalidDataException("synthetic original callback failure"));
        Throws<InvalidDataException>(()=>f.Read(Options("{fail}")));
        CombatantData.ExportVariables["reenter"]=new("reenter","","",(_,_)=>{f.Host.ReadTextReportCatalog();return "bad";});
        Throws<InvalidOperationException>(()=>f.Read(Options("{reenter}")));
        CombatantData.ExportVariables["combat"]=new("combat","","",(_,_)=>{f.Root.SetEncounter(DateTime.UnixEpoch,"Alpha","Target");return "bad";});
        Throws<InvalidOperationException>(()=>f.Read(Options("{combat}")));
        CombatantData.ExportVariables["dispose"]=new("dispose","","",(_,_)=>{f.Host.Dispose();return "bad";});
        Throws<InvalidOperationException>(()=>f.Read(Options("{dispose}")));
        Check(!f.Root.IsActClosing&&ReferenceEquals(ActGlobals.oFormActMain,f.Root)&&!File.Exists(Path.Combine(f.Root.AppDataFolder.FullName,"CustomTriggers.xml")),"unsafe Dispose rejected before lifecycle/persistence effects");
        CombatantData.ExportVariables["change"]=new("change","","",(_,_)=>{CombatantData.ExportVariables["change"].GetExportString=(_,_)=>"replacement";return "old";});
        Throws<InvalidOperationException>(()=>f.Read(Options("{change}")));
        CombatantData.ExportVariables["both"]=new("both","","",(_,_)=>{CombatantData.ExportVariables.Remove("added");throw new InvalidDataException("original body failure");});
        var both=Throws<AggregateException>(()=>f.Read(Options("{both}")));
        Check(both.InnerExceptions.Any(error=>error.Message=="original body failure")&&both.InnerExceptions.Any(error=>error.Message.Contains("registry changed",StringComparison.Ordinal)),"preserve callback and postvalidation errors");
        var newerCalls=0;var column=CombatantData.ColumnDefs["Name"];
        var original=column.SortComparer;
        column.SortComparer=(_,_)=>{column.SortComparer=(_,_)=>{newerCalls++;return 0;};return 0;};
        Throws<InvalidOperationException>(()=>f.Read(Options()));Check(newerCalls==0,"no unapproved second sort callback");column.SortComparer=original;
        CombatantData.ExportVariables["reported"]=new("reported","","",(_,_)=>{f.Root.WriteExceptionLog(new InvalidOperationException("synthetic caught failure"),"report callback");return "success-looking";});
        Throws<InvalidOperationException>(()=>f.Read(Options("{reported}")));
        Check(f.Read(Options()).Text=="ZedAlpha\n","failure released query guard");
        Throws<InvalidOperationException>(()=>Task.Run(()=>f.Host.ReadTextReportCatalog()).GetAwaiter().GetResult());
    }
    private static void BudgetsAndParser()
    {
        using var f=new Fixture();f.Encounter.Items.Remove("ZED");f.Encounter.Items.Remove("UNKNOWN");
        var bulk=new string('x',1024*1024-1);
        CombatantData.ExportVariables["bulk"]=new("bulk","","",(_,_)=>bulk);
        Check(Encoding.UTF8.GetByteCount(f.Read(Options("{bulk}")).Text)==1024*1024,"exact output byte budget including final LF");
        bulk+="x";Throws<InvalidDataException>(()=>f.Read(Options("{bulk}")));
        foreach(var invalid in new[]{new string('x',16385),new string('한',5462)})Throws<ArgumentException>(()=>f.Read(Options(invalid)));
        Throws<EncoderFallbackException>(()=>f.Read(Options("\ud800")));
        Throws<ArgumentException>(()=>f.Read(Options() with{Sorting="bad\nkey"}));
        var parser=f.Root.TextExportFormatterRegex;
        f.Root.TextExportFormatterRegex=new("(a+)+$");
        Throws<RegexMatchTimeoutException>(()=>f.Read(Options(new string('a',16000)+"!")));
        f.Root.TextExportFormatterRegex=parser;
        CombatantData.ExportVariables["slow"]=new("slow","","",(_,_)=>{Thread.Sleep(2100);return "done";});
        Throws<TimeoutException>(()=>f.Read(Options("{slow}")));
        Check(f.Read(Options()).Text=="Alpha\n","timeout released query guard");
        CombatantData.ExportVariables["invalid"]=null!;Throws<InvalidDataException>(()=>f.Host.ReadTextReportCatalog());CombatantData.ExportVariables.Remove("invalid");
        var original=CombatantData.ExportVariables;
        CombatantData.ExportVariables=Enumerable.Range(0,1025).ToDictionary(index=>index.ToString(CultureInfo.InvariantCulture),_=>original["name"]);
        Throws<InvalidDataException>(()=>f.Host.ReadTextReportCatalog());CombatantData.ExportVariables=original;
        var revision=f.Host.ReadTextReportCatalog().Revision;f.Host.Dispose();
        Throws<ObjectDisposedException>(()=>f.Host.ReadTextReport(f.Id,revision,Options()));
    }
    private static void ScopeChanges()
    {
        foreach(var change in new[]{"clear actors","replace actor","remove encounter","replace zones","replace actors"})
        {
            using var f=new Fixture();
            CombatantData.ExportVariables["scope"]=new("scope","","",(_,_)=>{
                if(change=="clear actors")f.Encounter.Items.Clear();
                else if(change=="replace actor")f.Encounter.Items["ZED"]=new CombatantData("Zed",f.Encounter);
                else if(change=="replace zones")f.Root.ZoneList=new(f.Root.ZoneList);
                else if(change=="replace actors")f.Encounter.Items=new(f.Encounter.Items);
                else f.Root.ZoneList[0].Items.Remove(f.Encounter);
                return "stale";
            });
            Throws<InvalidOperationException>(()=>f.Read(Options("{scope}")));
        }
    }
    private static void AllyScopeChanges()
    {
        foreach(var change in new[]{"allies","character","title","passed clone","registered clone"})
        {
            using var f=new Fixture();
            EncounterData.ExportVariables["changeAllies"]=new("changeAllies","","",(encounter,allies,_)=>{
                if(change=="allies")encounter.SetAllies([encounter.Items["ZED"]]);
                else if(change=="passed clone")allies[0]=new CombatantData(allies[0].Name,encounter);
                else if(change=="registered clone")encounter.SetAllies([new CombatantData(allies[0].Name,encounter)]);
                else if(change=="character")encounter.CharName="Zed";
                else encounter.Title="changed";
                return "stale";
            });
            Throws<InvalidOperationException>(()=>f.Read(Options("{name}",true,true,"{changeAllies}")));
        }
    }

}
