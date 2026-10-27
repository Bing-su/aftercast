using System.Globalization;
using System.Text;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Testing;

internal static class ImportEncodingChecks
{
    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception("Import encoding check failed: " + message);
    }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-import-encoding-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            CombatFixture.ConfigureCategories();
            root.ChangeZone("live");
            root.LastKnownTime = DateTime.UnixEpoch.AddDays(1);
            root.GetDateTimeFromLog = line => DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0], CultureInfo.InvariantCulture));
            var received = new List<string>();
            root.OnLogLineRead += (_, info) =>
            {
                received.Add(info.originalLogLine);
                root.SetEncounter(info.detectedTime,"Source","Target");
                root.AddCombatAction(new(1,false,"None",new Dnum(100),info.detectedTime,
                    root.GlobalTimeSorter,"Strike","Source","Physical","Target"));
            };
            var path = Path.Combine(directory.FullName,"source.log");
            foreach (var (codePage, word) in new[] { (932,"日本語"), (949,"한글"), (1200,"한글日本語🙂"),
                (1201,"한글日本語🙂"), (12000,"한글日本語🙂"), (12001,"한글日本語🙂"), (65001,"한글日本語🙂") })
            {
                var encoding = Encoding.GetEncoding(codePage);
                var text = $"0|{word}\r\n1|{word}\r1|same\n2|{word}\n3|stop";
                File.WriteAllBytes(path,encoding.GetBytes(text));
                received.Clear();
                var result = host.ImportLog(path, text: new(codePage));
                Check(result.RecordsDispatched == 5 && result.Damage == 500 && received[1] == $"1|{word}", "full selected decoding");
                received.Clear();
                result = host.ImportLog(path, new(DateTime.UnixEpoch.AddSeconds(1),DateTime.UnixEpoch.AddSeconds(2)), new(codePage));
                Check(result.RecordsDispatched == 3 && result.Damage == 300 && received.SequenceEqual(new[] { $"1|{word}", "1|same", $"2|{word}" }), "inclusive range/same timestamp");
                Check(root.CurrentZone == "live" && root.LastKnownTime == DateTime.UnixEpoch.AddDays(1), "live context restoration");
                SnapshotAndCancel(host,root,path,codePage,word,received);
            }
            Failure(host,root,path,received);
        }
        finally { directory.Delete(true); }
        Console.WriteLine("Selected import encodings, inclusive ranges, snapshot policy, partial failure and cancellation passed.");
    }
    private static void SnapshotAndCancel(HostEnvironment host, FormActMain root, string path, int codePage, string word, List<string> received)
    {
        var encoding = (Encoding)Encoding.GetEncoding(codePage).Clone();
        encoding.DecoderFallback = DecoderFallback.ExceptionFallback;
        root.LogEncoding = encoding;
        File.WriteAllBytes(path,encoding.GetBytes($"0|{word}\n1|{word}\n"));
        host.BeginImport(path);
        // The admitted import owns a clone: later plugin changes must not alter the background reader.
        encoding.DecoderFallback = new DecoderReplacementFallback("changed");
        root.LogEncoding = Encoding.ASCII;
        ImportResult? result;
        do
        {
            result = host.AdvanceImport(false);
            if (result is null)
            {
                var progress = host.ImportProgress;
                Check(progress.BytesRead <= progress.TotalBytes && progress.RecordsDispatched <= progress.LinesRead, "encoded byte progress bounds");
                Thread.Sleep(1);
            }
        } while (result is null);
        Check(result.RecordsDispatched == 2 && received[^1] == $"1|{word}", "LogEncoding snapshot lost");
        File.WriteAllBytes(path,Encoding.GetEncoding(codePage).GetBytes(string.Concat(Enumerable.Repeat($"1|{word}\n", 10000))));
        received.Clear();
        host.BeginImport(path, text: new(codePage));
        do { result = host.AdvanceImport(false); if (result is null && host.ImportProgress.RecordsDispatched == 0) Thread.Sleep(1); }
        while (result is null && host.ImportProgress.RecordsDispatched == 0);
        Check(result is null && received.Count > 0, "cancellation must exercise admitted prefix");
        try
        {
            while (host.AdvanceImport(true) is null) Thread.Sleep(1);
            throw new Exception("cancel not reported");
        }
        catch (OperationCanceledException) { }
        Check(received.Count < 10000 && root.CurrentZone == "live" && !root.InCombat, "cancel cleanup/context");
        root.LogEncoding = new UTF8Encoding(false, true);
    }
    private static void Failure(HostEnvironment host, FormActMain root, string path, List<string> received)
    {
        foreach (var codePage in new[] { 65001, 1200, 12000 })
        {
            var encoding = Encoding.GetEncoding(codePage);
            var invalid = codePage == 65001 ? new byte[] { 0xff } : new byte[] { 0 };
            File.WriteAllBytes(path, encoding.GetBytes("1|valid\n").Concat(invalid).ToArray());
            received.Clear();
            var before = root.ZoneList[0].Items.Count;
            try { host.ImportLog(path,text: new(codePage)); throw new Exception("strict failure not reported"); }
            catch (InvalidDataException) { }
            Check(received.SequenceEqual(new[] { "1|valid" }), "malformed suffix discarded valid prefix");
            Check(root.ZoneList[0].Items.Count == before + 1 && root.ZoneList[0].Items[^1].Damage == 100, "failed decoder discarded admitted encounter actions");
            Check(root.CurrentZone == "live" && !root.InCombat, "failed decoder leaked context");
        }
        File.WriteAllBytes(path,new byte[] { 49,124,255,10 });
        received.Clear();
        Check(host.ImportLog(path,text: new(65001,true)).RecordsDispatched == 1 && received.Single() == "1|\uFFFD", "explicit ACT replacement policy");
        foreach (var replacement in new[] { "\n", "too long" })
        {
            var encoding = (Encoding)Encoding.UTF8.Clone();
            encoding.DecoderFallback = new DecoderReplacementFallback(replacement);
            root.LogEncoding = encoding;
            try { host.BeginImport(path); throw new Exception("line-injecting or unbounded replacement accepted"); }
            catch (ArgumentException) { }
            Check(root.CurrentZone == "live", "invalid fallback changed context");
        }
        root.LogEncoding = new UTF8Encoding(false,true);
        foreach (var unsupported in new[] { 0, 50220, -1, 123456 })
        {
            try { host.BeginImport(path,text: new(unsupported)); throw new Exception("unsupported encoding accepted"); }
            catch (ArgumentException) { }
            Check(root.CurrentZone == "live", "invalid option changed live context");
        }
    }
}
