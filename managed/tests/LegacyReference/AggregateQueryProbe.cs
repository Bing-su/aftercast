using System.Drawing;
using System.Runtime.CompilerServices;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class AggregateQueryProbe
{
    private static void Check(bool condition, [CallerArgumentExpression(nameof(condition))] string? scenario = null)
    {
        if (!condition) throw new Exception("Original-reference aggregate query probe failed: " + scenario);
    }

    public static string Run()
    {
        var outgoing = CombatantData.OutgoingDamageTypeDataObjects;
        var incoming = CombatantData.IncomingDamageTypeDataObjects;
        var outgoingLinks = CombatantData.SwingTypeToDamageTypeDataLinksOutgoing;
        var incomingLinks = CombatantData.SwingTypeToDamageTypeDataLinksIncoming;
        var names = new[] { CombatantData.DamageTypeDataOutgoingDamage, CombatantData.DamageTypeDataOutgoingHealing,
            CombatantData.DamageTypeDataIncomingDamage, CombatantData.DamageTypeDataIncomingHealing };
        var restrict = ActGlobals.restrictToAll;
        var longDuration = ActGlobals.longDuration;
        try
        {
            CombatantData.DamageTypeDataOutgoingDamage = "Damage";
            CombatantData.DamageTypeDataOutgoingHealing = "Healing";
            CombatantData.DamageTypeDataIncomingDamage = "Taken";
            CombatantData.DamageTypeDataIncomingHealing = "Received";
            CombatantData.OutgoingDamageTypeDataObjects = new()
            {
                ["Damage"] = new("Damage", -1, Color.Red), ["Healing"] = new("Healing", 1, Color.Green),
                ["All Out"] = new("All outgoing", 0, Color.Transparent)
            };
            CombatantData.IncomingDamageTypeDataObjects = new()
            {
                ["Taken"] = new("Taken", -1, Color.Red), ["Received"] = new("Received", 1, Color.Green),
                ["All In"] = new("All incoming", 0, Color.Transparent)
            };
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing = new() { [1] = new() { "Damage" }, [3] = new() { "Healing" } };
            CombatantData.SwingTypeToDamageTypeDataLinksIncoming = new() { [1] = new() { "Taken" }, [3] = new() { "Received" } };
            ActGlobals.restrictToAll = ActGlobals.longDuration = false;
            var sorter = 0;
            EncounterData Fixture(bool ignoreEnemies = false)
            {
                var zone = new ZoneData(DateTime.UnixEpoch, "Fixture", false, false, ignoreEnemies);
                return ignoreEnemies ? new EncounterData("Player", "Fixture", true, zone) : new EncounterData("Player", "Fixture", zone);
            }
            CombatantData Actor(EncounterData encounter, string name)
            {
                var key = name.ToUpper();
                if (!encounter.Items.TryGetValue(key, out var actor))
                {
                    actor = new CombatantData(name, encounter);
                    encounter.Items.Add(key, actor);
                }
                return actor;
            }
            MasterSwing Action(EncounterData encounter, string attacker, string victim, Dnum damage, string attackType = "Strike", int second = 0) =>
                new(1, false, "", damage, DateTime.UnixEpoch.AddSeconds(second), ++sorter, attackType, attacker, "Physical", victim)
                { ParentEncounter = encounter };
            void Add(EncounterData encounter, string attacker, string victim, Dnum damage, string attackType = "Strike", int second = 0)
            {
                var action = Action(encounter, attacker, victim, damage, attackType, second);
                Actor(encounter, attacker).AddCombatAction(action);
                Actor(encounter, victim).AddReverseCombatAction(action);
            }
            (EncounterData Encounter, CombatantData Player, CombatantData Spaced, CombatantData Enemy) DeathFixture(bool ignoreEnemies)
            {
                var encounter = Fixture(ignoreEnemies);
                var player = Actor(encounter, "Player");
                var spaced = Actor(encounter, "Player One");
                var enemy = Actor(encounter, "Enemy");
                encounter.SetAllies(new() { player, spaced });
                Add(encounter, "Player", "NPC With Space", Dnum.Death);
                Add(encounter, "Enemy", "Player", Dnum.Death, second: 1);
                Add(encounter, "Enemy", "Player One", Dnum.Death, second: 2);
                return (encounter, player, spaced, enemy);
            }

            var fixture = DeathFixture(false);
            Check(fixture.Player.Kills == 1 && fixture.Enemy.Kills == 1 && fixture.Spaced.Kills == 0);
            Check(fixture.Encounter.AlliedKills == 1 && fixture.Encounter.AlliedDeaths == 1);
            Check(fixture.Player.Deaths == 1 && fixture.Spaced.Deaths == 1);
            var ignored = DeathFixture(true);
            Check(ignored.Encounter.AlliedKills == 2 && ignored.Encounter.AlliedDeaths == 1);

            var killing = new AttackType("Killing", fixture.Player.AllOut["All"].Parent);
            killing.Items.Add(Action(fixture.Encounter, "Player", "NPC With Space", new Dnum(100), "Killing"));
            fixture.Player.AllOut.Add("Killing", killing);
            fixture.Player.InvalidateCachedValues();
            Check(fixture.Player.Kills == 0);
            Add(fixture.Encounter, "Player", "NPC With Space", Dnum.Death, "Killing");
            Check(fixture.Player.Kills == 1);
            killing.Items.Clear();
            Check(fixture.Player.Kills == 1);
            fixture.Player.InvalidateCachedValues();
            Check(fixture.Player.Kills == 0);
            fixture.Player.AllOut.Remove("Killing");
            fixture.Player.InvalidateCachedValues();
            Check(fixture.Player.Kills == 2);
            fixture.Player.AllOut["All"].Items.RemoveAt(0);
            Add(fixture.Encounter, "Player", "Enemy", new Dnum(1));
            Check(fixture.Player.Kills == 2);
            fixture.Player.InvalidateCachedValues();
            Check(fixture.Player.Kills == 1);
            Add(fixture.Encounter, "Player", "Enemy", Dnum.Death);
            Check(fixture.Player.Kills == 2);
            fixture.Player.InvalidateCachedValues();
            Check(fixture.Player.Kills == 2);

            fixture.Encounter.SetAllies(new() { fixture.Enemy });
            fixture.Player.InvalidateCachedValues();
            Check(fixture.Player.Kills == 1);
            Add(fixture.Encounter, "Player", "Enemy", new Dnum(-10, "custom death"));
            fixture.Player.InvalidateCachedValues();
            Check(fixture.Player.Kills == 1);

            var rates = Fixture();
            var source = Actor(rates, "Player");
            rates.SetAllies(new() { source });
            Check(double.IsNaN(rates.DPS));
            Add(rates, "Player", "Enemy", new Dnum(100));
            Check(double.IsPositiveInfinity(rates.DPS));
            Add(rates, "Player", "Enemy", new Dnum(300), second: 2);
            Check(rates.DPS == 200);
            return "Aggregate queries";
        }
        finally
        {
            CombatantData.OutgoingDamageTypeDataObjects = outgoing;
            CombatantData.IncomingDamageTypeDataObjects = incoming;
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing = outgoingLinks;
            CombatantData.SwingTypeToDamageTypeDataLinksIncoming = incomingLinks;
            CombatantData.DamageTypeDataOutgoingDamage = names[0];
            CombatantData.DamageTypeDataOutgoingHealing = names[1];
            CombatantData.DamageTypeDataIncomingDamage = names[2];
            CombatantData.DamageTypeDataIncomingHealing = names[3];
            ActGlobals.restrictToAll = restrict;
            ActGlobals.longDuration = longDuration;
        }
    }
}
