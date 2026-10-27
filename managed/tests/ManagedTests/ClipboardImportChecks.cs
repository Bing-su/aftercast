using System.Globalization;
using System.Reflection;
using System.Text;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Testing;

internal static class ClipboardImportChecks
{
    private static void Check(bool condition, string message) { if (!condition) throw new Exception("Clipboard import: " + message); }
    private static void Throws<T>(Action action) where T : Exception
    { try { action(); } catch (T) { return; } throw new Exception("Expected " + typeof(T).Name); }
    internal static void Run() => HostLifetimeChecks.RunBounded(Scenarios);
    private static void Configure(FormActMain root)
    {
        CombatFixture.ConfigureCategories(); root.LogFilePath = "/synthetic/live.log"; root.ChangeZone("Live zone");
        root.LogEncoding = Encoding.ASCII; root.LastKnownTime = DateTime.UnixEpoch.AddDays(1);
        root.GetDateTimeFromLog = line => line.StartsWith("U|", StringComparison.Ordinal) ? DateTime.MinValue
            : DateTime.UnixEpoch.AddSeconds(int.Parse(line.TrimStart('\ufeff').Split('|')[0], CultureInfo.InvariantCulture));

        root.BeforeLogLineRead += (import, info) => {
            Check(import && root.LogFilePath == "", "pathless import hooks");
            var fields = info.logLine.Split('|');
            root.SetEncounter(info.detectedTime, "Source", "Target");
            root.AddCombatAction(new(1, false, "", new Dnum(long.Parse(fields[1], CultureInfo.InvariantCulture)), info.detectedTime, root.GlobalTimeSorter, "Strike", "Source", "Physical", "Target"));
        };
    }
    private static ImportResult Read(HostEnvironment host, string text, out ImportProgress progress)
    {
        host.BeginClipboardImport(text); return Finish(host, out progress);
    }
    private static ImportResult Finish(HostEnvironment host, out ImportProgress progress)
    {
        var deadline = DateTime.UtcNow.AddSeconds(10); progress = host.ImportProgress;
        while (true)
        {
            progress = host.ImportProgress;
            if (host.AdvanceImport(false) is { } result) return result;
            Check(DateTime.UtcNow < deadline, "bounded completion"); Thread.Sleep(1);
        }
    }
    private static void Restored(FormActMain root) => Check(root.LogFilePath == "/synthetic/live.log" && root.CurrentZone == "Live zone" &&
        root.LastKnownTime == DateTime.UnixEpoch.AddDays(1) && !root.InCombat, "live context restoration");
    private static void Scenarios()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-clipboard-");
        try
        {
            using (var host = new HostEnvironment(directory.FullName))
            {
                var root = ActGlobals.oFormActMain!; var seen = new List<OriginalLogRecord>(); Configure(root); host.OriginalLogRecordRead += seen.Add;
                const string text = "\ufeff1|10|한글\r\n\rU|20|日本語\n2|30|emoji😀";
                var result = Read(host, text, out var progress);
                Check(result is { RecordsDispatched: 3, LinesRead: 4, Damage: 60 }, "Unicode/newline/final unterminated rows");
                Check(progress.TotalBytes == Encoding.UTF8.GetByteCount(text) && progress.BytesRead == progress.TotalBytes, "UTF-8 equivalent progress");
                Check(seen.Select(line => line.Line).SequenceEqual(["\ufeff1|10|한글", "U|20|日本語", "2|30|emoji😀"]), "no byte transcoding/BOM stripping");
                Check(seen.All(line => line.Path == "" && line.IsImport && line.CompanionLogName == ""), "no live or fake source path");
                Check(seen[1].Time == DateTime.UnixEpoch.AddSeconds(1), "undated line inherits last timestamp");
                var encounter = root.ZoneList[0].Items.Single();
                Check(encounter.HistoryRecord!.FolderHint == "", "pathless history hint"); Restored(root);
                Check(host.ArchiveOperation(3, "", 0, false) == 1, "save private history");
                var archive = Directory.GetFiles(Path.Combine(directory.FullName, "history"), "*.act").Single();
                var xml = Encoding.UTF8.GetString(root.MemDecompressGzip(File.ReadAllBytes(archive)));
                Check(xml.Contains("AftercastClipboardImport=\"True\"", StringComparison.Ordinal), "private source marker");
                var actions = host.ReadEncounterActions(host.ListEncounters(0, 10).Items.Single().Id, 0, 10)!.Items;
                Check(actions.Length == 3 && actions.All(action => action.SourceIndex >= 0), "source/action identity retained");
                foreach (var undated in new[] { "U|20", "U|20\r1|5" })
                {
                    seen.Clear(); Read(host, undated, out _);
                    Check(seen[0].Time == DateTime.UnixEpoch.AddDays(1), "first undated row preserves pre-import LastKnownTime");
                    Restored(root);
                }
                AdmissionAndFailures(host, root);
                Check(Read(host, new string('\n', 1000) + "1|7", out _).Damage == 7, "1000 empty rows allowed");
                Check(Read(host, new string('\n', 1001) + "1|999", out _).RecordsDispatched == 0, "ACT consecutive-empty stop");
                Check(Read(host, "\r\n\r\n", out _).RecordsDispatched == 0, "empty physical lines are not dispatched");
                Restored(root);
            }
            using (var restarted = new HostEnvironment(directory.FullName))
            {
                CombatFixture.ConfigureCategories(); Check(restarted.ArchiveOperation(4, "", 0, false) == 1, "restore only saved encounter");
                var root = ActGlobals.oFormActMain!; var encounter = root.ZoneList[0].Items.Single();
                Check(encounter.Damage == 60 && encounter.LogLines.Count == 3 && encounter.HistoryRecord!.FolderHint == "", "restored pathless graph/logs");
                Check((bool)typeof(EncounterData).GetProperty("ClipboardImport", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(encounter)!, "restored source marker");
                Check(restarted.ReadEncounterActions(restarted.ListEncounters(0, 10).Items.Single().Id, 0, 10)!.Items.All(action => action.SourceIndex >= 0), "restored action/log links");
            }
            Console.WriteLine("Clipboard Unicode snapshot, CR/LF framing, ACT empty threshold, budgets, cancel/failure and pathless private history checks passed.");
        }
        finally { directory.Delete(true); }
    }
    private static void AdmissionAndFailures(HostEnvironment host, FormActMain root)
    {
        Throws<ArgumentNullException>(() => host.BeginClipboardImport(null!));
        Throws<ArgumentException>(() => host.BeginClipboardImport(""));
        Throws<EncoderFallbackException>(() => host.BeginClipboardImport("\ud800"));
        Throws<ArgumentException>(() => host.BeginClipboardImport(new string('x', 8 * 1024 * 1024 + 1)));
        Throws<ArgumentException>(() => host.BeginClipboardImport(new string('한', 8 * 1024 * 1024 / 3 + 1)));
        Restored(root);
        foreach (var text in new[] { new string('x', 8 * 1024 * 1024), new string('한', 8 * 1024 * 1024 / 3) + "xx" })
        {
            host.BeginClipboardImport(text); Throws<InvalidOperationException>(() => host.BeginClipboardImport("1|999"));
            Throws<OperationCanceledException>(() => { while (host.AdvanceImport(true) is null) Thread.Sleep(1); }); Restored(root);
        }
        Throws<FormatException>(() => Read(host, "1|5\nbad timestamp", out _));
        Check(host.ListEncounters(0, 100).Items.Any(item => item.Status == EncounterStatus.PartialImport && item.Damage == 5), "partial parse failure"); Restored(root);
        var content = string.Concat(Enumerable.Repeat("1|1\n", 10_000)); host.BeginClipboardImport(content); content = "1|999";
        var deadline = DateTime.UtcNow.AddSeconds(5);
        while (host.ImportProgress.RecordsDispatched == 0) { Check(host.AdvanceImport(false) is null && DateTime.UtcNow < deadline, "cancel progress"); Thread.Sleep(1); }
        Check(host.ImportProgress.TotalBytes == 40_000, "admitted snapshot unaffected by caller reassignment");
        Throws<OperationCanceledException>(() => { while (host.AdvanceImport(true) is null) Thread.Sleep(1); }); Restored(root);
        Check(Read(host, "1|9", out _).Damage == 9, "reuse after failure/cancel");
    }
}
