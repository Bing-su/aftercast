using System.Collections;
using System.Drawing;
using System.Reflection;
using System.Text.Json;

internal static class AggregateValues
{
    internal static int Run(Assembly act)
    {
        var failures = 0;
        Type Type(string name) => act.GetType("Advanced_Combat_Tracker." + name, throwOnError: true)!;
        object Create(string name, params object?[] args) => Activator.CreateInstance(Type(name), args)!;
        object? Read(object value, string property) => value.GetType().GetProperty(property)!.GetValue(value);
        void Observe(string stage, Func<object> action)
        {
            try { Console.WriteLine(JsonSerializer.Serialize(new { stage, success = true, result = action() })); }
            catch (Exception error)
            {
                failures++;
                while (error is TargetInvocationException && error.InnerException is not null) error = error.InnerException;
                Console.WriteLine(JsonSerializer.Serialize(new { stage, success = false, error = error.ToString() }));
            }
        }

        var combatantType = Type("CombatantData");
        var definitionType = combatantType.GetNestedType("DamageTypeDef")!;
        IDictionary Registry(string field) => (IDictionary)combatantType.GetField(field)!.GetValue(null)!;
        var outgoing = Registry("OutgoingDamageTypeDataObjects");
        var incoming = Registry("IncomingDamageTypeDataObjects");
        outgoing.Clear();
        incoming.Clear();
        foreach (var (field, label) in new[]
        {
            ("DamageTypeDataOutgoingDamage", "Fixture Outgoing Damage"),
            ("DamageTypeDataIncomingDamage", "Fixture Incoming Damage"),
            ("DamageTypeDataOutgoingHealing", "Fixture Outgoing Healing"),
            ("DamageTypeDataIncomingHealing", "Fixture Incoming Healing"),
            ("DamageTypeDataNonSkillDamage", "Fixture NonSkill"),
            ("DamageTypeDataOutgoingPowerReplenish", "Fixture Power Replenish"),
            ("DamageTypeDataOutgoingPowerDamage", "Fixture Power Damage"),
            ("DamageTypeDataOutgoingCures", "Fixture Cures")
        }) combatantType.GetField(field)!.SetValue(null, label);
        foreach (var (direction, label, ally) in new[]
        {
            (outgoing, "Fixture Outgoing Damage", -1),
            (outgoing, "Fixture Outgoing Healing", 1),
            (incoming, "Fixture Incoming Damage", -1),
            (incoming, "Fixture Incoming Healing", 1),
            // The genuine constructor retains the last category as its all-actions accumulator.
            (outgoing, "Fixture All Outgoing", 0),
            (incoming, "Fixture All Incoming", 0)
        }) direction.Add(label, Activator.CreateInstance(definitionType, label, ally, Color.Transparent)!);
        foreach (var (field, damage, healing) in new[]
        {
            ("SwingTypeToDamageTypeDataLinksOutgoing", "Fixture Outgoing Damage", "Fixture Outgoing Healing"),
            ("SwingTypeToDamageTypeDataLinksIncoming", "Fixture Incoming Damage", "Fixture Incoming Healing")
        })
        {
            var links = Registry(field);
            links.Clear();
            links.Add(1, new List<string> { damage });
            links.Add(3, new List<string> { healing });
        }
        foreach (var (field, swingType) in new[] { ("DamageSwingTypes", 1), ("HealingSwingTypes", 3) })
        {
            var list = (IList)combatantType.GetField(field)!.GetValue(null)!;
            list.Clear();
            list.Add(swingType);
        }
        Type("ActGlobals").GetField("charName")!.SetValue(null, "Source");
        Console.WriteLine(JsonSerializer.Serialize(new
        {
            configuration = "Explicit fixture categories, not FFXIV defaults",
            damageSwing = 1,
            healingSwing = 3,
            damageAllyValue = -1,
            healingAllyValue = 1,
            selectiveParsing = false,
            character = "Source"
        }));

        var start = DateTime.UnixEpoch;
        var zone = Create("ZoneData", start, "Fixture Zone", false, false, false);
        var encounter = Create("EncounterData", "Source", "Fixture Zone", zone);
        ((IList)Read(zone, "Items")!).Add(encounter);
        zone.GetType().GetProperty("ActiveEncounter")!.SetValue(zone, encounter);
        var add = zone.GetType().GetMethod("AddCombatAction")!;
        foreach (var (swing, critical, amount, second, sorter, attack, victim) in new[]
        {
            (1, false, 100L, 0, 1, "Strike", "Target"),
            (3, false, 20L, 1, 2, "Heal", "Source"),
            (1, true, 300L, 2, 3, "Strike", "Target")
        }) Observe($"AddCombatAction {sorter}", () =>
        {
            var action = Create("MasterSwing", swing, critical, "Fixture", Create("Dnum", amount), start.AddSeconds(second), sorter,
                attack, "Source", swing == 3 ? "Healing" : "Physical", victim);
            add.Invoke(zone, [action]);
            return new { amount, swing, second, parentMatches = ReferenceEquals(Read(action, "ParentEncounter"), encounter) };
        });

        var combatants = (IDictionary)Read(encounter, "Items")!;
        // Configure actual public ally selection; automatic resolution also writes to the absent main-form debug log.
        var allies = (IList)Activator.CreateInstance(typeof(List<>).MakeGenericType(combatantType))!;
        allies.Add(combatants["SOURCE"]!);
        encounter.GetType().GetMethod("SetAllies")!.Invoke(encounter, [allies]);
        Console.WriteLine(JsonSerializer.Serialize(new { manualAllies = new[] { "Source" }, allOutgoingLast = "Fixture All Outgoing", allIncomingLast = "Fixture All Incoming" }));
        foreach (var key in new[] { "SOURCE", "TARGET" }) Observe($"Combatant {key}", () =>
        {
            var actor = combatants[key]!;
            return new
            {
                name = Read(actor, "Name"), damage = Read(actor, "Damage"), healed = Read(actor, "Healed"),
                damageTaken = Read(actor, "DamageTaken"), healsTaken = Read(actor, "HealsTaken"),
                swings = Read(actor, "Swings"), hits = Read(actor, "Hits"), critHits = Read(actor, "CritHits"), heals = Read(actor, "Heals")
            };
        });
        Observe("Encounter totals", () => new
        {
            active = Read(encounter, "Active"), damage = Read(encounter, "Damage"), healed = Read(encounter, "Healed"),
            duration = Read(encounter, "Duration"), combatants = combatants.Count,
            mainFormIsNull = Type("ActGlobals").GetField("oFormActMain")!.GetValue(null) is null
        });
        return failures;
    }
}
