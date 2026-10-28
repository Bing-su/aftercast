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
        Console.WriteLine("Original special-count fixtures passed.");
    }
}
