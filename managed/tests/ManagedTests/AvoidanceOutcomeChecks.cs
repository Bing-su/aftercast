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
        Console.WriteLine("Avoidance outcome grouping, sentinel identity and explicit bounds passed.");
    }
}
