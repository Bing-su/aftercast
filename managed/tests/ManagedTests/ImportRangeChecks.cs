using System.Globalization;
using System.Text;
using System.Text.RegularExpressions;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Testing;

internal static class ImportRangeChecks
{
    private static readonly DateTime Epoch = DateTime.UnixEpoch;
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Source range import contract failed");
    }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-range-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            CombatFixture.ConfigureCategories();
            root.LastKnownTime = Epoch.AddDays(1);
            root.ChangeZone("Live zone");
            root.LogFilePath = "live.log";
            root.ReadThreadLock = true;
            root.ZoneChangeRegex = new Regex("zone=(?<zone>.*)");
            root.GetDateTimeFromLog = line => line.StartsWith("undated", StringComparison.Ordinal)
                ? DateTime.MinValue : Epoch.AddSeconds(int.Parse(line.Split('|')[0], CultureInfo.InvariantCulture));
            var delivered = new List<string>();
            root.OnLogLineRead += (import, info) =>
            {
                Check(import);
                delivered.Add(info.originalLogLine);
                Check(root.CurrentZone == "Selected zone");
                root.SetEncounter(info.detectedTime, "Source", "Target");
                root.AddCombatAction(new(1, false, "None", new Dnum(100), info.detectedTime,
                    root.GlobalTimeSorter, "Strike", "Source", "Physical", "Target"));
            };
            var path = Path.Combine(directory.FullName, "source.log");
            // BOM, CR/LF, same-time rows and an unterminated suffix use the existing reader unchanged.
            File.WriteAllText(path, "0|zone=Selected zone\r\n1|first\r1|second\nundated\n\n2|end\n3|stop\n1|never", new UTF8Encoding(true));
            var range = new ImportRange(Epoch.AddSeconds(1), Epoch.AddSeconds(2));
            var result = host.ImportLog(path, range);
            Check(result is { LinesRead: 7, RecordsDispatched: 3, Encounters: 1, Damage: 300 });
            Check(delivered.SequenceEqual(new[] { "1|first", "1|second", "2|end" }));
            Check(root.CurrentZone == "Live zone" && root.LogFilePath == "live.log" && root.ReadThreadLock);
            Check(root.LastKnownTime == Epoch.AddDays(1) && !root.InCombat);
            delivered.Clear();
            var exact = host.ImportLog(path, new ImportRange(Epoch.AddSeconds(1), Epoch.AddSeconds(1)));
            Check(exact is { RecordsDispatched: 2, Damage: 200 });
            Check(host.ImportLog(path, new ImportRange(Epoch.AddSeconds(10), Epoch.AddSeconds(11))) is { RecordsDispatched: 0, Encounters: 0 });
            // A range ending on the final queued record must not observe its own producer cancellation as failure.
            File.WriteAllText(path, "0|zone=Selected zone\n1|first\n3|stop\n");
            for (var repeat = 0; repeat < 32; repeat++) Check(host.ImportLog(path, range).Damage == 100);
            File.WriteAllBytes(path, Encoding.UTF8.GetBytes("0|zone=Selected zone\n1|first\n3|stop\n").Concat(new byte[] { 255, 10 }).ToArray());
            Check(host.ImportLog(path, range).Damage == 100);
            FailureAndCancellation(host, root, path, range);
            try { _ = new ImportRange(Epoch.AddSeconds(1), Epoch); throw new Exception("reversed bounds accepted"); }
            catch (ArgumentException) { }
        }
        finally { directory.Delete(true); }
    }
    private static void FailureAndCancellation(HostEnvironment host, FormActMain root, string path, ImportRange range)
    {
        // Malformed bytes before the end fail visibly while retaining the admitted prefix.
        File.WriteAllBytes(path, Encoding.UTF8.GetBytes("0|zone=Selected zone\n1|first\n").Concat(new byte[] { 255, 10 }).ToArray());
        var before = root.ZoneList[0].Items.Count;
        try { host.ImportLog(path, range); throw new Exception("malformed source accepted"); }
        catch (InvalidDataException) { }
        Check(root.ZoneList[0].Items.Count == before + 1 && root.ZoneList[0].Items[^1].Damage == 100);
        Check(root.CurrentZone == "Live zone" && root.ReadThreadLock && !root.InCombat);
        File.WriteAllText(path, "0|zone=Selected zone\n" + string.Concat(Enumerable.Repeat("1|first\n", 5000)));
        host.BeginImport(path, range);
        ImportResult? result;
        do { result = host.AdvanceImport(false); if (result is null && host.ImportProgress.RecordsDispatched == 0) Thread.Sleep(1); }
        while (result is null && host.ImportProgress.RecordsDispatched == 0);
        Check(result is null);
        try
        {
            while (host.AdvanceImport(true) is null) Thread.Sleep(1);
            throw new Exception("cancel not reported");
        }
        catch (OperationCanceledException) { }
        Check(root.CurrentZone == "Live zone" && root.ReadThreadLock && !root.InCombat);
        // Completed/canceled state releases admission for the next import.
        File.WriteAllText(path, "0|zone=Selected zone\n1|first\n");
        Check(host.ImportLog(path, range).Damage == 100);
    }
}
