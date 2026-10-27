using System.Globalization;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using Aftercast.ActCompat;
using Advanced_Combat_Tracker;
using Aftercast.Testing;

internal static class HistorySourceCaptureChecks
{
    private static readonly DateTime Epoch = DateTime.UnixEpoch;
    private static void Check(bool value, string message) { if (!value) throw new Exception("Source capture: " + message); }
    private static void Reject<T>(Action action) where T : Exception
    { try { action(); } catch (T) { return; } throw new Exception("Expected " + typeof(T).Name); }
    private sealed class Fixture : IDisposable
    {
        internal readonly DirectoryInfo Directory = System.IO.Directory.CreateTempSubdirectory("aftercast-source-images-");
        internal readonly HostEnvironment Host;
        internal readonly FormActMain Root;
        internal string Main => Path.Combine(Directory.FullName, "main.log");
        internal string Side(string name) => Root.GetCompanionFilePath(Main, name);
        internal Fixture()
        {
            Host = new(Directory.FullName); Root = ActGlobals.oFormActMain!; CombatFixture.ConfigureCategories();
            Root.LogFilePath = Main;
            foreach (var second in new[] { 1, 3 })
            {
                Root.SetEncounter(Epoch.AddSeconds(second), "Source", "Target");
                Root.AddCombatAction(new(1, false, "", new Dnum(1), Epoch.AddSeconds(second), second, "Strike", "Source", "Physical", "Target"));
            }
            Host.EndCombat(); Root.ActiveZone.ActiveEncounter!.CharName = "Source"; Host.ArchiveOperation(3, "", 0, false);
            Root.GetDateTimeFromLog = line => Epoch.AddSeconds(int.Parse(line.Split('|')[0], CultureInfo.InvariantCulture));
            var owner = Environment.CurrentManagedThreadId;
            Root.BeforeLogLineRead += (import, info) =>
            {
                Check(import && Environment.CurrentManagedThreadId == owner, "parser left its owner/import path");
                Root.SetEncounter(info.detectedTime, "Source", "Target");
                Root.AddCombatAction(new(1, false, "", new Dnum(long.Parse(info.logLine.Split('|')[1], CultureInfo.InvariantCulture)),
                    info.detectedTime, Root.GlobalTimeSorter, "Strike", "Source", "Physical", "Target"));
            };
            System.IO.Directory.CreateDirectory(Path.Combine(Directory.FullName, "CompanionLogs"));
            Root.LogFilePath = "live.log"; ActGlobals.charName = "LiveHero";
        }
        internal HistorySourceSnapshot Proposal()
        {
            var page = Host.ReadHistory(new());
            return Host.ReadHistorySources(new(page.IndexRevision, [page.Items.Single().Id]), new([Main], []));
        }
        internal void NoScratch() => Check(!System.IO.Directory.EnumerateFiles(Directory.FullName, ".aftercast-source-*.tmp").Any(), "scratch name survived cleanup");
        public void Dispose() { try { Host.Dispose(); } finally { Directory.Delete(true); } }
    }
    internal static void Run()
    {
        HostLifetimeChecks.RunBounded(FrozenReplay);
        HostLifetimeChecks.RunBounded(BudgetAndAdmission);
        HostLifetimeChecks.RunBounded(IntegrityAndPartialResults);
        HostLifetimeChecks.RunBounded(CancellationAndStop);
        Console.WriteLine("Captured source bytes, range/companion replay, integrity, budget and ready shutdown checks passed.");
    }
    private static ImportResult Drain(HostEnvironment host)
    {
        var limit = DateTime.UtcNow.AddSeconds(10);
        while (true)
        {
            var before = host.ImportProgress;
            if (host.AdvanceImport(false) is { } result) return result;
            Check(DateTime.UtcNow < limit, "import did not finish");
            if (before == host.ImportProgress) Thread.Sleep(1);
        }
    }
    private static void FrozenReplay()
    {
        using var f = new Fixture();
        File.WriteAllText(f.Main, "0|999\n1|10|한글\n3|20\n4|999\n", new UTF8Encoding(false));
        File.WriteAllText(f.Side("alpha"), "1|21|한글\n3|22\n6|999\n", Encoding.GetEncoding(949));
        File.WriteAllText(f.Side("beta"), "2|30|日本\n", Encoding.Unicode);
        var original = File.ReadAllBytes(f.Main); var snapshot = f.Proposal();
        using var image = f.Host.CaptureHistorySourceAsync(snapshot, f.Main, companions: [new("beta"), new("alpha", new(949))]).GetAwaiter().GetResult();
        Check(image.Range.Start == Epoch.AddSeconds(1) && image.Range.End == Epoch.AddSeconds(3), "history inclusive range");
        Check(image.Files.Count == 3 && image.Files[0].Sha256 == Convert.ToHexStringLower(SHA256.HashData(original)), "captured digest/count");
        Check(File.ReadAllBytes(f.Main).SequenceEqual(original), "capture wrote original bytes");
        Reject<InvalidOperationException>(() => f.Host.ImportLog(f.Main));
        Reject<InvalidOperationException>(() => f.Host.ArchiveOperation(3, "", 0, false));
        File.Move(f.Main, f.Main + ".old"); File.WriteAllText(f.Main, "2|9999\n");
        File.Delete(f.Side("alpha")); File.Delete(f.Side("beta")); System.IO.Directory.Delete(Path.GetDirectoryName(f.Side("alpha"))!);
        f.Root.LogEncoding = Encoding.ASCII;
        var paths = new List<OriginalLogRecord>(); f.Host.OriginalLogRecordRead += paths.Add;
        var characters = new List<string?>(); f.Root.OnLogLineRead += (_, _) => characters.Add(ActGlobals.charName);
        f.Host.BeginCapturedHistoryImport(image); image.Dispose(); // Observer disposal cannot close transferred readers.
        var result = Drain(f.Host);
        Check(result.RecordsDispatched == 5 && result.Damage == 103 && result.CompanionHeadsExcluded == 1, $"frozen aggregate/range: {result}; records={string.Join(",", paths.Select(record => record.CompanionLogName + ":" + record.Line))}");
        Check(paths.All(record => record.Path == (record.CompanionLogName.Length == 0 ? f.Main : f.Side(record.CompanionLogName))), "scratch path leaked to plugin");
        Check(characters.All(name => name == "Source") && ActGlobals.charName == "LiveHero", "history character/live context restoration");
        Check(image.State == HistorySourceCaptureState.Consumed, "capture was not one shot");
        Reject<InvalidOperationException>(() => f.Host.BeginCapturedHistoryImport(image)); f.NoScratch();
    }
    private static void BudgetAndAdmission()
    {
        using var f = new Fixture(); File.WriteAllText(f.Main, "1|10\n"); var snapshot = f.Proposal();
        Reject<ArgumentOutOfRangeException>(() => f.Host.CaptureHistorySourceAsync(snapshot, f.Main, maximumBytes: 0));
        Reject<InvalidDataException>(() => f.Host.CaptureHistorySourceAsync(snapshot, f.Main, maximumBytes: 1).GetAwaiter().GetResult());
        File.AppendAllText(f.Main, "3|20\n");
        Reject<IOException>(() => f.Host.CaptureHistorySourceAsync(snapshot, f.Main).GetAwaiter().GetResult());
        snapshot = f.Proposal();
        File.WriteAllText(f.Side("alpha"), "1|10\n");
        Reject<FileNotFoundException>(() => f.Host.CaptureHistorySourceAsync(snapshot, f.Main, companions: [new("alpha"), new("zmissing")]).GetAwaiter().GetResult());
        using (var probe = new FileStream(f.Main, FileMode.Open, FileAccess.ReadWrite, FileShare.None)) { }
        using (var probe = new FileStream(f.Side("alpha"), FileMode.Open, FileAccess.ReadWrite, FileShare.None)) { }
        using var capture = f.Host.CaptureHistorySourceAsync(snapshot, f.Main).GetAwaiter().GetResult();
        var index = Path.Combine(f.Directory.FullName, "history/index.xml"); var saved = File.ReadAllBytes(index); File.AppendAllText(index, " ");
        try { Reject<InvalidOperationException>(() => f.Host.BeginCapturedHistoryImport(capture)); Check(capture.State == HistorySourceCaptureState.Ready, "stale index consumed capture"); }
        finally { File.WriteAllBytes(index, saved); }
        capture.Dispose(); Reject<InvalidOperationException>(() => f.Host.BeginCapturedHistoryImport(capture)); f.NoScratch();
    }
    private static void IntegrityAndPartialResults()
    {
        using var f = new Fixture(); File.WriteAllText(f.Main, string.Concat(Enumerable.Repeat("1|1|" + new string('x', 1024) + "\n", 1000)), new UTF8Encoding(false));
        using var capture = f.Host.CaptureHistorySourceAsync(f.Proposal(), f.Main).GetAwaiter().GetResult();
        // Corrupt only the private scratch descriptor; no test or product writes through a source handle.
        var sources = (Array)typeof(HistorySourceCapture).GetField("sources", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(capture)!;
        var source = sources.GetValue(0)!; var image = source.GetType().GetProperty("Image")!.GetValue(source)!;
        var file = (FileStream)image.GetType().GetField("file", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(image)!;
        var handle = file.SafeFileHandle; RandomAccess.Write(handle, [0xff], 65536);
        f.Host.BeginCapturedHistoryImport(capture);
        Reject<InvalidDataException>(() => Drain(f.Host));
        var damage = f.Host.ListEncounters(0, 16).Items.Last().Damage;
        Check(damage > 0 && damage < 1000, "integrity failure lost/over-published partial result");
        Check(handle.IsClosed, "scratch handle survived failed import"); f.NoScratch();
    }
    private static void CancellationAndStop()
    {
        using var f = new Fixture(); File.WriteAllText(f.Main, "1|10\n");
        using var cancel = new CancellationTokenSource(); cancel.Cancel();
        Reject<OperationCanceledException>(() => f.Host.CaptureHistorySourceAsync(f.Proposal(), f.Main, cancellation: cancel.Token).GetAwaiter().GetResult());
        var ready = f.Host.CaptureHistorySourceAsync(f.Proposal(), f.Main).GetAwaiter().GetResult();
        f.Host.StopProcessing(); Check(ready.State == HistorySourceCaptureState.Disposed, "stop retained ready bytes");
        Reject<ObjectDisposedException>(() => f.Host.CaptureHistorySourceAsync(f.Proposal(), f.Main)); f.NoScratch();
    }
}
