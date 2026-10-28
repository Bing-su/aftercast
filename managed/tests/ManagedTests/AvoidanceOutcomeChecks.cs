using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using System.Text.Json;
using System.Globalization;

internal static class AvoidanceOutcomeChecks
{
    internal static void Run()
    {
        using var data = JsonDocument.Parse(File.ReadAllText("managed/tests/fixtures/avoidance/outcomes.json"));
        var root = data.RootElement;
        var previous = CultureInfo.CurrentCulture;
        try
        {
            CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo(root.GetProperty("culture").GetString()!);
            var types = root.GetProperty("registeredSwingTypes").EnumerateArray().Select(v => v.GetInt32()).ToArray();
            var actions = root.GetProperty("actions").EnumerateArray().Select((v, i) => new MasterSwing(
                v.GetProperty("swingType").GetInt32(), false, "", new Dnum(v.GetProperty("amount").GetInt64(), v.GetProperty("label").GetString()),
                DateTime.UnixEpoch, i, "Strike", "Source", "Physical", "Target")).ToArray();
            var actual = AvoidanceOutcomes.GroupCategory(actions, types);
            var expected = root.GetProperty("expected").EnumerateArray().ToArray();
            if (actual.Length != expected.Length) throw new Exception("Avoidance outcome group count");
            for (var i = 0; i < actual.Length; i++)
                if (actual[i].Label != expected[i].GetProperty("label").GetString() || actual[i].Amount != expected[i].GetProperty("amount").GetInt64() ||
                    !actual[i].Evidence.SequenceEqual(expected[i].GetProperty("evidence").EnumerateArray().Select(v => v.GetInt32())))
                    throw new Exception("Avoidance outcome grouping");
            try { AvoidanceOutcomes.GroupCategory(Enumerable.Repeat(actions[0], 16385).ToArray(), types); throw new Exception("Expected row limit"); }
            catch (InvalidDataException) { }
            var distinct = Enumerable.Range(0, 2001).Select(i => new MasterSwing(1, false, "", new Dnum(-1, "label" + i),
                DateTime.UnixEpoch, i, "Strike", "Source", "Physical", "Target")).ToArray();
            try { AvoidanceOutcomes.GroupCategory(distinct, types); throw new Exception("Expected comparison limit"); }
            catch (InvalidDataException) { }
        }
        finally { CultureInfo.CurrentCulture = previous; }
        HostLifetimeChecks.RunBounded(ProjectCategories);
        Console.WriteLine("Avoidance outcome grouping, sentinel identity and explicit bounds passed.");
    }
    private static void ProjectCategories()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-avoidance-category-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            Aftercast.Testing.CombatFixture.ConfigureCategories();
            CombatantData.DamageSwingTypes.Add(1);
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing[1] = new() { "Damage", "Damage" };
            var root = ActGlobals.oFormActMain!;
            root.GetDateTimeFromLog = _ => DateTime.UnixEpoch;
            root.BeforeLogLineRead += (_, info) =>
            {
                root.SetEncounter(info.detectedTime, "Source", "Target");
                root.AddCombatAction(new(1, false, "", new Dnum(long.Parse(info.logLine)), info.detectedTime,
                    root.GlobalTimeSorter, "Strike", "Source", "Physical", "Target"));
            };
            var path = Path.Combine(directory.FullName, "input.log");
            File.WriteAllText(path, "100\n-1\n-10\n");
            host.ImportLog(path);
            var actor = root.ZoneList[0].Items.Single().GetCombatant("Target")!;
            var categories = AvoidanceOutcomes.ProjectCategories(actor, "All");
            if (categories.Count != 1 || categories["Damage"].Count != 4) throw new Exception("Duplicate category routing");
            var outcome = AvoidanceOutcomes.GroupCategory(categories["Damage"], CombatantData.DamageSwingTypes).Single();
            if (!outcome.Evidence.SequenceEqual(new[] { 2, 3 })) throw new Exception("Duplicate category evidence");
            var encounter = root.ZoneList[0].Items.Single();
            var pool = encounter.GetCombatant("Source")!.AllOut["Strike"];
            var failed = actor.AllInc["Strike"].Items.Single(action => action.Damage.Number == -1);
            pool.AddCombatAction(new MasterSwing(2, false, "", new Dnum(900), DateTime.UnixEpoch,
                999, "Strike", "Source", "Physical", "Target"));
            var priorBlock = ActGlobals.blockIsHit;
            ActGlobals.blockIsHit = true;
            try
            {
                var estimate = AvoidanceEstimates.Calculate(encounter, failed, "All");
                if (Math.Abs(estimate.Estimate - 500.0) > 1e-9 || estimate.UsedFallback || !estimate.HasSampleAverage)
                    throw new Exception("Victim sample includes non-damage swing");
                if (!ActGlobals.blockIsHit) throw new Exception("Block setting not restored");
                var originalNonSkill = CombatantData.DamageTypeDataNonSkillDamage;
                CombatantData.DamageTypeDataNonSkillDamage = "Damage";
                var melee = AvoidanceEstimates.Calculate(encounter, failed, "All");
                if (melee.Estimate != 100 || melee.SamplePool != "Damage") throw new Exception("Non-skill All sample pool");
                CombatantData.DamageTypeDataNonSkillDamage = originalNonSkill;
                var victim = typeof(MasterSwing).GetProperty("Victim")!;
                victim.SetValue(failed, "Other");
                var fallback = AvoidanceEstimates.Calculate(encounter, failed, "All");
                if (Math.Abs(fallback.Estimate - 500.0) > 1e-9 || !fallback.UsedFallback || fallback.MatchingSamples.Length != 1)
                    throw new Exception("All-victim fallback");
                victim.SetValue(failed, "Target");
                var missing = typeof(MasterSwing).GetProperty("AttackType")!;
                missing.SetValue(failed, "Unknown");
                try { AvoidanceEstimates.Calculate(encounter, failed, "All"); throw new Exception("Expected missing sample pool"); }
                catch (InvalidDataException) { }
                if (!ActGlobals.blockIsHit) throw new Exception("Block setting not restored after failure");
                missing.SetValue(failed, "NoSample");
                var emptyPool = new AttackType("NoSample", pool.Parent);
                emptyPool.AddCombatAction(failed);
                encounter.GetCombatant("Source")!.AllOut.Add("NoSample", emptyPool);
                var noSample = AvoidanceEstimates.Calculate(encounter, failed, "All");
                if (noSample.Estimate != 0 || !noSample.UsedFallback || noSample.HasSampleAverage)
                    throw new Exception("Missing averages produce zero");
                encounter.GetCombatant("Source")!.AllOut.Remove("NoSample");
                missing.SetValue(failed, "Strike");
                var cached = new AttackType("Strike", null);
                cached.AddCombatAction(new MasterSwing(1, false, "", new Dnum(100), DateTime.UnixEpoch, 1, "Strike", "Source", "Physical", "Target"));
                cached.AddCombatAction(new MasterSwing(1, false, "", new Dnum(0), DateTime.UnixEpoch, 2, "Strike", "Source", "Physical", "Target"));
                if (cached.Hits != 2) throw new Exception("True-mode hits");
                ActGlobals.blockIsHit = false;
                if (cached.Average != 50) throw new Exception("Cached average preserved");
            }
            finally { ActGlobals.blockIsHit = priorBlock; }
            var id = host.ListEncounters(0, 1).Items.Single().Id;
            var snapshot = host.CaptureCombatantAvoidance(id, "Target")!;
            if (ActGlobals.blockIsHit != priorBlock) throw new Exception("Block setting not restored after capture");
            byte[] ReadSnapshot(AvoidanceSnapshotInfo value)
            {
                var result = new List<byte>();
                for (var offset = 0; offset < value.ByteLength; offset += 17)
                    result.AddRange(host.ReadCombatantAvoidance(value.Revision, offset, 17).Bytes);
                return result.ToArray();
            }
            var bytes = ReadSnapshot(snapshot);
            using (var json = JsonDocument.Parse(bytes))
            {
                var report = json.RootElement;
                var category = report.GetProperty("categories")[0];
                var outcomeJson = category.GetProperty("outcomes")[0];
                if (category.GetProperty("name").GetString() != "Damage" || outcomeJson.GetProperty("count").GetInt32() != 2 ||
                    Math.Abs(outcomeJson.GetProperty("estimatedDamage").GetDouble() - 1000) > 1e-9 || report.GetProperty("logs").GetArrayLength() != 3)
                    throw new Exception("Frozen avoidance report content");
                var firstEvidence = outcomeJson.GetProperty("evidence")[0];
                if (firstEvidence.GetProperty("action").GetProperty("SourceIndex").GetInt32() != 1)
                    throw new Exception("Frozen avoidance source identity");
                if (firstEvidence.GetProperty("samples").GetArrayLength() != 4 ||
                    !firstEvidence.GetProperty("samples").EnumerateArray().Any(sample => sample.GetProperty("SwingType").GetInt32() == 2))
                    throw new Exception("Frozen cross-swing sample evidence");
            }
            var retained = encounter.LogLines.ToArray();
            encounter.LogLines.Clear();
            if (!ReadSnapshot(snapshot).SequenceEqual(bytes))
                throw new Exception("Avoidance bytes changed after graph mutation");
            encounter.LogLines.AddRange(retained);
            var savedLinks = CombatantData.SwingTypeToDamageTypeDataLinksOutgoing[1];
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing.Remove(1);
            try { host.CaptureCombatantAvoidance(id, "Target"); throw new Exception("Expected missing mapping"); }
            catch (InvalidDataException) { }
            if (!ReadSnapshot(snapshot).SequenceEqual(bytes))
                throw new Exception("Failed capture replaced snapshot");
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing.Add(1, savedLinks);
            var logCount = encounter.LogLines.Count;
            var huge = new LogLineEntry(DateTime.UnixEpoch, new string('x', 1024 * 1024), 0, -12345);
            encounter.LogLines.AddRange(Enumerable.Repeat(huge, 65));
            try { host.CaptureCombatantAvoidance(id, "Target"); throw new Exception("Expected snapshot byte limit"); }
            catch (InvalidDataException) { }
            if (ActGlobals.blockIsHit != priorBlock || !ReadSnapshot(snapshot).SequenceEqual(bytes))
                throw new Exception("Failed bounded capture changed state");
            encounter.LogLines.RemoveRange(logCount, 65);
            var replacement = host.CaptureCombatantAvoidance(id, "Target")!;
            try { host.ReadCombatantAvoidance(snapshot.Revision, 0, 1); throw new Exception("Expected stale avoidance snapshot"); }
            catch (InvalidOperationException) { }
            if (host.ReadCombatantAvoidance(replacement.Revision, replacement.ByteLength, 1).Bytes.Length != 0)
                throw new Exception("Avoidance snapshot end page");
            var victimField = typeof(MasterSwing).GetProperty("Victim")!;
            victimField.SetValue(failed, "Other");
            var fallbackReport = host.CaptureCombatantAvoidance(id, "Target")!;
            using (var json = JsonDocument.Parse(ReadSnapshot(fallbackReport)))
            {
                var evidence = json.RootElement.GetProperty("categories")[0].GetProperty("outcomes")[0].GetProperty("evidence")[0];
                if (!evidence.GetProperty("UsedFallback").GetBoolean() || evidence.GetProperty("samples").GetArrayLength() != evidence.GetProperty("PoolCount").GetInt32())
                    throw new Exception("Fallback sample evidence");
            }
            victimField.SetValue(failed, "Target");
            CombatantData.DamageSwingTypes.Add(2);
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing.Add(2, new() { "Empty" });
            var emptyReport = host.CaptureCombatantAvoidance(id, "Target")!;
            using (var json = JsonDocument.Parse(ReadSnapshot(emptyReport)))
            {
                var empty = json.RootElement.GetProperty("categories").EnumerateArray().Single(value => value.GetProperty("name").GetString() == "Empty");
                if (empty.GetProperty("percentSwings").GetString() != "NaN") throw new Exception("Nonfinite percentage must be explicit");
            }
            CombatantData.DamageSwingTypes.Remove(2);
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing.Remove(2);
            var changed = actor.AllInc["Strike"].Items[0];
            var attackType = typeof(MasterSwing).GetProperty("AttackType")!;
            attackType.SetValue(changed, "All");
            if (AvoidanceOutcomes.ProjectCategories(actor, "All")["Damage"].Count != 6) throw new Exception("All key collision routing");
            attackType.SetValue(changed, "Strike");
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing[1].AddRange(new[] { "é", "e\u0301" });
            var unicode = AvoidanceOutcomes.ProjectCategories(actor, "All");
            if (unicode.Count != 3 || !unicode.ContainsKey("é") || !unicode.ContainsKey("e\u0301")) throw new Exception("Category names use ordinal equality");
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing[1].AddRange(Enumerable.Range(0, 1021).Select(i => "extra-" + i));
            if (AvoidanceOutcomes.ProjectCategories(actor, "All").Count != 1024) throw new Exception("Category limit boundary");
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing[1].Add("overflow");
            try { AvoidanceOutcomes.ProjectCategories(actor, "All"); throw new Exception("Expected category limit"); }
            catch (InvalidDataException) { }
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing[1].RemoveRange(4, 1022);
            CombatantData.DamageSwingTypes.Add(99);
            try { AvoidanceOutcomes.ProjectCategories(actor, "All"); throw new Exception("Expected missing mapping"); }
            catch (InvalidDataException) { }
        }
        finally { directory.Delete(true); }
    }

}
