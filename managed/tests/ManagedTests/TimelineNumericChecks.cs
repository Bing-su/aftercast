using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using System.Text.Json;

internal static class TimelineNumericChecks
{
    internal static void Run()
    {
        using var fixture = JsonDocument.Parse(File.ReadAllText("managed/tests/fixtures/timeline/bins.json"));
        foreach (var item in fixture.RootElement.GetProperty("cases").EnumerateArray())
        {
            var samples = item.GetProperty("samples").EnumerateArray().Select(row => new TimelineNumeric.Sample(row[0].GetDouble(), row[1].GetInt64()));
            var actual = TimelineNumeric.Calculate(item.GetProperty("duration").GetInt32(), item.GetProperty("window").GetInt32(), item.GetProperty("rolling").GetBoolean(), samples);
            if (!actual.DisplayedRates.SequenceEqual(item.GetProperty("displayed").EnumerateArray().Select(value => value.GetInt64())))
                throw new Exception($"Timeline bins: {item.GetProperty("name").GetString()}");
        }
        try { TimelineNumeric.Calculate(4, 10, false, [new(0, 1)]); throw new Exception("Expected ACT short encounter failure"); }
        catch (InvalidDataException) { }
        try { TimelineNumeric.Calculate(2, 1, true, Enumerable.Repeat(new TimelineNumeric.Sample(0, 1), 16385)); throw new Exception("Expected sample limit"); }
        catch (InvalidDataException) { }
        var overflow = TimelineNumeric.Calculate(2, 1, true, [new(1, long.MaxValue), new(1, 1)]);
        if (overflow.DisplayedRates[0] != long.MinValue) throw new Exception("ACT unchecked sum");
        var recovered = TimelineNumeric.Calculate(2, 1, true, [new(1, long.MaxValue), new(1, 1), new(1, 1)]);
        if (recovered.DisplayedRates[0] != 1) throw new Exception("ACT Dnum overflow recovery");
        HostLifetimeChecks.RunBounded(Snapshot);
        Console.WriteLine("ACT timeline positive bins, displayed indices, bounds and atomic snapshot passed.");
    }
    private static void Snapshot()
    {
        var dir = Directory.CreateTempSubdirectory("aftercast-timeline-");
        try
        {
            using var host = new HostEnvironment(dir.FullName);
            Aftercast.Testing.CombatFixture.ConfigureCategories();
            CombatantData.DamageSwingTypes.Add(1);
            CombatantData.DamageTypeDataNonSkillDamage = "Damage";
            var root = ActGlobals.oFormActMain!;
            root.GetDateTimeFromLog = line => DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0]));
            root.BeforeLogLineRead += (_, info) => {
                var fields = info.logLine.Split('|');
                root.SetEncounter(info.detectedTime, "Source", "Target");
                root.AddCombatAction(new MasterSwing(1, false, "", new Dnum(long.Parse(fields[1])), info.detectedTime,
                    root.GlobalTimeSorter, fields.Length > 2 ? fields[2] : "Strike", "Source", "Physical", "Target"));
            };
            var path = Path.Combine(dir.FullName, "input.log");
            File.WriteAllText(path, "0|100|Strike\n5|-1|Other\n10|200|Other\n15|-1|Strike\n20|-1|Other\n");
            ActGlobals.oFormSpellTimers!.TimerLookups.Add("Strike", new());
            host.ImportLog(path);
            var id = host.ListEncounters(0, 1).Items.Single().Id;
            _ = host.ReadEncounter(id, false);
            var snapshot = host.CaptureTimeline(id, "{\"Grouping\":0,\"Window\":10,\"Rolling\":true,\"Metrics\":3}")!;
            var bytes = host.ReadTimeline(snapshot.Revision, 0, 65536).Bytes;
            using var report = JsonDocument.Parse(bytes);
            if (report.RootElement.GetProperty("duration").GetInt32() != 20 || report.RootElement.GetProperty("points").GetInt32() != 20)
                throw new Exception($"Timeline duration {report.RootElement.GetProperty("duration")} points {report.RootElement.GetProperty("points")}");
            var rows = report.RootElement.GetProperty("rows").EnumerateArray().ToArray();
            var source = rows.Single(row => row.GetProperty("name").GetString() == "Source");
            var outgoing = source.GetProperty("series").EnumerateArray().Single(row => row.GetProperty("metric").GetInt32() == 0);
            if (outgoing.GetProperty("rates")[0].GetInt64() != 10 || outgoing.GetProperty("rates")[9].GetInt64() != 20)
                throw new Exception("Timeline category rates");
            if (source.GetProperty("markers")[14].GetInt32() != 4 || source.GetProperty("markers")[0].GetInt32() != 0)
                throw new Exception("Timeline timer marker spacing or first point");
            if (report.RootElement.GetProperty("logs").GetArrayLength() != 5) throw new Exception("Timeline logs");
            var encounterForTimer = root.ZoneList[0].Items.Single();
            var timerRows = encounterForTimer.GetCombatant("Source")!.AllOut["Strike"].Items;
            var exactTwelve = new MasterSwing(1, false, "", new Dnum(1), DateTime.UnixEpoch.AddSeconds(12), 2, "Strike", "Source", "Physical", "Target");
            timerRows.Add(exactTwelve);
            var exactCapture = host.CaptureTimeline(id, "{\"Grouping\":0,\"Window\":10,\"Rolling\":true,\"Metrics\":3}")!;
            using (var exactReport = JsonDocument.Parse(host.ReadTimeline(exactCapture.Revision, 0, 65536).Bytes))
            {
                var markers = exactReport.RootElement.GetProperty("rows").EnumerateArray().Single(row => row.GetProperty("name").GetString() == "Source").GetProperty("markers");
                if (markers[11].GetInt32() != 0 || markers[14].GetInt32() != 0) throw new Exception("Timer marker exact twelve seconds");
            }
            timerRows.Remove(exactTwelve);
            foreach (var grouping in new[] { 1, 2, 3 })
            {
                var groupSnapshot = host.CaptureTimeline(id, $"{{\"Grouping\":{grouping},\"Window\":10,\"Rolling\":true,\"Metrics\":15}}")!;
                using var grouped = JsonDocument.Parse(host.ReadTimeline(groupSnapshot.Revision, 0, 65536).Bytes);
                var names = grouped.RootElement.GetProperty("rows").EnumerateArray().Select(row => row.GetProperty("name").GetString()).ToArray();
                if (grouping == 1 && !new[] { "Tanks", "Healers", "Melee DPS", "Other DPS" }.All(names.Contains) ||
                    grouping == 2 && (!names.Contains("Us") || !names.Contains("Them")) || grouping == 3 && !names.Contains("Us"))
                    throw new Exception("Timeline grouping");
                if (grouped.RootElement.GetProperty("rows")[0].GetProperty("series").GetArrayLength() != 4)
                    throw new Exception("Timeline metric selection");
            }
            var markersOnly = host.CaptureTimeline(id, "{\"Grouping\":0,\"Window\":10,\"Rolling\":true,\"Metrics\":0}")!;
            using (var markerReport = JsonDocument.Parse(host.ReadTimeline(markersOnly.Revision, 0, 65536).Bytes))
                if (markerReport.RootElement.GetProperty("rows")[0].GetProperty("series").GetArrayLength() != 0) throw new Exception("Marker-only timeline");
            snapshot = host.CaptureTimeline(id, "{\"Grouping\":0,\"Window\":10,\"Rolling\":true,\"Metrics\":3}")!;
            bytes = host.ReadTimeline(snapshot.Revision, 0, 65536).Bytes;
            var encounter = root.ZoneList[0].Items.Single();
            encounter.GetCombatant("Source")!.AllOut["All"].Items.Clear();
            if (!bytes.SequenceEqual(host.ReadTimeline(snapshot.Revision, 0, 65536).Bytes)) throw new Exception("Timeline snapshot changed");
            try { host.CaptureTimeline(id, "{\"Grouping\":9,\"Window\":10,\"Rolling\":true,\"Metrics\":3}"); throw new Exception("Expected option rejection"); }
            catch (InvalidDataException) { }
            if (!bytes.SequenceEqual(host.ReadTimeline(snapshot.Revision, 0, 65536).Bytes)) throw new Exception("Failed capture replaced timeline");
            var replacement = host.CaptureTimeline(id, "{\"Grouping\":2,\"Window\":10,\"Rolling\":true,\"Metrics\":3}")!;
            try { host.ReadTimeline(snapshot.Revision, 0, 1); throw new Exception("Stale timeline accepted"); }
            catch (InvalidOperationException) { }
            if (replacement.Revision == snapshot.Revision) throw new Exception("Revision reused");
        }
        finally { dir.Delete(true); }
    }
}
