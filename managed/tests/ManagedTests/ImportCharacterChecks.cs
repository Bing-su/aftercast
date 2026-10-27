using System.Globalization;
using System.Text;
using System.Xml;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Testing;

internal static class ImportCharacterChecks
{
    private static readonly DateTime Epoch = DateTime.UnixEpoch;
    private const string Combat = "1|Source|Target|100\n2|Target|Source|40\n";
    private static void Check(bool value, string reason) { if (!value) throw new Exception("Import viewpoint: " + reason); }
    private static void Reject<T>(Action action) where T : Exception
    { try { action(); } catch (T) { return; } throw new Exception("Expected " + typeof(T).Name); }
    private sealed class Fixture : IDisposable
    {
        internal readonly DirectoryInfo Directory;
        internal readonly HostEnvironment Host;
        internal readonly FormActMain Root;
        internal readonly List<string> Seen = new();
        private readonly bool ownsDirectory;
        internal string Path => System.IO.Path.Combine(Directory.FullName, "main.log");
        internal Fixture(DirectoryInfo? directory = null)
        {
            ownsDirectory = directory is null; Directory = directory ?? System.IO.Directory.CreateTempSubdirectory("aftercast-character-");
            Host = new(Directory.FullName); Root = ActGlobals.oFormActMain!; CombatFixture.ConfigureCategories();
            Root.ChangeZone("Live zone"); Root.LogFilePath = "live.log"; Root.LastKnownTime = Epoch.AddDays(1);
            Root.GetDateTimeFromLog = line => Epoch.AddSeconds(int.Parse(line.Split('|')[0], CultureInfo.InvariantCulture));
            Root.BeforeLogLineRead += (import, info) => {
                Check(import, "expected import"); Seen.Add(ActGlobals.charName!);
                var fields = info.logLine.Split('|');
                var attacker = fields[1] == "SELF" ? ActGlobals.charName! : fields[1];
                var victim = fields[2] == "SELF" ? ActGlobals.charName! : fields[2];
                Root.SetEncounter(info.detectedTime, attacker, victim);
                Root.AddCombatAction(new(1, false, "", new Dnum(long.Parse(fields[3], CultureInfo.InvariantCulture)), info.detectedTime,
                    Root.GlobalTimeSorter, "Strike", attacker, "Physical", victim));
            };
        }
        internal ImportResult Read(string text, ImportCharacterOptions? viewpoint)
        { Seen.Clear(); File.WriteAllText(Path, text); return Host.ImportLog(Path, viewpoint: viewpoint); }
        internal EncounterData Latest => Root.ZoneList[0].Items.Last();
        internal void Restored(string character = "Source") => Check(ActGlobals.charName == character && Root.LogFilePath == "live.log" &&
            Root.CurrentZone == "Live zone" && Root.LastKnownTime == Epoch.AddDays(1) && !Root.InCombat, "restored exact live context");
        public void Dispose() { try { Host.Dispose(); } finally { if (ownsDirectory) Directory.Delete(true); } }
    }
    internal static void Run()
    {
        HostLifetimeChecks.RunBounded(ChoicesAndOwnership);
        HostLifetimeChecks.RunBounded(FailureCancelAndSources);
        HostLifetimeChecks.RunBounded(PrivateRestoreAndCapturedCharacter);
        Console.WriteLine("Import character ACT selection/fallback, ally viewpoint, admission ownership, failure/cancel, companion/text and exact saved-character checks passed.");
    }
    private static ImportResult Drain(HostEnvironment host)
    {
        var deadline = DateTime.UtcNow.AddSeconds(10);
        while (true) { if (host.AdvanceImport(false) is { } result) return result; Check(DateTime.UtcNow < deadline, "bounded drain"); Thread.Sleep(1); }
    }
    private static void ChoicesAndOwnership()
    {
        using var f = new Fixture();
        Check(f.Read(Combat, null).Damage == 100, "current Source viewpoint"); f.Restored();
        Check(f.Read(Combat, new("Target")).Damage == 40 && f.Latest.CharName == "Target" && f.Latest.Title == "Source", "opposing viewpoint changes ally aggregate/title");
        Check(f.Latest.Items.Count == 2 && f.Latest.GetCombatant("Source")!.Damage == 100, "viewpoint must not filter raw actors/actions"); f.Restored();
        Check(f.Read(Combat, new("Unknown actor")).Damage == 0 && f.Latest.Items.Count == 2, "unmatched viewpoint has no allied aggregate");
        foreach (var (name, effective) in new[] { ("", "Source"), ("Al", "YOU"), ("한글", "YOU"), ("😀", "YOU"), ("😀A", "😀A"), ("手動 이름", "手動 이름") })
        {
            Check(f.Read("1|SELF|Target|11\n", new(name)).Damage == 11 && f.Latest.CharName == effective && f.Seen.Single() == effective, "ACT empty/UTF-16 length fallback and parser self alias: " + name);
            f.Restored();
        }
        Check(f.Read(Combat, new(" Target ")).Damage == 0 && f.Latest.CharName == " Target ", "manual identity is not trimmed");
        ActGlobals.charName = "Al";
        Check(f.Read("1|SELF|Target|11\n", null).Damage == 11 && f.Latest.CharName == "Al", "omitted programmatic context preserves exact current short name"); f.Restored("Al");
        Check(f.Read("1|SELF|Target|11\n", new("")).Damage == 11 && f.Latest.CharName == "YOU", "explicit UI current choice applies SetCharName fallback"); f.Restored("Al");
        ActGlobals.charName = "Source";
        File.WriteAllText(f.Path, Combat); f.Seen.Clear(); f.Host.BeginImport(f.Path);
        ActGlobals.charName = "Target";
        Check(Drain(f.Host).Damage == 100 && f.Seen.All(name => name == "Source"), "current identity is copied at Begin"); f.Restored("Target");
        ActGlobals.charName = "Source";
        var choice = new ImportCharacterOptions("Target"); f.Seen.Clear(); f.Host.BeginImport(f.Path, viewpoint: choice); choice = choice with { Name = "Source" };
        ActGlobals.charName = "Other live";
        Check(Drain(f.Host).Damage == 40 && f.Seen.All(name => name == "Target"), "owned manual selection"); f.Restored("Other live"); ActGlobals.charName = "Source";
        foreach (var invalid in new[] { "bad\nname", "bad\0name", new string('x', 1025), new string('한', 342) })
            Reject<ArgumentException>(() => f.Host.BeginImport(f.Path, viewpoint: new(invalid)));
        Reject<ArgumentNullException>(() => f.Host.BeginImport(f.Path, viewpoint: new(null!)));
        Reject<EncoderFallbackException>(() => f.Host.BeginImport(f.Path, viewpoint: new("\ud800")));
        Reject<XmlException>(() => f.Host.BeginImport(f.Path, viewpoint: new("bad\uffff")));
        f.Restored();
        f.Host.BeginImport(f.Path, viewpoint: new(new string('x', 1024)));
        Reject<OperationCanceledException>(() => { while (f.Host.AdvanceImport(true) is null) Thread.Sleep(1); }); f.Restored();
    }
    private static void FailureCancelAndSources()
    {
        using var f = new Fixture();
        File.WriteAllText(f.Path, "1|Source|Target|7\nbad timestamp");
        Reject<FormatException>(() => f.Host.ImportLog(f.Path, viewpoint: new("Target")));
        Check(f.Latest.CharName == "Target" && f.Latest.Items.Count == 2 && f.Host.ListEncounters(0,100).Items.Any(row => row.Status == EncounterStatus.PartialImport), "partial failure retains selected identity"); f.Restored();
        File.WriteAllText(f.Path, string.Concat(Enumerable.Repeat(Combat, 5000))); f.Host.BeginImport(f.Path, viewpoint: new("Target"));
        var deadline = DateTime.UtcNow.AddSeconds(5);
        while (f.Host.ImportProgress.RecordsDispatched == 0) { Check(f.Host.AdvanceImport(false) is null && DateTime.UtcNow < deadline, "cancel progress"); Thread.Sleep(1); }
        Reject<OperationCanceledException>(() => { while (f.Host.AdvanceImport(true) is null) Thread.Sleep(1); });
        Check(f.Latest.CharName == "Target", "canceled encounter identity"); f.Restored();
        Reject<FileNotFoundException>(() => f.Host.ImportLog(System.IO.Path.Combine(f.Directory.FullName,"missing.log"), viewpoint:new("Target"))); f.Restored();
        var side = System.IO.Directory.CreateDirectory(System.IO.Path.Combine(f.Directory.FullName,"CompanionLogs"));
        File.WriteAllText(System.IO.Path.Combine(side.FullName,"main.Alpha.log"), "2|Target|Source|30\n");
        File.WriteAllText(f.Path,"1|Source|Target|10\n3|Target|Source|2\n");
        Check(f.Host.ImportLog(f.Path,companions:[new("Alpha")],viewpoint:new("Target")).Damage==32, "same viewpoint for companion stream"); f.Restored();
        f.Host.BeginClipboardImport("1|SELF|Target|8", new("手動 이름"));
        Check(Drain(f.Host).Damage == 8 && f.Latest.CharName == "手動 이름", "supplied Unicode text context without OS clipboard"); f.Restored();
        Check(f.Read(Combat,null).Damage==100,"no selection leak to next import");
    }
    private static void PrivateRestoreAndCapturedCharacter()
    {
        var directory = System.IO.Directory.CreateTempSubdirectory("aftercast-character-archive-");
        try
        {
            using (var f = new Fixture(directory))
            {
                Check(f.Read(Combat,new("Target")).Damage==40,"selected history graph");
                Check(f.Host.ArchiveOperation(3,"",0,false)==1,"save selected character");
            }
            using (var f = new Fixture(directory))
            {
                Check(f.Host.ArchiveOperation(4,"",0,false)==1,"restore private history");
                Check(f.Latest.CharName=="Target"&&f.Latest.Damage==40&&f.Latest.LogLines.Count==2,"restored viewpoint/logs"); f.Restored();
            }
        }
        finally { directory.Delete(true); }
        using var saved = new Fixture();
        ActGlobals.charName="Al"; saved.Root.SetEncounter(Epoch.AddSeconds(1),"Al","Target");
        saved.Root.AddCombatAction(new(1,false,"",new Dnum(5),Epoch.AddSeconds(1),1,"Strike","Al","Physical","Target")); saved.Host.EndCombat();
        Check(saved.Host.ArchiveOperation(3,"",0,false)==1,"saved exact private short name"); ActGlobals.charName="Source";
        File.WriteAllText(saved.Path,"1|SELF|Target|5\n");
        var page=saved.Host.ReadHistory(new());
        var proposal=saved.Host.ReadHistorySources(new(page.IndexRevision,[page.Items.Single().Id]),new([saved.Path],[]));
        using var capture=saved.Host.CaptureHistorySourceAsync(proposal,saved.Path).GetAwaiter().GetResult();
        saved.Host.BeginCapturedHistoryImport(capture);
        Check(Drain(saved.Host).Damage==5&&saved.Latest.CharName=="Al", "captured historical character is exact, not a manual short-name choice"); saved.Restored();
    }
}
