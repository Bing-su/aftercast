using System.Globalization;
using System.Text;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Testing;

internal static class CompanionImportChecks
{
    private static readonly DateTime Epoch = DateTime.UnixEpoch;
    private static void Check(bool value, string message)
    {
        if (!value) throw new Exception("Companion import: " + message);
    }
    private sealed class Fixture : IDisposable
    {
        internal readonly DirectoryInfo Directory = System.IO.Directory.CreateTempSubdirectory("aftercast-companion-import-");
        internal readonly HostEnvironment Host;
        internal readonly FormActMain Root;
        internal string Main => Path.Combine(Directory.FullName, "main.log");
        internal string Companion(string name) => Root.GetCompanionFilePath(Main, name);
        internal Fixture()
        {
            Host = new(Directory.FullName);
            Root = ActGlobals.oFormActMain!;
            var owner = Environment.CurrentManagedThreadId;
            CombatFixture.ConfigureCategories();
            Root.ChangeZone("Live zone"); Root.LogFilePath = "live.log"; Root.LastKnownTime = Epoch.AddDays(1);
            Root.GetDateTimeFromLog = line =>
            {
                Check(Environment.CurrentManagedThreadId == owner, "timestamp parser left owner thread");
                return line.StartsWith("undated", StringComparison.Ordinal) ? DateTime.MinValue : Epoch.AddSeconds(int.Parse(line.Split('|')[0], CultureInfo.InvariantCulture));
            };
            System.IO.Directory.CreateDirectory(Path.Combine(Directory.FullName, "CompanionLogs"));
        }
        public void Dispose() { try { Host.Dispose(); } finally { Directory.Delete(true); } }
    }
    internal static void Run()
    {
        HostLifetimeChecks.RunBounded(MergeAndOwnership);
        HostLifetimeChecks.RunBounded(RangeAndIncomplete);
        HostLifetimeChecks.RunBounded(ClockAndOutOfOrder);
        HostLifetimeChecks.RunBounded(FailureAndCancel);
        HostLifetimeChecks.RunBounded(TruncationAndShutdown);
        HostLifetimeChecks.RunBounded(AliasAndMemoryBounds);
        Console.WriteLine("Companion owner timestamp merge, encodings, source/name identity, ranges, EOF, file leases, failure and cancellation passed.");
    }
    private static void MergeAndOwnership()
    {
        using var fixture = new Fixture();
        File.WriteAllText(fixture.Main, "0|main\n3|main-tie 한글\n", new UTF8Encoding(false));
        File.WriteAllText(fixture.Companion("alpha"), "1|한글\n3|alpha-tie\n", Encoding.GetEncoding(949));
        File.WriteAllText(fixture.Companion("beta"), "2|日本\r\n3|beta-tie\r\n", Encoding.Unicode);
        var records = new List<OriginalLogRecord>();
        var delivered = new List<string>();
        fixture.Host.OriginalLogRecordRead += record =>
        {
            records.Add(record);
            if (records.Count != 1) return;
            // All source handles and lengths precede dispatch; new paths/appends cannot redirect the operation.
            File.Move(fixture.Companion("alpha"), fixture.Companion("alpha") + ".old");
            File.WriteAllText(fixture.Companion("alpha"), "1|wrong replacement\n", Encoding.UTF8);
            File.AppendAllText(fixture.Main, "4|late append\n", Encoding.UTF8);
            fixture.Root.LogEncoding = Encoding.ASCII;
        };
        fixture.Root.BeforeLogLineRead += (_, info) =>
        {
            info.logLine += " mutated";
            fixture.Root.SetEncounter(info.detectedTime, "Source", "Target");
            fixture.Root.AddCombatAction(new(1, false, "None", new Dnum(100), info.detectedTime,
                fixture.Root.GlobalTimeSorter, "Strike", "Source", "Physical", "Target"));
        };
        fixture.Root.OnLogLineRead += (import, info) =>
        {
            Check(import && info.logLine.EndsWith(" mutated", StringComparison.Ordinal), "raw hook order");
            delivered.Add(info.companionLogName + ":" + info.originalLogLine);
        };
        var result = fixture.Host.ImportLog(fixture.Main, companions: [new("beta"), new("alpha", new(949))]);
        Check(result.RecordsDispatched == 6 && result.Damage == 600, "merged aggregation totals");
        Check(delivered.SequenceEqual([":0|main", "alpha:1|한글", "beta:2|日本", ":3|main-tie 한글", "alpha:3|alpha-tie", "beta:3|beta-tie"]), "stable timestamp/tie order or source text");
        Check(records.Select(record => record.TimeSorter).Distinct().Count() == 6, "shared timestamp reused sorter");
        Check(records.All(record => record.Path == (record.CompanionLogName.Length == 0 ? fixture.Main : fixture.Companion(record.CompanionLogName))), "captured source path/name");
        Check(records.All(record => !record.Line.Contains("mutated", StringComparison.Ordinal)), "capture lost original bytes");
        var id = fixture.Host.ListEncounters(0, 16).Items.Single().Id;
        Check(fixture.Host.ReadEncounterActions(id, 0, 16)!.Items.Select(action => action.SourceIndex).SequenceEqual(Enumerable.Range(0, 6)), "same-time action/log identity");
        Check(fixture.Host.ArchiveOperation(3, "", 0, false) == 1, "companion encounter archive");
        Check(fixture.Root.CurrentZone == "Live zone" && fixture.Root.LogFilePath == "live.log" && fixture.Root.LastKnownTime == Epoch.AddDays(1), "live context restoration");
        fixture.Host.Dispose();
        using var restarted = new HostEnvironment(fixture.Directory.FullName);
        CombatFixture.ConfigureCategories();
        Check(restarted.ArchiveOperation(4, "", 0, false) == 1, "companion archive restoration");
        var restored = restarted.ListEncounters(0, 16).Items.Single().Id;
        Check(restarted.ReadEncounterActions(restored, 0, 16)!.Items.Select(action => action.SourceIndex).SequenceEqual(Enumerable.Range(0, 6)), "restored companion action/log identity");
    }
    private static void RangeAndIncomplete()
    {
        using var fixture = new Fixture();
        File.WriteAllText(fixture.Main, "0|prefix\n1|start\n3|stop\n1|never\n");
        File.WriteAllText(fixture.Companion("side"), "undated\n1|same\n2|end\n4|stop\n1|never\n");
        var seen = new List<string>();
        fixture.Root.OnLogLineRead += (_, info) => seen.Add(info.companionLogName + ":" + info.originalLogLine);
        var result = fixture.Host.ImportLog(fixture.Main, new(Epoch.AddSeconds(1), Epoch.AddSeconds(2)), companions: [new("side")]);
        Check(result.RecordsDispatched == 3 && seen.SequenceEqual([":1|start", "side:1|same", "side:2|end"]), "inclusive per-source boundaries");
        seen.Clear();
        File.WriteAllText(fixture.Main, "0|main-final", new UTF8Encoding(false));
        File.WriteAllBytes(fixture.Companion("side"), Encoding.UTF8.GetBytes("1|complete\n2|pending").Concat(new byte[] { 0xff }).ToArray());
        result = fixture.Host.ImportLog(fixture.Main, companions: [new("side")]);
        Check(result.RecordsDispatched == 2 && seen.SequenceEqual([":0|main-final", "side:1|complete"]), "companion incomplete EOF dispatch");
        Check(fixture.Host.RecentDiagnostics.Any(text => text.Contains("incomplete EOF", StringComparison.Ordinal)), "excluded companion tail was hidden");
        seen.Clear();
        File.WriteAllBytes(fixture.Companion("side"), Encoding.UTF8.GetBytes("1|").Concat(new byte[] { 0xff, 10 }).ToArray());
        Check(fixture.Host.ImportLog(fixture.Main, companions: [new("side", new(65001, true))]).RecordsDispatched == 2 && seen.Contains("side:1|\uFFFD"), "source-specific replacement policy");
    }
    private static void ClockAndOutOfOrder()
    {
        using var fixture = new Fixture();
        File.WriteAllText(fixture.Main, "2|main\n");
        File.WriteAllText(fixture.Companion("side"), "3|forward\n1|backward\nundated\n");
        var clocks = new List<DateTime>();
        var lines = new List<string>();
        fixture.Root.OnLogLineRead += (_, info) => { clocks.Add(fixture.Root.LastKnownTime); lines.Add(info.originalLogLine); };
        Check(fixture.Host.ImportLog(fixture.Main, companions: [new("side")]).RecordsDispatched == 4, "out-of-order count");
        Check(lines.SequenceEqual(["2|main", "3|forward", "1|backward", "undated"]), "physical companion order was resorted");
        Check(clocks.SequenceEqual(new[] { Epoch.AddSeconds(2), Epoch.AddSeconds(3), Epoch.AddSeconds(3), Epoch.AddSeconds(3) }), "companion rewound main clock");
    }
    private static void FailureAndCancel()
    {
        using var fixture = new Fixture();
        File.WriteAllText(fixture.Main, "0|main\n3|later\n");
        File.WriteAllBytes(fixture.Companion("side"), Encoding.UTF8.GetBytes("1|valid\n").Concat(new byte[] { 0xff, 10 }).ToArray());
        var seen = new List<string>();
        fixture.Root.BeforeLogLineRead += (_, info) =>
        {
            fixture.Root.SetEncounter(info.detectedTime, "Source", "Target");
            fixture.Root.AddCombatAction(new(1, false, "None", new Dnum(100), info.detectedTime,
                fixture.Root.GlobalTimeSorter, "Strike", "Source", "Physical", "Target"));
        };
        fixture.Root.OnLogLineRead += (_, info) => seen.Add(info.originalLogLine);
        try { fixture.Host.ImportLog(fixture.Main, companions: [new("side")]); throw new Exception("Invalid complete companion accepted"); }
        catch (InvalidDataException) { }
        Check(seen.SequenceEqual(["0|main", "1|valid"]) && fixture.Root.LogFilePath == "live.log" && fixture.Root.ZoneList[0].Items[^1].Damage == 200, "partial failure prefix/restoration");
        File.WriteAllText(fixture.Companion("side"), string.Concat(Enumerable.Repeat("1|cancel\n", 5000)));
        ImportCompanion[] selection = [new("side")];
        fixture.Host.BeginImport(fixture.Main, companions: selection);
        selection[0] = new("missing");
        while (fixture.Host.ImportProgress.RecordsDispatched == 0)
        {
            Check(fixture.Host.AdvanceImport(false) is null, "import ended before cancel");
            Thread.Sleep(1);
        }
        try { while (fixture.Host.AdvanceImport(true) is null) Thread.Sleep(1); throw new Exception("Cancel accepted as success"); }
        catch (OperationCanceledException) { }
        Check(fixture.Root.LogFilePath == "live.log", "cancel restoration");
        File.WriteAllText(fixture.Companion("side"), "1|reused\n");
        Check(fixture.Host.ImportLog(fixture.Main, companions: [new("side")]).RecordsDispatched == 3, "admission not reusable");
        try { fixture.Host.ImportLog(fixture.Main, companions: [new("missing")]); throw new Exception("Missing selection accepted"); }
        catch (FileNotFoundException) { }
        Check(fixture.Root.LogFilePath == "live.log", "missing file restoration");
        try { fixture.Host.BeginImport(fixture.Main, companions: [new("side"), new("SIDE")]); throw new Exception("Duplicate selection accepted"); }
        catch (ArgumentException) { }
    }
    private static void TruncationAndShutdown()
    {
        using var fixture = new Fixture();
        File.WriteAllText(fixture.Main, "0|main\n");
        File.WriteAllText(fixture.Companion("side"), string.Concat(Enumerable.Repeat("1|body\n", 5000)));
        LogLineEventDelegate truncate = (_, info) =>
        {
            if (info.originalLogLine != "0|main") return;
            using var stream = new FileStream(fixture.Companion("side"), FileMode.Open, FileAccess.Write, FileShare.ReadWrite | FileShare.Delete);
            stream.SetLength(0);
        };
        fixture.Root.OnLogLineRead += truncate;
        try { fixture.Host.ImportLog(fixture.Main, companions: [new("side")]); throw new Exception("Truncated companion accepted"); }
        catch (EndOfStreamException) { }
        fixture.Root.OnLogLineRead -= truncate;
        Check(fixture.Root.LogFilePath == "live.log", "truncation restoration");
        File.WriteAllText(fixture.Companion("side"), string.Concat(Enumerable.Repeat("1|shutdown\n", 5000)));
        fixture.Host.BeginImport(fixture.Main, companions: [new("side")]);
        while (fixture.Host.ImportProgress.RecordsDispatched == 0)
        {
            Check(fixture.Host.AdvanceImport(false) is null, "shutdown fixture completed");
            Thread.Sleep(1);
        }
        // The low-level Host deliberately rejects Stop during an import; its caller must cancel/drain first.
        try { fixture.Host.Dispose(); throw new Exception("Active-import Stop accepted"); }
        catch (InvalidOperationException) { }
        try { while (fixture.Host.AdvanceImport(true) is null) Thread.Sleep(1); }
        catch (OperationCanceledException) { }
        fixture.Host.Dispose();
        Check(ActGlobals.oFormActMain is null, "shutdown retained globals");
        // Returned disposal joins the input task; exclusive opens cannot depend on a future background cleanup.
        using var exclusive = new FileStream(fixture.Companion("side"), FileMode.Open, FileAccess.ReadWrite, FileShare.None);
    }
    [System.Runtime.InteropServices.DllImport("libc", EntryPoint = "link", SetLastError = true)]
    private static extern int HardLink(string source, string target);
    private static void AliasAndMemoryBounds()
    {
        using var fixture = new Fixture();
        File.WriteAllText(fixture.Main, "0|main\n");
        if (OperatingSystem.IsLinux())
        {
            Check(HardLink(fixture.Main, fixture.Companion("alias")) == 0, "hardlink fixture");
            try { fixture.Host.ImportLog(fixture.Main, companions: [new("alias")]); throw new Exception("Aliased source accepted"); }
            catch (InvalidDataException) { }
            File.Delete(fixture.Companion("alias"));
        }
        fixture.Root.GetDateTimeFromLog = _ => Epoch;
        var maximum = new string('x', 8 * 1024 * 1024) + "\n";
        File.WriteAllText(fixture.Main, maximum, new UTF8Encoding(false));
        var selected = Enumerable.Range(0, 4).Select(index => new ImportCompanion("s" + index)).ToArray();
        foreach (var source in selected) File.WriteAllText(fixture.Companion(source.Name), maximum, new UTF8Encoding(false));
        var dispatched = 0;
        fixture.Root.OnLogLineRead += (_, _) => dispatched++;
        try { fixture.Host.ImportLog(fixture.Main, companions: selected); throw new Exception("Excess head text accepted"); }
        catch (InvalidDataException error) { Check(error.Message.Contains("64 MiB", StringComparison.Ordinal), "head limit diagnostic"); }
        Check(dispatched == 0 && fixture.Root.LogFilePath == "live.log", "head limit caused partial unbounded dispatch or lost restoration");
    }

}
