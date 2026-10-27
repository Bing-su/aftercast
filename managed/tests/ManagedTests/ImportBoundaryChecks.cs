using System.Globalization;
using System.Text;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Testing;

internal static class ImportBoundaryChecks
{
    private static readonly DateTime Epoch=DateTime.UnixEpoch;
    private static void Check(bool value,string message){if(!value)throw new Exception("Import boundaries: "+message);}
    internal static void Run()=>HostLifetimeChecks.RunBounded(Scenarios);
    private static void Scenarios()
    {
        var directory=Directory.CreateTempSubdirectory("aftercast-boundary-");
        try
        {
            using var host=new HostEnvironment(directory.FullName);var root=ActGlobals.oFormActMain!;CombatFixture.ConfigureCategories();
            root.GetDateTimeFromLog=line=>line.StartsWith("U|",StringComparison.Ordinal)?DateTime.MinValue:Epoch.AddSeconds(int.Parse(line.Split('|')[0],CultureInfo.InvariantCulture));
            var seen=new List<string>();
            root.BeforeLogLineRead+=(import,info)=>{
                Check(import,"not import");seen.Add(info.originalLogLine);var fields=info.logLine.Split('|');
                if(fields[1]=="E"){root.EndCombat(false);return;}
                if(fields[1]!="A")return;
                root.SetEncounter(info.detectedTime,"Source","Target");
                root.AddCombatAction(new(1,false,"",new Dnum(long.Parse(fields[2],CultureInfo.InvariantCulture)),info.detectedTime,root.GlobalTimeSorter,"Strike","Source","Physical","Target"));
            };
            var path=Path.Combine(directory.FullName,"source.log");
            ImportResult Read(string text,ImportRange range){seen.Clear();File.WriteAllText(path,text);return host.ImportLog(path,range);}
            ImportRange Range(bool extend=false)=>new(Epoch.AddSeconds(1),Epoch.AddSeconds(2),extend);
            const string closing="0|N\n1|A|10\n2|A|20\n3|A|30\n4|E\n5|A|999\n1|A|999\n";
            Check(Read(closing,Range()).Damage==30&&seen.SequenceEqual(["1|A|10","2|A|20"]),"strict end changed");
            var extended=Read(closing,Range(true));
            Check(extended.Damage==60&&extended.RecordsDispatched==4&&seen[^1]=="4|E","extend must inspect pre-parse combat");
            var backwards=Read("1|A|10\n3|E\n1|A|20\n9|E\n10|N\n1|A|999\n",Range(true));
            Check(backwards.Damage==30&&backwards.Encounters==2&&backwards.RecordsDispatched==4,"regression back inside bounds or terminal rejection");
            var restarted=Read("1|A|10\n20|A|100\n21|E\n22|N\n",Range(true));
            Check(restarted.Damage==110&&restarted.Encounters==2&&restarted.RecordsDispatched==3,"accepted post-end row may start a new encounter after idle end");
            Check(Read("0|N\n3|A|999\n",Range(true)).RecordsDispatched==0,"extension cannot start idle combat outside the window");
            Check(Read("1|A|10\n3|A|20",Range(true)).Damage==30,"active EOF must drain");
            Check(Read("",Range(true)).RecordsDispatched==0,"empty EOF");
            Check(Read("1|N\n2|A|10\n2|A|20\n3|A|999\n",new(Epoch.AddSeconds(2),Epoch.AddSeconds(2))).Damage==30,"equal endpoints and duplicate timestamps");
            Check(Read("U|N\n0|A|1\n1|A|2\n3|A|999\n",new(DateTime.MinValue,Epoch.AddSeconds(1))).Damage==3&&seen[0]=="U|N","file beginning includes undated rows");
            Check(Read("0|A|999\n1|A|10\n3|A|20\n",new(Epoch.AddSeconds(1),DateTime.MaxValue)).Damage==30,"file EOF bound");
            // The extended interval consumes the malformed suffix, so it must retain prefix actions and report the failure.
            File.WriteAllBytes(path,Encoding.UTF8.GetBytes("1|A|10\n3|A|20\n").Concat(new byte[]{255,10}).ToArray());
            var before=root.ZoneList[0].Items.Count;
            try{host.ImportLog(path,Range(true));throw new Exception("malformed extended suffix accepted");}catch(InvalidDataException){}
            Check(root.ZoneList[0].Items.Count==before+1&&root.ZoneList[0].Items[^1].Damage==30&&!root.InCombat,"partial failure and restoration");
            File.WriteAllText(path,"1|A|1\n"+string.Concat(Enumerable.Repeat("3|A|1\n",5000)));host.BeginImport(path,Range(true));
            ImportResult? step;do{step=host.AdvanceImport(false);if(step is null&&host.ImportProgress.RecordsDispatched==0)Thread.Sleep(1);}while(step is null&&host.ImportProgress.RecordsDispatched==0);
            Check(step is null,"fixture completed before cancel");try{while(host.AdvanceImport(true)is null)Thread.Sleep(1);throw new Exception("cancel not reported");}catch(OperationCanceledException){}
            Check(!root.InCombat,"cancel restored parser state");
            Check(Read("1|A|5\n",Range()).Damage==5,"reuse after cancel");
            var sides=Directory.CreateDirectory(Path.Combine(directory.FullName,"CompanionLogs"));File.WriteAllText(Path.Combine(sides.FullName,"source.Alpha.log"),"2|A|100\n3|A|200\n5|A|999\n");
            File.WriteAllText(path,"1|A|10\n4|E\n5|N\n");seen.Clear();
            var merged=host.ImportLog(path,Range(true),companions:[new("Alpha")]);
            Check(merged.Damage==310&&merged.RecordsDispatched==4,"extended companion must not retire on an accepted past-end row");
            Console.WriteLine("Import per-side effective file/time bounds, pre-parse extension, regression/new encounter, EOF, failure/cancel and companion checks passed.");
        }
        finally{directory.Delete(true);}
    }
}
