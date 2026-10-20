using System.Drawing;
using Advanced_Combat_Tracker;

namespace Bxivact.LegacyReference;

// Exercise the same registered categories and action objects against original ACT and its replacement.
public static class CombatActionProbe
{
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Original-reference combat action probe failed");
    }

    public static string Run()
    {
        var outgoing = CombatantData.OutgoingDamageTypeDataObjects;
        var incoming = CombatantData.IncomingDamageTypeDataObjects;
        var outgoingLinks = CombatantData.SwingTypeToDamageTypeDataLinksOutgoing;
        var incomingLinks = CombatantData.SwingTypeToDamageTypeDataLinksIncoming;
        var damageTypes = CombatantData.DamageSwingTypes;
        var healingTypes = CombatantData.HealingSwingTypes;
        var names = new[] { CombatantData.DamageTypeDataNonSkillDamage, CombatantData.DamageTypeDataOutgoingDamage,
            CombatantData.DamageTypeDataOutgoingHealing, CombatantData.DamageTypeDataIncomingDamage,
            CombatantData.DamageTypeDataIncomingHealing, CombatantData.DamageTypeDataOutgoingPowerReplenish,
            CombatantData.DamageTypeDataOutgoingPowerDamage, CombatantData.DamageTypeDataOutgoingCures };
        var restrict = ActGlobals.restrictToAll;
        var longDuration = ActGlobals.longDuration;
        var charName = ActGlobals.charName;
        try
        {
            CombatantData.DamageTypeDataNonSkillDamage = "NonSkill";
            CombatantData.DamageTypeDataOutgoingDamage = "Outgoing Damage";
            CombatantData.DamageTypeDataOutgoingHealing = "Outgoing Healing";
            CombatantData.DamageTypeDataIncomingDamage = "Incoming Damage";
            CombatantData.DamageTypeDataIncomingHealing = "Incoming Healing";
            CombatantData.DamageTypeDataOutgoingPowerReplenish = "Power Replenish";
            CombatantData.DamageTypeDataOutgoingPowerDamage = "Power Damage";
            CombatantData.DamageTypeDataOutgoingCures = "Cures";
            CombatantData.OutgoingDamageTypeDataObjects = new()
            {
                ["Outgoing Damage"] = new("Damage", -1, Color.Red),
                ["Outgoing Healing"] = new("Healing", 1, Color.Green),
                ["All Outgoing"] = new("All outgoing", 0, Color.Transparent)
            };
            CombatantData.IncomingDamageTypeDataObjects = new()
            {
                ["Incoming Damage"] = new("Damage taken", -1, Color.Red),
                ["Incoming Healing"] = new("Healing taken", 1, Color.Green),
                ["All Incoming"] = new("All incoming", 0, Color.Transparent)
            };
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing = new()
            {
                [1] = new() { "Outgoing Damage" }, [3] = new() { "Outgoing Healing" }
            };
            CombatantData.SwingTypeToDamageTypeDataLinksIncoming = new()
            {
                [1] = new() { "Incoming Damage" }, [3] = new() { "Incoming Healing" }
            };
            CombatantData.DamageSwingTypes = new() { 1 };
            CombatantData.HealingSwingTypes = new() { 3 };
            ActGlobals.restrictToAll = ActGlobals.longDuration = false;
            ActGlobals.charName = "Source";
            var epoch = DateTime.UnixEpoch;
            MasterSwing Action(int type, long amount, int second, int sorter, bool critical = false) =>
                new(type, critical, "Fixture", new Dnum(amount), epoch.AddSeconds(second), sorter,
                    type == 3 ? "Heal" : "Strike", "Source", type == 3 ? "Healing" : "Physical", type == 3 ? "Source" : "Target");
            (ZoneData Zone, EncounterData Encounter) Fixture(bool merged = false)
            {
                var zone = new ZoneData(epoch, "Fixture Zone", merged, false, false);
                var encounter = new EncounterData("Source", "Fixture Zone", zone);
                zone.Items.Add(encounter);
                zone.ActiveEncounter = encounter;
                return (zone, encounter);
            }

            var (zone, encounter) = Fixture();
            var first = Action(1, 100, 0, 1);
            zone.AddCombatAction(first);
            zone.AddCombatAction(Action(3, 20, 1, 2));
            zone.AddCombatAction(Action(1, 300, 2, 3, true));
            var source = encounter.GetCombatant("source")!;
            var target = encounter.GetCombatant("TARGET")!;
            var allies = new List<CombatantData> { source };
            encounter.SetAllies(allies);
            Check(ReferenceEquals(first.ParentEncounter, encounter) && encounter.Active);
            Check(encounter.NumCombatants == 2 && encounter.NumAllies == 1 && encounter.NumEnemies == 1);
            Check(ReferenceEquals(encounter.GetAllies(), allies) && ReferenceEquals(encounter.GetAllies(true), allies));
            Check(source.Damage == 400 && source.Healed == 20 && target.DamageTaken == 400 && source.HealsTaken == 20);
            Check(source.Swings == 2 && source.Hits == 2 && source.CritHits == 1 && source.Heals == 1);
            Check(source.Allies["TARGET"] == -2 && source.Allies["SOURCE"] == 2);
            Check(encounter.Damage == 400 && encounter.Healed == 20 && encounter.Duration == TimeSpan.FromSeconds(2));
            Check(source.StartTime == epoch && source.EndTime == epoch.AddSeconds(2));
            encounter.EndCombat(false);
            Check(!encounter.Active && encounter.EndTimes.SequenceEqual(new[] { epoch.AddSeconds(2) }));
            zone.AddCombatAction(Action(1, 1, 10, 4));
            zone.AddCombatAction(Action(1, 1, 12, 5));
            Check(encounter.StartTimes.SequenceEqual(new[] { epoch, epoch.AddSeconds(10) }));
            Check(encounter.Duration == TimeSpan.FromSeconds(4));

            var duplicate = Fixture();
            duplicate.Encounter.DuplicateDetection = true;
            var repeated = Action(1, 100, 0, 7);
            duplicate.Zone.AddCombatAction(repeated);
            duplicate.Zone.AddCombatAction(repeated);
            duplicate.Zone.AddCombatAction(Action(1, 100, 0, 8));
            Check(duplicate.Encounter.GetCombatant("Source")!.Damage == 200);

            ActGlobals.restrictToAll = true;
            var restricted = Fixture();
            restricted.Zone.AddCombatAction(Action(1, 10, 0, 9));
            var restrictedSource = restricted.Encounter.GetCombatant("Source")!;
            Check(restrictedSource.Items["Outgoing Damage"].Items.Count == 1);
            Check(restrictedSource.AllOut.ContainsKey("Strike") && restrictedSource.Damage == 10);
            ActGlobals.restrictToAll = false;

            var merged = Fixture(true);
            var shared = Action(1, 10, 0, 10);
            merged.Zone.AddCombatAction(shared);
            Check(merged.Zone.Items[0].GetCombatant("Source")!.Damage == 10);
            Check(merged.Encounter.GetCombatant("Source")!.Damage == 10 && ReferenceEquals(shared.ParentEncounter, merged.Encounter));
            Check(merged.Zone.Items[0].Active && merged.Zone.Items[0].StartTimes.Count == 1);
            return "Combat actions";
        }
        finally
        {
            CombatantData.OutgoingDamageTypeDataObjects = outgoing;
            CombatantData.IncomingDamageTypeDataObjects = incoming;
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing = outgoingLinks;
            CombatantData.SwingTypeToDamageTypeDataLinksIncoming = incomingLinks;
            CombatantData.DamageSwingTypes = damageTypes;
            CombatantData.HealingSwingTypes = healingTypes;
            CombatantData.DamageTypeDataNonSkillDamage = names[0];
            CombatantData.DamageTypeDataOutgoingDamage = names[1];
            CombatantData.DamageTypeDataOutgoingHealing = names[2];
            CombatantData.DamageTypeDataIncomingDamage = names[3];
            CombatantData.DamageTypeDataIncomingHealing = names[4];
            CombatantData.DamageTypeDataOutgoingPowerReplenish = names[5];
            CombatantData.DamageTypeDataOutgoingPowerDamage = names[6];
            CombatantData.DamageTypeDataOutgoingCures = names[7];
            ActGlobals.restrictToAll = restrict;
            ActGlobals.longDuration = longDuration;
            ActGlobals.charName = charName;
        }
    }
}
