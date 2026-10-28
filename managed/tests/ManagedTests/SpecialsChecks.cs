using Advanced_Combat_Tracker;
using System.Text.Json;

internal static class SpecialsChecks
{
    internal static void Run()
    {
        using var fixture = JsonDocument.Parse(File.ReadAllText("managed/tests/fixtures/specials/counts.json"));
        foreach (var item in fixture.RootElement.GetProperty("cases").EnumerateArray())
        {
            var attack = new AttackType("Strike", null);
            foreach (var row in item.GetProperty("rows").EnumerateArray())
                attack.AddCombatAction(new MasterSwing(1, false, row[1].GetString()!, new Dnum(100), DateTime.UnixEpoch,
                    row[0].GetInt32(), "Strike", "Source", "Physical", "Target"));
            var actual = attack.GetAttackSpecials();
            var expected = item.GetProperty("expected");
            if (actual.Count != expected.EnumerateObject().Count() || expected.EnumerateObject().Any(e => !actual.TryGetValue(e.Name, out var count) || count != e.Value.GetInt32()))
                throw new Exception($"Special counts differ: {item.GetProperty("name").GetString()}");
        }
        HostLifetimeChecks.RunBounded(Snapshot);
        Console.WriteLine("Original special-count and bounded snapshot fixtures passed.");
    }
    private static void Snapshot()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-specials-");
        try
        {
            using var host = new Aftercast.ActCompat.HostEnvironment(directory.FullName);
            Aftercast.Testing.CombatFixture.ConfigureCategories();
            CombatantData.DamageSwingTypes.Add(1);
            var root = ActGlobals.oFormActMain!;
            root.GetDateTimeFromLog = line => DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0]));
            root.BeforeLogLineRead += (_, info) => {
                var fields = info.logLine.Split('|');
                root.SetEncounter(info.detectedTime, "Source", "Target");
                root.AddCombatAction(new MasterSwing(1, false, fields[1], new Dnum(100), info.detectedTime,
                    root.GlobalTimeSorter, "Strike", "Source", "Physical", "Target"));
            };
            var path = Path.Combine(directory.FullName, "input.log");
            File.WriteAllText(path, "0|None\n1|Crit\n2|Crit\n3|None\n4|Flurry\n");
            host.ImportLog(path);
            var encounter = root.ZoneList[0].Items.Single();
            var id = host.ListEncounters(0, 10).Items.Single().Id;
            var snapshot = host.CaptureSpecials(id, "Source") ?? throw new Exception("Missing snapshot");
            var bytes = host.ReadSpecials(snapshot.Revision, 0, 65536).Bytes;
            using var report = JsonDocument.Parse(bytes);
            var rows = report.RootElement.GetProperty("rows").EnumerateArray().ToArray();
            var pair = rows.Single(row => row.GetProperty("key").GetString() == "Strike(Crit)");
            if (pair.GetProperty("occurrences").GetInt32() != 1 || pair.GetProperty("swings").GetInt32() != 2 || pair.GetProperty("totalSwings").GetInt32() != 5 || pair.GetProperty("occurrenceRatio").GetDouble() != 0.5)
                throw new Exception("Incorrect pair report");
            var skill = rows.Single(row => row.GetProperty("group").GetString() == "skill");
            if (skill.GetProperty("occurrences").GetInt32() != 2 || skill.GetProperty("swings").GetInt32() != 3 || skill.GetProperty("occurrenceRatio").GetDouble() != 1)
                throw new Exception("Incorrect skill report");
            if (report.RootElement.GetProperty("logs").GetArrayLength() != 5 || pair.GetProperty("evidence")[0].GetProperty("SourceIndex").GetInt32() != 1)
                throw new Exception("Incorrect source identity");
            var all = encounter.GetCombatant("Source")!.AllOut["All"].Items;
            var original = all.ToArray();
            var specialProperty = typeof(MasterSwing).GetProperty("Special")!;
            specialProperty.SetValue(all[1], "nested(value)");
            try { host.CaptureSpecials(id, "Source"); throw new Exception("Expected ACT ambiguous pair failure"); }
            catch (KeyNotFoundException) { }
            specialProperty.SetValue(all[1], "Crit");
            if (!bytes.SequenceEqual(host.ReadSpecials(snapshot.Revision, 0, 65536).Bytes)) throw new Exception("Failed capture replaced snapshot");
            all.Clear(); all.AddRange(Enumerable.Repeat(original[1], 2001));
            try { host.CaptureSpecials(id, "Source"); throw new Exception("Expected work budget failure"); }
            catch (InvalidDataException) { }
            all.Clear(); all.Add(original[1]);
            var noNone = host.CaptureSpecials(id, "Source")!;
            var noNoneBytes = host.ReadSpecials(noNone.Revision, 0, 65536).Bytes;
            using (var noNoneJson = JsonDocument.Parse(noNoneBytes))
                if (noNoneJson.RootElement.GetProperty("rows")[0].GetProperty("occurrenceRatio").GetString() != "Infinity")
                    throw new Exception("Expected nonfinite original ratio");
            all.Clear();
            if (!noNoneBytes.SequenceEqual(host.ReadSpecials(noNone.Revision, 0, 65536).Bytes)) throw new Exception("Snapshot changed");
            var replacement = host.CaptureSpecials(id, "Source")!;
            try { host.ReadSpecials(noNone.Revision, 0, 1); throw new Exception("Old revision accepted"); }
            catch (InvalidOperationException) { }
            if (replacement.Revision == snapshot.Revision) throw new Exception("Revision reused");
        }
        finally { directory.Delete(true); }
    }
}
