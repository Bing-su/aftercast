using System.Drawing;
using System.Reflection;
using System.Security.Cryptography;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

// Compare real action-derived ally graphs; no manually seeded aggregate or graph state is needed.
public static class AutomaticAlliesProbe
{
    private const string OriginalHash = "934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657";

    private static void Check(bool condition, string scenario)
    {
        if (!condition) throw new Exception("Automatic ally probe failed: " + scenario);
    }

    internal static List<CombatantData> ReadAllies(EncounterData encounter, bool allowLimited = false)
    {
        try { return encounter.GetAllies(allowLimited); }
        catch (NullReferenceException error)
        {
            // Original ACT commits its cache before calling a null main form's debug logger (0600078e IL712).
            // Accept only that pinned artifact and committed cache; replacement failures must propagate.
            var assembly = typeof(EncounterData).Assembly;
            if (ActGlobals.oFormActMain is not null || error.TargetSite?.Name != "GetAllies" ||
                error.TargetSite.DeclaringType != typeof(EncounterData) ||
                Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(assembly.Location))).ToLowerInvariant() != OriginalHash)
                throw;
            var fields = typeof(EncounterData).GetFields(BindingFlags.Instance | BindingFlags.NonPublic);
            var cached = fields.Single(field => field.MetadataToken == 0x04000683);
            var values = fields.Single(field => field.MetadataToken == 0x04000684);
            if (cached.GetValue(encounter) is not true || values.GetValue(encounter) is not List<CombatantData> committed)
                throw;
            var result = encounter.GetAllies(allowLimited);
            Check(ReferenceEquals(result, committed), "original cache survives debug logger failure");
            return result;
        }
    }

    private static void Names(EncounterData encounter, params string[] expected) =>
        Check(ReadAllies(encounter).Select(actor => actor.Name).OrderBy(name => name)
            .SequenceEqual(expected.OrderBy(name => name)), string.Join(",", expected));

    public static string Run()
    {
        var outgoing = CombatantData.OutgoingDamageTypeDataObjects;
        var incoming = CombatantData.IncomingDamageTypeDataObjects;
        var outgoingLinks = CombatantData.SwingTypeToDamageTypeDataLinksOutgoing;
        var incomingLinks = CombatantData.SwingTypeToDamageTypeDataLinksIncoming;
        var damageTypes = CombatantData.DamageSwingTypes;
        var healingTypes = CombatantData.HealingSwingTypes;
        var outgoingDamage = CombatantData.DamageTypeDataOutgoingDamage;
        var outgoingHealing = CombatantData.DamageTypeDataOutgoingHealing;
        var incomingDamage = CombatantData.DamageTypeDataIncomingDamage;
        var incomingHealing = CombatantData.DamageTypeDataIncomingHealing;
        var restrict = ActGlobals.restrictToAll;
        try
        {
            CombatantData.DamageTypeDataOutgoingDamage = "Damage";
            CombatantData.DamageTypeDataOutgoingHealing = "Healing";
            CombatantData.DamageTypeDataIncomingDamage = "Taken";
            CombatantData.DamageTypeDataIncomingHealing = "Received";
            CombatantData.OutgoingDamageTypeDataObjects = new()
            {
                ["Damage"] = new("Damage", -1, Color.Red),
                ["Healing"] = new("Healing", 1, Color.Green),
                ["All Out"] = new("All", 0, Color.Transparent)
            };
            CombatantData.IncomingDamageTypeDataObjects = new()
            {
                ["Taken"] = new("Taken", -1, Color.Red),
                ["Received"] = new("Received", 1, Color.Green),
                ["All In"] = new("All", 0, Color.Transparent)
            };
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing = new()
            { [1] = new() { "Damage" }, [3] = new() { "Healing" } };
            CombatantData.SwingTypeToDamageTypeDataLinksIncoming = new()
            { [1] = new() { "Taken" }, [3] = new() { "Received" } };
            CombatantData.DamageSwingTypes = new() { 1 };
            CombatantData.HealingSwingTypes = new() { 3 };
            ActGlobals.restrictToAll = false;
            var sorter = 0;
            EncounterData Fixture(string character = "Player")
            {
                var zone = new ZoneData(DateTime.UnixEpoch, "Fixture", false, false, false);
                var encounter = new EncounterData(character, "Fixture", zone);
                zone.Items.Add(encounter);
                zone.ActiveEncounter = encounter;
                return encounter;
            }
            void Add(EncounterData encounter, string attacker, string victim, int type = 1) =>
                encounter.Parent.AddCombatAction(new MasterSwing(type, false, "Fixture", new Dnum(10),
                    DateTime.UnixEpoch.AddSeconds(++sorter), sorter, "Action", attacker, "Fixture", victim));

            var attack = Fixture();
            Add(attack, "Player", "Enemy");
            Names(attack, "Player");
            var cached = ReadAllies(attack);
            Check(ReferenceEquals(cached, ReadAllies(attack, true)), "limited cached identity");
            Add(attack, "Other", "Disconnected");
            Names(attack, "Player");
            Add(attack, "Healer", "Player", 3);
            Names(attack, "Healer", "Player");

            var victim = Fixture();
            Add(victim, "Enemy", "Player");
            Names(victim, "Player");
            var healing = Fixture();
            Add(healing, "Player", "Friend", 3);
            Names(healing, "Friend", "Player");
            var unknown = Fixture("Unknown");
            Add(unknown, "Player", "Enemy");
            Names(unknown);

            // Opposite category weights cancel on both endpoints; action amount does not set ally weight.
            var cancelled = Fixture();
            Add(cancelled, "Player", "Enemy");
            Add(cancelled, "Player", "Enemy", 3);
            Names(cancelled);
            Add(cancelled, "Player", "Enemy");
            Names(cancelled, "Player");

            var manual = new List<CombatantData> { attack.GetCombatant("Enemy")! };
            attack.SetAllies(manual);
            Add(attack, "Player", "Enemy");
            attack.SetAlliesUncached();
            Check(ReferenceEquals(ReadAllies(attack), manual) && ReferenceEquals(ReadAllies(attack, true), manual),
                "manual list survives actions and invalidation");
            attack.SetAllies(new List<CombatantData>());
            Names(attack, "Healer", "Player");
            attack.SetAllies(manual);
            attack.SetAllies(null!);
            Names(attack, "Healer", "Player");
            return "Automatic allies";
        }
        finally
        {
            CombatantData.OutgoingDamageTypeDataObjects = outgoing;
            CombatantData.IncomingDamageTypeDataObjects = incoming;
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing = outgoingLinks;
            CombatantData.SwingTypeToDamageTypeDataLinksIncoming = incomingLinks;
            CombatantData.DamageSwingTypes = damageTypes;
            CombatantData.HealingSwingTypes = healingTypes;
            CombatantData.DamageTypeDataOutgoingDamage = outgoingDamage;
            CombatantData.DamageTypeDataOutgoingHealing = outgoingHealing;
            CombatantData.DamageTypeDataIncomingDamage = incomingDamage;
            CombatantData.DamageTypeDataIncomingHealing = incomingHealing;
            ActGlobals.restrictToAll = restrict;
        }
    }
}
