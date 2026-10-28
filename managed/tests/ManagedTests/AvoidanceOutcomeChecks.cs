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
