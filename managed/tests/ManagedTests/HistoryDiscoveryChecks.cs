using System.Xml;
using Aftercast.ActCompat;
using Aftercast.Bridge;
using Advanced_Combat_Tracker;
using Aftercast.Testing;

internal static class HistoryDiscoveryChecks
{
    private static readonly DateTime Epoch = DateTime.UnixEpoch.AddTicks(1234567);
    private static void Check(bool condition,string message) { if (!condition) throw new Exception(message); }
    private static void Reject<T>(Action action) where T : Exception
    { try { action(); } catch (T) { return; } throw new Exception("Expected " + typeof(T).Name); }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-history-discovery-");
        try
        {
            Seed(directory.FullName);
            DiscoveryAndFailures(directory.FullName);
            SelectedIdentity(directory.FullName);
            AsyncLeaseAndOwnership(directory.FullName);
            Console.WriteLine("History metadata scopes/dates, snapshot selection, corruption/lease atomicity, private identity and async ownership passed.");
        }
        finally { directory.Delete(true); }
    }
    private static void Seed(string directory)
    {
        using var host = new HostEnvironment(directory);
        CombatFixture.ConfigureCategories();
        var root = ActGlobals.oFormActMain!;
        for (var index = 0; index < 2; index++)
        {
            root.ChangeZone("Zone " + (index == 0 ? "Alpha" : "Beta"));
            root.SetEncounter(Epoch,"Source","Target");
            root.AddCombatAction(new MasterSwing(1,false,"",new Dnum(100+index),Epoch,42+index,"Strike","Source","Physical","Target"));
            host.EndCombat();
            var encounter = root.ActiveZone.ActiveEncounter!;
            encounter.Title = index == 0 ? "Alpha fight" : "Beta fight";
            encounter.CharName = "Tester " + index;
            encounter.LogLines.Add(new(Epoch,"other log",12,10));
            encounter.LogLines.Add(new(Epoch,"raw-"+index,12,42+index));
        }
        Check(host.ArchiveOperation(3,"",0,false) == 2,"Seed checkpoint did not retain two encounters");
    }
    private static void DiscoveryAndFailures(string directory)
    {
        var index = Path.Combine(directory,"history/index.xml");
        var original = File.ReadAllBytes(index);
        File.Move(index,index+".saved");
        using var host = new HostEnvironment(directory);
        CombatFixture.ConfigureCategories();
        var root = ActGlobals.oFormActMain!;
        var missing = host.ReadHistory(new());
        Check(!missing.IndexPresent && missing.Total == 0 && missing.IndexedCount == 0,"Missing index was not an empty snapshot");
        File.Move(index+".saved",index);
        var page = host.ReadHistory(new());
        Check(page.IndexPresent && page.Total == 2 && page.IndexedCount == 2 && page.Items.Length == 2,"Metadata discovery failed");
        Check(page.Items.All(item => item.SelectiveRecording == false), "nonselective recording evidence missing");
        var legacyIndex = new XmlDocument(); legacyIndex.Load(index);
        foreach (XmlElement entry in legacyIndex.SelectNodes("/Records/Record")!) entry.RemoveAttribute("SelectiveRecording");
        legacyIndex.Save(index);
        Check(host.ReadHistory(new()).Items.All(item => item.SelectiveRecording is null), "old index invented a recording policy");
        File.WriteAllBytes(index, original);
        var contradiction = new XmlDocument(); contradiction.Load(index);
        ((XmlElement)contradiction.SelectSingleNode("/Records/Record")!).SetAttribute("SelectiveRecording", "true");
        contradiction.Save(index);
        var inconsistent = host.ReadHistory(new());
        Reject<XmlException>(() => host.RestoreSelectedHistory(new(inconsistent.IndexRevision, [inconsistent.Items.Single(item => item.SelectiveRecording == true).Id])));
        Check(host.ListEncounters(0,16).Total == 0, "metadata mismatch attached a graph");
        File.WriteAllBytes(index, original);
        Check(root.EncDatabase.Count == 0 && host.ListEncounters(0,16).Total == 0,"Browse mutated public metadata or combat graph");
        Check(page.Items.All(item => item.StartTicks == Epoch.Ticks && item.StartKind == (int)Epoch.Kind && item.PayloadPresent && !item.AlreadyRestored),"Metadata lost time kind or presence state");
        var first = host.ReadHistory(new(Limit:1)); var second = host.ReadHistory(new(Offset:1,Limit:1));
        Check(first.IndexRevision == second.IndexRevision && first.Items[0].Id != second.Items[0].Id,"Equal-time records collapsed or pages changed snapshot");
        Check(host.ReadHistory(new("ALPHA",HistorySearchScope.Title)).Items.Single().Title == "Alpha fight","Title scope/case mismatch");
        Check(host.ReadHistory(new("Zone Beta",HistorySearchScope.Zone)).Total == 1,"Zone scope mismatch");
        Check(host.ReadHistory(new("Tester 0",HistorySearchScope.Character)).Total == 1,"Character scope mismatch");
        Check(host.ReadHistory(new("Zone",HistorySearchScope.Title)).Total == 0,"Scope leaked into another field");
        Check(host.ReadHistory(new(StartTicks:Epoch.Ticks,EndTicks:Epoch.Ticks+1)).Total == 2,"Start-inclusive date boundary failed");
        Check(host.ReadHistory(new(EndTicks:Epoch.Ticks)).Total == 0 && host.ReadHistory(new(StartTicks:Epoch.Ticks+1)).Total == 0,"End-exclusive/start-based date search failed");
        Check(File.ReadAllBytes(index).SequenceEqual(original),"Browse rewrote the index");
        QueryAndSelectionValidation(host,page);
        var selected = new HistorySelection(page.IndexRevision,page.Items.Select(item => item.Id).ToArray());
        var payload = Path.Combine(directory,"history",selected.Ids[1]); var payloadBytes = File.ReadAllBytes(payload);
        root.GlobalTimeSorter = 900;
        File.WriteAllBytes(payload,[1,2,3]);
        try
        {
            Reject<InvalidDataException>(() => host.RestoreSelectedHistory(selected));
            Check(host.ListEncounters(0,16).Total == 0 && root.GlobalTimeSorter == 900,"Corrupt second payload partially attached or changed sorter");
        }
        finally { File.WriteAllBytes(payload,payloadBytes); }
        File.Move(payload,payload+".saved");
        try
        {
            Check(host.ReadHistory(new()).Items.Single(item => item.Id == selected.Ids[1]).PayloadPresent == false,"Unavailable payload was advertised present");
            Reject<FileNotFoundException>(() => host.RestoreSelectedHistory(selected));
            Check(host.ListEncounters(0,16).Total == 0,"Missing selected payload partially attached graph");
        }
        finally { File.Move(payload+".saved",payload); }
        File.WriteAllText(index,File.ReadAllText(index).Replace("Alpha fight","Changed title",StringComparison.Ordinal));
        try { Reject<InvalidOperationException>(() => host.RestoreSelectedHistory(selected)); Check(host.ListEncounters(0,16).Total == 0,"Stale snapshot attached graph"); }
        finally { File.WriteAllBytes(index,original); }
        foreach (var xml in new[] { "<Records><Record>", "<!DOCTYPE x [<!ENTITY x 'bad'>]><Records/>", File.ReadAllText(index).Replace("Archive=\"","Archive=\"../",StringComparison.Ordinal) })
        {
            File.WriteAllText(index,xml);
            try { Reject<XmlException>(() => host.ReadHistory(new())); Check(root.EncDatabase.Count == 0,"Corrupt browse partially published metadata"); }
            finally { File.WriteAllBytes(index,original); }
        }
        NestedCompression(host,directory,index,original,page);
        ExpandedBudget(host,directory,index,original,page);
        using (var lease = new FileStream(Path.Combine(directory,"history/checkpoint.lock"),FileMode.Open,FileAccess.ReadWrite,FileShare.None))
            Reject<IOException>(() => host.RestoreSelectedHistory(selected));
        Check(host.ListEncounters(0,16).Total == 0,"Competing writer lease changed graph");
        Check(host.ArchiveOperation(3,"",0,false) == 2,"Prime legacy metadata cache");
        File.Move(index,index+".saved");
        try
        {
            Check(!host.ReadHistory(new()).IndexPresent,"Removed index still reported present");
            Check(host.ArchiveOperation(4,"",0,false) == 0,"Legacy restore reused a removed index cache");
        }
        finally { File.Move(index+".saved",index); }
    }
    private static void NestedCompression(HostEnvironment host,string directory,string index,byte[] original,HistoryPage page)
    {
        var root = ActGlobals.oFormActMain!;
        var payload = File.ReadAllBytes(Path.Combine(directory,"history",page.Items[0].Id));
        var nested = root.MemCompressGZip(payload);
        var file = Convert.ToHexStringLower(System.Security.Cryptography.SHA256.HashData(nested)) + ".act";
        var path = Path.Combine(directory,"history",file);
        File.WriteAllBytes(path,nested);
        File.WriteAllText(index,System.Text.Encoding.UTF8.GetString(original).Replace(page.Items[0].Id,file,StringComparison.Ordinal));
        try
        {
            var snapshot = host.ReadHistory(new());
            Reject<XmlException>(() => host.RestoreSelectedHistory(new(snapshot.IndexRevision,[file])));
            Check(host.ListEncounters(0,16).Total == 0,"Nested compression bypassed XML normalization");
        }
        finally { File.WriteAllBytes(index,original); File.Delete(path); }
    }
    private static void ExpandedBudget(HostEnvironment host,string directory,string index,byte[] original,HistoryPage page)
    {
        var root = ActGlobals.oFormActMain!;
        var source = root.MemDecompressGzip(File.ReadAllBytes(Path.Combine(directory,"history",page.Items[0].Id)));
        var document = new XmlDocument { XmlResolver = null }; using (var sourceIndex = new MemoryStream(original,false)) document.Load(sourceIndex);
        var template = (XmlElement)document.DocumentElement!.FirstChild!;
        template = (XmlElement)template.CloneNode(true); document.DocumentElement.RemoveAll();
        var paths = new List<string>();
        try
        {
            // Ignored comments expand to 4*33 MiB but retain only tiny graphs; test the real 128 MiB policy.
            var chunk = Enumerable.Repeat((byte)'x',8192).ToArray();
            for (var item = 0; item < 4; item++)
            {
                using var output = new MemoryStream();
                using (var zip = new System.IO.Compression.GZipStream(output,System.IO.Compression.CompressionLevel.Fastest,true))
                {
                    zip.Write(source); zip.Write("<!--"u8);
                    for (var written = 0; written < 33 * 1024 * 1024; written += chunk.Length) zip.Write(chunk);
                    zip.WriteByte((byte)('0'+item)); zip.Write("-->"u8);
                }
                var bytes = output.ToArray();
                var file = Convert.ToHexStringLower(System.Security.Cryptography.SHA256.HashData(bytes)) + ".act";
                var path = Path.Combine(directory,"history",file); File.WriteAllBytes(path,bytes); paths.Add(path);
                var entry = (XmlElement)template.CloneNode(true); entry.SetAttribute("Archive",file); document.DocumentElement.AppendChild(entry);
            }
            File.WriteAllText(index,document.OuterXml);
            var snapshot = host.ReadHistory(new());
            try
            {
                host.RestoreSelectedHistory(new(snapshot.IndexRevision,snapshot.Items.Select(item => item.Id).ToArray()));
                throw new Exception("Expanded selection budget was not enforced");
            }
            catch (InvalidDataException error) { Check(error.Message.Contains("128 MiB",StringComparison.Ordinal),"Unexpected failure before aggregate XML budget"); }
            Check(host.ListEncounters(0,16).Total == 0 && root.GlobalTimeSorter == 900,"Over-budget selection partially published graph or sorter");
        }
        finally { File.WriteAllBytes(index,original); foreach (var path in paths) File.Delete(path); }
    }
    private static void QueryAndSelectionValidation(HostEnvironment host,HistoryPage page)
    {
        foreach (var query in new[] { new HistoryQuery(Limit:0), new HistoryQuery(Limit:17), new HistoryQuery(Offset:-1),
            new HistoryQuery(Scope:(HistorySearchScope)99),new HistoryQuery(StartTicks:10,EndTicks:10),new HistoryQuery(Text:"bad\0query") })
            Reject<ArgumentException>(() => host.ReadHistory(query));
        Reject<ArgumentException>(() => host.RestoreSelectedHistory(new(page.IndexRevision,[])));
        Reject<ArgumentException>(() => host.RestoreSelectedHistory(new(page.IndexRevision,Enumerable.Repeat(page.Items[0].Id,65).ToArray())));
        Reject<ArgumentException>(() => host.RestoreSelectedHistory(new(page.IndexRevision,[page.Items[0].Id,page.Items[0].Id])));
        Reject<XmlException>(() => host.RestoreSelectedHistory(new(page.IndexRevision,["../"+page.Items[0].Id])));
        Reject<InvalidOperationException>(() => host.RestoreSelectedHistory(new(new string('0',64),[page.Items[0].Id])));
        Task.Run(() => Reject<InvalidOperationException>(() => host.ReadHistory(new()))).GetAwaiter().GetResult();
    }
    private static void SelectedIdentity(string directory)
    {
        using var host = new HostEnvironment(directory);
        CombatFixture.ConfigureCategories();
        var page = host.ReadHistory(new()); var entry = page.Items[0];
        var selection = new HistorySelection(page.IndexRevision,[entry.Id]);
        Check(host.RestoreSelectedHistory(selection) == 1,"Selected restore added wrong count");
        Check(host.RestoreSelectedHistory(selection) == 0,"Repeated selected restore duplicated graph");
        var encounter = ActGlobals.oFormActMain!.ZoneList[0].Items.Single();
        Check(encounter.Title == entry.Title && encounter.ZoneName == entry.Zone,"Selected metadata resolved a different encounter");
        var action = encounter.GetCombatant("Source")!.AllOut["All"].Items.Single();
        Check(encounter.LogLines.Count == 2 && encounter.LogLines.All(line => line.Time == action.Time),"Same-time source lines were lost");
        Check(encounter.LogLines.Single(line => line.GlobalTimeSorter == action.TimeSorter).LogLine == "raw-"+(action.TimeSorter-42),"Private action/source identity changed");
        Check(host.ReadHistory(new()).Items.Single(item => item.Id == entry.Id).AlreadyRestored,"Restored state not reflected in metadata");
    }
    private static void AsyncLeaseAndOwnership(string directory)
    {
        using var context = new OwnerContext(_ => throw new Exception("Unexpected owner callback failure"));
        using var host = context.Run(() => new HostEnvironment(directory,() => context.Pump(),() => context.IsDispatching));
        CombatFixture.ConfigureCategories();
        var page = host.ReadHistory(new()); var original = page.Items[0]; var ids = new[] { original.Id };
        var operation = context.RunOperation(() => host.RestoreSelectedHistoryAsync(new(page.IndexRevision,ids)));
        ids[0] = page.Items[1].Id; // Caller changes must not redirect an already admitted selection.
        var path = Path.Combine(directory,"history/checkpoint.lock");
        bool Locked()
        {
            try { using var probe = new FileStream(path,FileMode.Open,FileAccess.ReadWrite,FileShare.None); return false; }
            catch (IOException) { return true; }
        }
        Check(SpinWait.SpinUntil(Locked,TimeSpan.FromSeconds(5)),"Async restore did not acquire checkpoint lease");
        Check(!operation.IsCompleted,"Restore completed without owner continuation dispatch");
        Reject<IOException>(() => { using var writer = new FileStream(path,FileMode.Open,FileAccess.ReadWrite,FileShare.None); });
        Check(SpinWait.SpinUntil(() => { context.Pump(); return operation.IsCompleted; },TimeSpan.FromSeconds(5)),"Restore owner continuation did not complete");
        Check(operation.GetAwaiter().GetResult() == 1 && ActGlobals.oFormActMain!.ZoneList[0].Items.Single().Title == original.Title,"Caller-owned selection redirected async restore");
        using var released = new FileStream(path,FileMode.Open,FileAccess.ReadWrite,FileShare.None);
    }
}
