using System.Collections;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text.Json;

// Execute only the pinned original's data classes: no ACT form constructor or drawing.
internal static class DeathBuckets
{
    internal static int Run(Assembly act, string fixture)
    {
        using var json = JsonDocument.Parse(File.ReadAllText(fixture));
        var data = json.RootElement;
        var globals = act.GetType("Advanced_Combat_Tracker.ActGlobals", true)!;
        var root = act.GetType("Advanced_Combat_Tracker.FormActMain", true)!;
        var actor = act.GetType("Advanced_Combat_Tracker.CombatantData", true)!;
        var swing = act.GetType("Advanced_Combat_Tracker.MasterSwing", true)!;
        var dnum = act.GetType("Advanced_Combat_Tracker.Dnum", true)!;
        var death = act.GetType("Advanced_Combat_Tracker.FormResistsDeathReport+DeathData", true)!;
        var main = globals.GetField("oFormActMain")!;
        var damage = actor.GetField("DamageSwingTypes")!;
        var healing = actor.GetField("HealingSwingTypes")!;
        var previous = new[] { main.GetValue(null), damage.GetValue(null), healing.GetValue(null) };
        void Check(bool condition, string detail) { if (!condition) throw new InvalidDataException("Original death fixture: " + detail); }
        try
        {
            main.SetValue(null, RuntimeHelpers.GetUninitializedObject(root));
            damage.SetValue(null, data.GetProperty("damageTypes").EnumerateArray().Select(v => v.GetInt32()).ToList());
            healing.SetValue(null, data.GetProperty("healingTypes").EnumerateArray().Select(v => v.GetInt32()).ToList());
            var marker = dnum.GetProperty("Death", BindingFlags.Static | BindingFlags.Public)!.GetValue(null)!;
            Check(Activator.CreateInstance(dnum, new object?[] { -10L, null })!.Equals(marker), "null death label");
            Check(Activator.CreateInstance(dnum, new object[] { -10L, "Death" })!.Equals(marker), "explicit death label");
            Check(!Activator.CreateInstance(dnum, new object[] { -10L, "custom" })!.Equals(marker), "numeric sentinel alone is not death");
            var start = new DateTime(2026, 1, 1);
            foreach (var scenario in data.GetProperty("scenarios").EnumerateArray())
            {
                var list = (IList)Activator.CreateInstance(typeof(List<>).MakeGenericType(swing))!;
                foreach (var row in scenario.GetProperty("actions").EnumerateArray())
                    list.Add(Activator.CreateInstance(swing, new object[] { row.GetProperty("swingType").GetInt32(), false, "",
                        Activator.CreateInstance(dnum, long.Parse(row.GetProperty("amount").GetString()!))!,
                        start.AddTicks(row.GetProperty("offsetTicks").GetInt64()), row.GetProperty("sorter").GetInt32(),
                        "S" + row.GetProperty("sorter").GetInt32(), "Source", "Physical", "Target" })!);
                var comparison = swing.GetMethod("CompareTime", BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic)!;
                var delegateType = typeof(Comparison<>).MakeGenericType(swing);
                list.GetType().GetMethod("Sort", new[] { delegateType })!.Invoke(list, new[] { comparison.CreateDelegate(delegateType) });
                var report = Activator.CreateInstance(death, BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic,
                    null, new object[] { start.AddTicks(data.GetProperty("deathOffsetTicks").GetInt64()), list }, null)!;
                Check((DateTime)death.GetProperty("TimeOfDeath")!.GetValue(report)! == start, "truncated death");
                var buckets = (IDictionary)death.GetProperty("Items")!.GetValue(report)!;
                Check(buckets.Count == 10, "ten buckets");
                foreach (var expected in scenario.GetProperty("expected").EnumerateArray())
                {
                    var bucket = buckets[expected.GetProperty("index").GetInt32()]!;
                    long Value(string name) => (long)bucket.GetType().GetProperty(name)!.GetValue(bucket)!;
                    var d = long.Parse(expected.GetProperty("damage").GetString()!);
                    var h = long.Parse(expected.GetProperty("healing").GetString()!);
                    Check(Value("Damage") == d && Value("Healing") == h && Value("Result") == unchecked(d - h), scenario.GetProperty("name").GetString()!);
                    var text = bucket.ToString()!.Split('\n');
                    Check(text.Length - 2 == expected.GetProperty("tooltipRows").GetInt32(), "tooltip multiplicity");
                    var sorters = expected.GetProperty("tooltipSorters").EnumerateArray().Select(v => "Source - S" + v.GetInt32() + " - ").ToArray();
                    Check(text.Skip(1).Take(sorters.Length).Zip(sorters).All(pair => pair.First.StartsWith(pair.Second, StringComparison.Ordinal)), "tooltip ordering");
                    Check(text[0] == start.AddSeconds(1 - expected.GetProperty("index").GetInt32()).ToLongTimeString(), "bucket label uses exclusive end");
                }
                foreach (DictionaryEntry entry in buckets)
                    if (!scenario.GetProperty("expected").EnumerateArray().Any(v => v.GetProperty("index").GetInt32() == (int)entry.Key))
                    {
                        var bucket = entry.Value!;
                        Check((long)bucket.GetType().GetProperty("Damage")!.GetValue(bucket)! == 0 &&
                            (long)bucket.GetType().GetProperty("Healing")!.GetValue(bucket)! == 0, "empty buckets");
                    }
                Console.WriteLine(JsonSerializer.Serialize(new { originalDeathFixture = scenario.GetProperty("name").GetString(), passed = true }));
            }
            return 0;
        }
        finally { main.SetValue(null, previous[0]); damage.SetValue(null, previous[1]); healing.SetValue(null, previous[2]); }
    }
}
