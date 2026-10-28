using System.Text;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Testing;

internal static class EncounterLogChecks
{
    private static readonly DateTime Epoch = DateTime.UnixEpoch;
    private static void Check(bool condition, [System.Runtime.CompilerServices.CallerArgumentExpression(nameof(condition))] string? expression = null)
    { if (!condition) throw new Exception("Encounter log check failed: " + expression); }

    internal static void Run()
    {
        HostLifetimeChecks.RunBounded(ImportAndRestart);
        HostLifetimeChecks.RunBounded(SwitchesAndFailure);
        HostLifetimeChecks.RunBounded(Live);
        HostLifetimeChecks.RunBounded(Retention);
        HostLifetimeChecks.RunBounded(UnknownSourcesSurviveRestart);
        Console.WriteLine("Automatic encounter logs, ACT hook order, boundaries, bounded omissions and restart identity passed.");
    }

    private static void Add(FormActMain root, LogLineEventArgs info)
    {
        root.SetEncounter(info.detectedTime, "Source", "Target");
        root.AddCombatAction(new(1, false, "None", new Dnum(100), info.detectedTime,
            root.GlobalTimeSorter, "Strike", "Source", "Physical", "Target"));
    }

    private static void ImportAndRestart()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-recorded-import-");
        try
        {
            using (var host = new HostEnvironment(directory.FullName))
            {
                CombatFixture.ConfigureCategories();
                var root = ActGlobals.oFormActMain!;
                root.OriginalLoglineViewLogs = true;
                root.GetDateTimeFromLog = line => line switch { "later" => Epoch.AddSeconds(20), "idle" => Epoch.AddSeconds(28), _ => Epoch };
                root.BeforeLogLineRead += (_, info) =>
                {
                    if (info.originalLogLine is "first" or "same" or "later") Add(root, info);
                    if (info.originalLogLine == "end") root.EndCombat(false);
                    info.logLine = "processed " + info.originalLogLine;
                    info.detectedType = 9;
                };
                root.OnLogLineRead += (_, info) =>
                {
                    if (info.originalLogLine is "first" or "same" or "end" or "later" or "idle")
                        Check(root.ActiveZone.ActiveEncounter!.LogLines[^1].LogLine == info.originalLogLine);
                };
                var path = Path.Combine(directory.FullName, "source.log");
                File.WriteAllText(path, "outside\nfirst\nsame\nsame\nend\noutside\nlater\nidle\n", new UTF8Encoding(false));
                Check(host.ImportLog(path) is { Encounters: 2, Damage: 400 });
                CheckLogs(root.ZoneList[0].Items);
                CheckQueries(host);
                Check(host.ArchiveOperation(3, "", 0, false) == 2);
            }
            using (var restarted = new HostEnvironment(directory.FullName))
            {
                CombatFixture.ConfigureCategories();
                Check(restarted.ArchiveOperation(4, "", 0, false) == 2);
                CheckLogs(ActGlobals.oFormActMain!.ZoneList[0].Items);
                CheckQueries(restarted);
            }
        }
        finally { directory.Delete(true); }
    }

    private static void CheckLogs(List<EncounterData> encounters)
    {
        var ordered = encounters.OrderBy(encounter => encounter.StartTime).ToArray();
        Check(ordered.Length == 2);
        Check(ordered[0].LogLines.Select(line => line.LogLine).SequenceEqual(new[] { "first", "same", "same", "end" }));
        Check(ordered[1].LogLines.Select(line => line.LogLine).SequenceEqual(new[] { "later", "idle" }));
        foreach (var encounter in ordered)
        {
            Check(encounter.DroppedLogLines == 0 && encounter.LogLines.All(line => line.Type == 9));
            foreach (var action in encounter.GetCombatant("Source")!.AllOut["All"].Items)
            {
                var source = encounter.LogLines.Single(line => line.GlobalTimeSorter == action.TimeSorter);
                Check(source.Time == action.Time && source.LogLine is "first" or "same" or "later");
            }
        }
    }

    private static void CheckQueries(HostEnvironment host)
    {
        var id = host.ListEncounters(0, 16).Items.OrderBy(item => item.Start).First().Id;
        var actions = host.ReadEncounterActions(id, 0, 1)!;
        Check(actions.Total == 3 && actions.Items.Length == 1 && actions.Items[0].SourceIndex == 0);
        Check(host.ReadEncounterActions(id, 1, 2)!.Items.Select(action => action.SourceIndex).SequenceEqual(new[] { 1, 2 }));
        CombatantData.DamageSwingTypes.Add(1);
        CombatantData.HealingSwingTypes.Add(1);
        var snapshot = host.CaptureActionSnapshot(id)!;
        var first = host.ReadActionSnapshot(snapshot.Revision, 0, 1).Items.Single();
        Check(first.RegisteredDamage && first.RegisteredHealing && first.Action.SourceIndex == 0);
        CombatantData.DamageSwingTypes.Clear();
        CombatantData.HealingSwingTypes.Clear();
        var originalEncounter = ActGlobals.oFormActMain!.ZoneList[0].Items.OrderBy(item => item.StartTime).First();
        var allActions = originalEncounter.GetCombatant("Source")!.AllOut["All"].Items;
        var removedAction = allActions[1];
        allActions.RemoveAt(1);
        var removedLine = originalEncounter.LogLines[1];
        originalEncounter.LogLines.RemoveAt(1);
        var next = host.ReadActionSnapshot(snapshot.Revision, 1, 2);
        var frozenLogs = host.ReadActionSnapshotLogs(snapshot.Revision, 1, 1);
        Check(frozenLogs.Page.Total == 4 && frozenLogs.Page.Items.Single().Text == removedLine.LogLine);
        Check(frozenLogs.Page.Items.Single().SourceIndex == next.Items[0].Action.SourceIndex);
        Check(next.Total == 3 && next.Items.All(item => item.RegisteredDamage && item.RegisteredHealing));
        Check(next.Items[0].Action.SourceIndex == 1 && next.Items[0].Action.Amount == 100);
        originalEncounter.LogLines.Insert(1, removedLine);
        allActions.Insert(1, removedAction);
        var replacement = host.CaptureActionSnapshot(id)!;
        Check(!host.ReadActionSnapshot(replacement.Revision, 0, 1).Items.Single().RegisteredDamage);
        try { host.ReadActionSnapshot(snapshot.Revision, 0, 1); throw new Exception("Expected stale snapshot rejection"); }
        catch (InvalidOperationException) { }
        try { host.ReadActionSnapshotLogs(snapshot.Revision, 0, 1); throw new Exception("Expected stale log snapshot rejection"); }
        catch (InvalidOperationException) { }
        var originalCount = allActions.Count;
        allActions.AddRange(Enumerable.Repeat(removedAction, 16384 - originalCount));
        var atLimit = host.CaptureActionSnapshot(id)!;
        Check(atLimit.Total == 16384 && host.ReadActionSnapshot(atLimit.Revision, 16383, 1).Items.Length == 1);
        allActions.Add(removedAction);
        try { host.CaptureActionSnapshot(id); throw new Exception("Expected action snapshot limit"); }
        catch (InvalidDataException) { }
        Check(host.ReadActionSnapshot(atLimit.Revision, 16383, 1).Total == 16384);
        allActions.RemoveRange(originalCount, allActions.Count - originalCount);
        var logs = host.ReadEncounterLogs(id, 1, 2)!;
        Check(logs.Dropped == 0 && logs.Page.Total == 4 && logs.Page.Items.Select(line => line.Text).SequenceEqual(new[] { "same", "same" }));
        Check(logs.Page.Items[0].TimeSorter != logs.Page.Items[1].TimeSorter);
        Check(host.ReadEncounterLogs(ulong.MaxValue, 0, 1) is null && host.ReadEncounterActions(ulong.MaxValue, 0, 1) is null);
        Check(host.ReadEncounterLogs(id, 99, 1)!.Page.Items.Length == 0);
        var encounter = ActGlobals.oFormActMain!.ZoneList[0].Items.OrderBy(item => item.StartTime).First();
        var line = encounter.LogLines[0];
        encounter.LogLines.Add(line);
        Check(host.ReadEncounterActions(id, 0, 1)!.Items[0].SourceIndex == -2);
        // The duplicate is outside this one-row page, added after the initial action query above.
        Check(host.ReadEncounterLogs(id, 0, 1)!.Page.Items[0].SourceIndex == -2);
        encounter.LogLines.RemoveAt(encounter.LogLines.Count - 1);
        encounter.LogLines.RemoveAt(0);
        Check(host.ReadEncounterActions(id, 0, 1)!.Items[0].SourceIndex == -1);
        encounter.LogLines.Insert(0, line);
        using var xml = new MemoryStream();
        var root = ActGlobals.oFormActMain!;
        root.EncountersToXml([encounter], xml, true);
        var sorter = root.GlobalTimeSorter;
        root.GlobalTimeSorter = line.GlobalTimeSorter - 1;
        xml.Position = 0;
        var imported = root.XmlToEncounters(xml).Single();
        imported.Parent = root.ZoneList[0];
        root.ZoneList[0].Items.Add(imported);
        var importedId = host.ListEncounters(0, 16).Items.Last().Id;
        Check(imported.LogLines[0].GlobalTimeSorter == line.GlobalTimeSorter);
        Check(host.ReadEncounterActions(importedId, 0, 1)!.Items[0].SourceIndex == -1);
        root.ZoneList[0].Items.Remove(imported);
        root.GlobalTimeSorter = sorter;
    }

    private static void SwitchesAndFailure()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-recorded-failure-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            CombatFixture.ConfigureCategories();
            var root = ActGlobals.oFormActMain!;
            Check(root.RecordEncounterLogs && !root.OriginalLoglineViewLogs);
            root.GetDateTimeFromLog = _ => Epoch;
            root.BeforeLogLineRead += (_, info) =>
            {
                root.RecordEncounterLogs = info.originalLogLine != "off";
                Add(root, info);
                info.logLine = "processed " + info.originalLogLine;
                if (info.originalLogLine == "fail") throw new InvalidOperationException("Failure after parser admission");
            };
            var path = Path.Combine(directory.FullName, "source.log");
            File.WriteAllText(path, "off\nprocessed\nfail\n");
            try { host.ImportLog(path); throw new Exception("Failed parser reported success"); }
            catch (InvalidDataException) { }
            var encounter = root.ZoneList[0].Items.Single();
            Check(encounter.Damage == 300 && encounter.DroppedLogLines == 0);
            Check(encounter.LogLines.Select(line => line.LogLine).SequenceEqual(new[] { "processed processed", "processed fail" }));
            Check(host.ListEncounters(0, 16).Items.Single().Status == EncounterStatus.PartialImport);
        }
        finally { directory.Delete(true); }
    }

    private static void Live()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-recorded-live-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            CombatFixture.ConfigureCategories();
            var root = ActGlobals.oFormActMain!;
            root.OriginalLoglineViewLogs = true;
            root.GetDateTimeFromLog = _ => Epoch;
            var path = Path.Combine(directory.FullName, "live.log");
            File.WriteAllText(path, "old\n");
            root.LogFilePath = path;
            using var done = new ManualResetEventSlim();
            root.BeforeLogLineRead += (_, info) =>
            {
                if (info.originalLogLine == "first") Add(root, info);
                else if (info.originalLogLine == "end") root.EndCombat(false);
            };
            root.OnLogLineRead += (_, info) => { if (info.originalLogLine == "end") done.Set(); };
            root.OpenLog(false, false);
            File.AppendAllText(path, "outside\nfirst\nend\n");
            Check(done.Wait(TimeSpan.FromSeconds(5)));
            host.StopLogReading();
            var encounter = root.ActiveZone.ActiveEncounter!;
            Check(encounter.LogLines.Select(line => line.LogLine).SequenceEqual(new[] { "first", "end" }));
            Check(encounter.LogLines[0].GlobalTimeSorter == encounter.GetCombatant("Source")!.AllOut["All"].Items.Single().TimeSorter);
            Check(root.ActiveZone.Items[0].LogLines.Count == 0 && host.Diagnostics.Item1 == 0);
        }
        finally { directory.Delete(true); }
    }

    private static void UnknownSourcesSurviveRestart()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-unknown-sources-");
        try
        {
            using (var host = new HostEnvironment(directory.FullName))
            {
                CombatFixture.ConfigureCategories();
                var root = ActGlobals.oFormActMain!;
                root.GetDateTimeFromLog = _ => Epoch;
                root.BeforeLogLineRead += (_, info) => Add(root, info);
                var path = Path.Combine(directory.FullName, "source.log");
                File.WriteAllText(path, "first\n");
                host.ImportLog(path);
                var id = host.ListEncounters(0, 1).Items.Single().Id;
                var xml = Path.Combine(directory.FullName, "external.xml");
                host.ArchiveOperation(2, xml, id, false);
                root.ZoneList[0].Items.Clear();
                root.GlobalTimeSorter = 0;
                host.ArchiveOperation(1, xml, 0, false);
                id = host.ListEncounters(0, 1).Items.Single().Id;
                Check(host.ReadEncounterActions(id, 0, 1)!.Items.Single().SourceIndex == -1);
                Check(host.ArchiveOperation(3, "", 0, false) == 1);
            }
            using var restarted = new HostEnvironment(directory.FullName);
            CombatFixture.ConfigureCategories();
            Check(restarted.ArchiveOperation(4, "", 0, false) == 1);
            var restoredId = restarted.ListEncounters(0, 1).Items.Single().Id;
            Check(restarted.ReadEncounterActions(restoredId, 0, 1)!.Items.Single().SourceIndex == -1);
            Check(restarted.ReadEncounterLogs(restoredId, 0, 1)!.Page.Items.Single().Text == "first");
        }
        finally { directory.Delete(true); }
    }

    private static void Retention()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-recorded-bound-");
        try
        {
            using (var host = new HostEnvironment(directory.FullName))
            {
                CombatFixture.ConfigureCategories();
                var root = ActGlobals.oFormActMain!;
                root.GetDateTimeFromLog = _ => Epoch;
                root.BeforeLogLineRead += (_, info) => { if (info.originalLogLine == "start") Add(root, info); };
                var path = Path.Combine(directory.FullName, "source.log");
                // Non-action lines still belong to combat; an over-budget record must not hide a later small line.
                File.WriteAllText(path, "start\n" + string.Concat(Enumerable.Repeat(new string('x', 512 * 1024) + "\n", 8)) + "tail\n" + string.Concat(Enumerable.Repeat("x\n", 65537)));
                Check(host.ImportLog(path).Damage == 100);
                var encounter = root.ZoneList[0].Items.Single();
                Check(encounter.LogLines.Count == 65536 && encounter.DroppedLogLines == 11);
                Check(encounter.LogLines.Any(line => line.LogLine == "tail"));
                Check(encounter.LogLines.Count(line => line.LogLine.Length == 512 * 1024) == 7);
                var id = host.ListEncounters(0, 1).Items.Single().Id;
                var snapshot = host.CaptureActionSnapshot(id)!;
                var logs = host.ReadActionSnapshotLogs(snapshot.Revision, 0, 1);
                Check(logs.Dropped == 11 && logs.Page.Total == 65536 && logs.Page.Items.Single().Text == "start");
                encounter.LogLines.Add(encounter.LogLines[0]);
                try { host.CaptureActionSnapshot(id); throw new Exception("Expected log snapshot limit"); }
                catch (InvalidDataException) { }
                Check(host.ReadActionSnapshotLogs(snapshot.Revision, 0, 1).Dropped == 11);
                encounter.LogLines.RemoveAt(encounter.LogLines.Count - 1);
                Check(host.ArchiveOperation(3, "", 0, false) == 1);
            }
            using var restarted = new HostEnvironment(directory.FullName);
            CombatFixture.ConfigureCategories();
            Check(restarted.ArchiveOperation(4, "", 0, false) == 1);
            Check(ActGlobals.oFormActMain!.ZoneList[0].Items.Single().DroppedLogLines == 11);
        }
        finally { directory.Delete(true); }
    }
}
