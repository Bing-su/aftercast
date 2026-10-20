using Advanced_Combat_Tracker;

namespace Bxivact.LegacyReference;

public static class DamageStateProbe
{
    private sealed class DerivedCategory(CombatantData parent) : DamageTypeData(true, "stored", parent)
    {
        public override string ToString() => "derived category";
    }

    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Original-reference damage graph probe failed");
    }

    public static string Run()
    {
        var zone = new ZoneData(DateTime.UnixEpoch, "Fixture Zone", false, false, false);
        var encounter = new EncounterData("Fixture Player", "Fixture Zone", false, zone);
        var combatant = new CombatantData("Fixture Player", encounter);
        var category = new DamageTypeData(true, "Outgoing fixture", combatant);
        var other = new DamageTypeData(false, "Incoming fixture", combatant);
        Check(ReferenceEquals(category.Parent, combatant) && category.Outgoing && !other.Outgoing);
        Check(category.Type == "Outgoing fixture" && category.ToString() == category.Type);
        Check(category.Items.Count == 0 && other.Items.Count == 0 && category.Tags.Count == 0);
        Check(!ReferenceEquals(category.Items, other.Items) && !ReferenceEquals(category.Tags, other.Tags));
        category.Outgoing = false;
        Check(!category.Outgoing);

        var attack = new AttackType("Fixture attack", category);
        var second = new AttackType("Second", other);
        Check(ReferenceEquals(attack.Parent, category) && attack.Type == "Fixture attack" && attack.ToString() == attack.Type);
        Check(attack.Items.Count == 0 && attack.Tags.Count == 0 && !ReferenceEquals(attack.Items, second.Items));
        category.Items.Add(attack.Type, attack);
        Check(ReferenceEquals(category.Items["Fixture attack"], attack));
        var swings = new List<MasterSwing>();
        var tags = new Dictionary<string, object> { ["fixture"] = new object() };
        attack.Items = swings;
        attack.Tags = tags;
        category.Tags = tags;
        Check(ReferenceEquals(attack.Items, swings) && ReferenceEquals(attack.Tags, tags) && ReferenceEquals(category.Tags, tags));
        Check(new DerivedCategory(combatant).Type == "derived category");

        // The genuine constructors store nullable identity/parent values without eager validation.
        var missingCategory = new DamageTypeData(false, null!, null!);
        var missingAttack = new AttackType(null!, null!);
        Check(missingCategory.Parent is null && missingCategory.Type is null && missingCategory.ToString() is null);
        Check(missingAttack.Parent is null && missingAttack.Type is null && missingAttack.ToString() is null);
        return attack.Type;
    }
}
