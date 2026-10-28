using System.Globalization;
using System.Reflection;
using System.Text.Json;

// Execute original AttackType calculations; selection is the loop read from report IL.
internal static class AvoidanceEstimatesProbe
{
    internal static int Run(Assembly act, string path)
    {
        using var fixture = JsonDocument.Parse(File.ReadAllText(path));
        var data = fixture.RootElement;
        var globals = act.GetType("Advanced_Combat_Tracker.ActGlobals", true)!;
        var block = globals.GetField("blockIsHit")!;
        var previous = block.GetValue(null);
        var attack = act.GetType("Advanced_Combat_Tracker.AttackType", true)!;
        var swing = act.GetType("Advanced_Combat_Tracker.MasterSwing", true)!;
        var dnum = act.GetType("Advanced_Combat_Tracker.Dnum", true)!;
        object New() => Activator.CreateInstance(attack, new object?[] { "Strike", null })!;
        object Action(long amount, string victim, int type = 1, string label = "") => Activator.CreateInstance(swing,
            new object[] { type, false, "", Activator.CreateInstance(dnum, new object[] { amount, label })!, DateTime.UnixEpoch, 0, "Strike", "Source", "Physical", victim })!;
        void Add(object target, object value) => attack.GetMethod("AddCombatAction")!.Invoke(target, new[] { value });
        double Average(object target) => (double)attack.GetProperty("Average")!.GetValue(target)!;
        int Count(object target, string field) => (int)attack.GetProperty(field)!.GetValue(target)!;
        void Check(bool value, string field) { if (!value) throw new InvalidDataException("Original avoidance estimate: " + field); }
        try
        {
            block.SetValue(null, false);
            var all = New();
            var rows = data.GetProperty("samples").EnumerateArray().Select(row => (Victim: row.GetProperty("victim").GetString()!,
                Value: Action(row.GetProperty("amount").GetInt64(), row.GetProperty("victim").GetString()!, row.GetProperty("swing").GetInt32()))).ToArray();
            foreach (var row in rows) Add(all, row.Value);
            foreach (var target in data.GetProperty("targets").EnumerateArray())
            {
                var local = New();
                foreach (var row in rows) if (row.Victim == target.GetProperty("victim").GetString()) Add(local, row.Value);
                var localAverage = Average(local);
                var fallback = double.IsNaN(localAverage);
                var result = fallback ? Average(all) : localAverage;
                Check(Math.Abs(result - target.GetProperty("average").GetDouble()) < 1e-9, "victim/fallback average");
                Check(fallback == target.GetProperty("fallback").GetBoolean(), "fallback origin");
            }
            Check(double.IsNaN(Average(New())), "empty average is NaN");
            var denominator = New(); var values = data.GetProperty("denominator");
            var amounts = values.GetProperty("amounts").EnumerateArray().ToArray();
            var labels = values.GetProperty("labels").EnumerateArray().ToArray();
            for (var i = 0; i < amounts.Length; i++) Add(denominator, Action(amounts[i].GetInt64(), "Target", 1, labels[i].GetString()!));
            Check(Count(denominator,"Swings") == values.GetProperty("swings").GetInt32(), "swings");
            Check(Count(denominator,"Hits") == values.GetProperty("hits").GetInt32(), "hits with block false");
            Check(Count(denominator,"Swings") - Count(denominator,"Hits") == values.GetProperty("nonHits").GetInt32(), "non-hit denominator");
            block.SetValue(null,true);
            var cached = New(); Add(cached, Action(100,"Target")); Add(cached, Action(0,"Target"));
            Check(Count(cached,"Hits") == 2, "pre-cached hits");
            block.SetValue(null,false);
            Check(Average(cached) == data.GetProperty("cachedAverage").GetDouble(), "same-count cache retained");
            var fresh = New(); Add(fresh,Action(100,"Target")); Add(fresh,Action(0,"Target"));
            Check(Average(fresh) == data.GetProperty("freshAverage").GetDouble(), "fresh hits follow false");
            Console.WriteLine(JsonSerializer.Serialize(new { originalAvoidanceEstimates = "passed", sampleSwing2Included = true, staleHitCacheObserved = true }));
            return 0;
        }
        finally { block.SetValue(null, previous); }
    }
}
