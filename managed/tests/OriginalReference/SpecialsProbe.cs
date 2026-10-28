using System.Collections;
using System.Reflection;
using System.Text.Json;

internal static class SpecialsProbe
{
    internal static int Run(Assembly act, string path)
    {
        using var fixture = JsonDocument.Parse(File.ReadAllText(path));
        var attackType = act.GetType("Advanced_Combat_Tracker.AttackType", true)!;
        var swingType = act.GetType("Advanced_Combat_Tracker.MasterSwing", true)!;
        var dnumType = act.GetType("Advanced_Combat_Tracker.Dnum", true)!;
        foreach (var item in fixture.RootElement.GetProperty("cases").EnumerateArray())
        {
            var attack = Activator.CreateInstance(attackType, new object?[] { "Strike", null })!;
            foreach (var row in item.GetProperty("rows").EnumerateArray())
            {
                var sorter = row[0].GetInt32();
                var damage = Activator.CreateInstance(dnumType, new object[] { 100L })!;
                var swing = Activator.CreateInstance(swingType, new object[] { 1, false, row[1].GetString()!, damage, DateTime.UnixEpoch, sorter, "Strike", "Source", "Physical", "Target" })!;
                attackType.GetMethod("AddCombatAction")!.Invoke(attack, new[] { swing });
            }
            var actual = (IDictionary)attackType.GetMethod("GetAttackSpecials")!.Invoke(attack, null)!;
            var expected = item.GetProperty("expected");
            if (actual.Count != expected.EnumerateObject().Count() || expected.EnumerateObject().Any(e => !actual.Contains(e.Name) || (int)actual[e.Name]! != e.Value.GetInt32()))
                throw new InvalidDataException($"Original special counts differ: {item.GetProperty("name").GetString()}");
        }
        Console.WriteLine("Original GetAttackSpecials fixture passed.");
        return 0;
    }
}
