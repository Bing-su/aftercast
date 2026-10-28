using System.Collections;
using System.Globalization;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text.Json;

// Invoke only ACT's category data and outcome methods, without constructing the report form.
internal static class AvoidanceOutcomesProbe
{
    internal static int Run(Assembly act, string path)
    {
        using var fixture = JsonDocument.Parse(File.ReadAllText(path));
        var root = fixture.RootElement;
        var previous = CultureInfo.CurrentCulture;
        try
        {
            CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo(root.GetProperty("culture").GetString()!);
            var formType = act.GetType("Advanced_Combat_Tracker.FormAvoidanceReport", true)!;
            var form = RuntimeHelpers.GetUninitializedObject(formType);
            var trans = formType.GetField("Trans", BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic)!;
            trans.SetValue(form, Activator.CreateInstance(trans.FieldType, true));
            var categoryType = act.GetType("Advanced_Combat_Tracker.DamageTypeData", true)!;
            var category = Activator.CreateInstance(categoryType, new object?[] { false, "fixture", null })!;
            var swingType = act.GetType("Advanced_Combat_Tracker.MasterSwing", true)!;
            var dnum = act.GetType("Advanced_Combat_Tracker.Dnum", true)!;
            var marker = dnum.GetProperty("Death")!.GetValue(null)!;
            var types = root.GetProperty("registeredSwingTypes").EnumerateArray().Select(v => v.GetInt32()).ToHashSet();
            var added = new List<(int Index, object Action)>();
            var index = 0;
            foreach (var row in root.GetProperty("actions").EnumerateArray())
            {
                var value = Activator.CreateInstance(dnum, new object?[] { row.GetProperty("amount").GetInt64(), row.GetProperty("label").GetString() })!;
                var swing = row.GetProperty("swingType").GetInt32();
                var action = Activator.CreateInstance(swingType, new object[] { swing, false, "", value,
                    DateTime.UnixEpoch, index, "Strike", "Source", "Physical", "Target" })!;
                if (types.Contains(swing) && !value.Equals(marker))
                {
                    categoryType.GetMethod("AddCombatAction")!.Invoke(category, new[] { action, "All" });
                    added.Add((index, action));
                }
                index++;
            }
            var flags = BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public;
            var groups = (IDictionary)formType.GetMethod("GetUniqueAvoidances", flags)!.Invoke(form, new[] { category })!;
            var getOccurrences = formType.GetMethod("GetDnumOccurances", flags)!;
            var expected = root.GetProperty("expected").EnumerateArray().ToArray();
            if (groups.Count != expected.Length) throw new InvalidDataException("Original avoidance group count differs");
            var position = 0;
            foreach (DictionaryEntry group in groups)
            {
                var sample = group.Value!;
                var occurrences = (IList)getOccurrences.Invoke(form, new[] { sample, category })!;
                var indices = added.Where(item => occurrences.Cast<object>().Any(value => ReferenceEquals(value, item.Action))).Select(item => item.Index).ToArray();
                var want = expected[position++];
                if ((string)group.Key != want.GetProperty("label").GetString() ||
                    (long)dnum.GetProperty("Number")!.GetValue(sample)! != want.GetProperty("amount").GetInt64() ||
                    !indices.SequenceEqual(want.GetProperty("evidence").EnumerateArray().Select(v => v.GetInt32())))
                    throw new InvalidDataException("Original avoidance grouping differs");
            }
            Console.WriteLine(JsonSerializer.Serialize(new { originalAvoidanceOutcomes = "passed", groups = groups.Count }));
            return 0;
        }
        finally { CultureInfo.CurrentCulture = previous; }
    }
}
