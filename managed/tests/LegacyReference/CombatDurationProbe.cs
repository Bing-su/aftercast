using System.Drawing;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Security.Cryptography;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class CombatDurationProbe
{
    private static readonly DateTime Epoch = DateTime.UnixEpoch;
    private static void Check(bool condition, [CallerArgumentExpression(nameof(condition))] string? scenario = null)
    {
        if (!condition) throw new Exception("Original-reference combat duration probe failed: " + scenario);
    }

    private static void Throws<T>(Action action) where T : Exception
    {
        try { action(); }
        catch (T) { return; }
        throw new Exception($"Duration query did not throw {typeof(T).Name}");
    }

    private static TimeSpan ActorDuration(CombatantData actor)
    {
        try { return actor.Duration; }
        catch (NullReferenceException error)
        {
            // Genuine ACT logs through its absent UI root after caching a segmented result; re-read only that cache.
            var assembly = typeof(CombatantData).Assembly;
            if (ActGlobals.oFormActMain is not null || error.TargetSite?.Name != "get_Duration" ||
                error.TargetSite.DeclaringType != typeof(CombatantData) || error.TargetSite.MetadataToken != 0x06000741 ||
                assembly.ManifestModule.ModuleVersionId != new Guid("94f61cd2-1f88-4a7c-98d1-0646897b1fb4") ||
                Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(assembly.Location))).ToLowerInvariant() !=
                    "934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657")
                throw;
            var fields = typeof(CombatantData).GetFields(BindingFlags.NonPublic | BindingFlags.Instance);
            if (fields.Single(field => field.MetadataToken == 0x04000673).GetValue(actor) is not true ||
                fields.Single(field => field.MetadataToken == 0x0400067b).GetValue(actor) is not TimeSpan committed)
                throw;
            var result = actor.Duration;
            Check(result == committed);
            return result;
        }
    }

    // Read the obsolete no-message alias through reflection so reference warnings stay errors.
    private static double AttackExtDps(AttackType attack) => (double)typeof(AttackType).GetProperty("ExtDPS")!.GetValue(attack)!;

    private static (EncounterData Encounter, CombatantData Actor) Fixture()
    {
        var zone = new ZoneData(Epoch, "Fixture Zone", false, false, false);
        var encounter = new EncounterData("Source", "Fixture Zone", zone);
        var actor = new CombatantData("Source", encounter);
        encounter.Items.Add("SOURCE", actor);
        encounter.SetAllies(new() { actor });
        return (encounter, actor);
    }

    private static MasterSwing Swing(int second, int sorter, long amount = 100, int type = 1) =>
        new(type, false, "", new Dnum(amount), Epoch.AddSeconds(second), sorter,
            type == 3 ? "Heal" : "Strike", "Source", "Physical", "Target");

    private static void Add(CombatantData actor, params (int Second, int Sorter)[] points)
    {
        foreach (var point in points) actor.AddCombatAction(Swing(point.Second, point.Sorter));
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
        var incremental = ActGlobals.disableIncrementalCaching;
        try
        {
            CombatantData.DamageTypeDataOutgoingDamage = "Damage";
            CombatantData.DamageTypeDataOutgoingHealing = "Healing";
            CombatantData.DamageTypeDataIncomingDamage = "Incoming Damage";
            CombatantData.DamageTypeDataIncomingHealing = "Incoming Healing";
            CombatantData.OutgoingDamageTypeDataObjects = new()
            {
                ["Damage"] = new("Damage", -1, Color.Red), ["Healing"] = new("Healing", 1, Color.Green),
                ["All Outgoing"] = new("All outgoing", 0, Color.Transparent)
            };
            CombatantData.IncomingDamageTypeDataObjects = new()
            {
                ["Incoming Damage"] = new("Damage taken", -1, Color.Red), ["Incoming Healing"] = new("Healing taken", 1, Color.Green),
                ["All Incoming"] = new("All incoming", 0, Color.Transparent)
            };
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing = new() { [1] = new() { "Damage" }, [3] = new() { "Healing" } };
            CombatantData.SwingTypeToDamageTypeDataLinksIncoming = new() { [1] = new() { "Incoming Damage" }, [3] = new() { "Incoming Healing" } };
            ActGlobals.restrictToAll = ActGlobals.longDuration = ActGlobals.disableIncrementalCaching = false;

            var empty = Fixture();
            Check(empty.Actor.Duration == TimeSpan.Zero && double.IsNaN(empty.Actor.DPS) && double.IsNaN(empty.Actor.EncDPS));
            Check(double.IsNaN(empty.Actor.ExtDPS) && double.IsNaN(empty.Actor.EncHPS) && double.IsNaN(empty.Actor.ExtHPS));
            var missing = new DamageTypeData(true, "Missing", null!);
            Check(missing.Duration == TimeSpan.Zero && missing.DPS == 0 && missing.EncDPS == 0 && missing.CharDPS == 0 && missing.ExtDPS == 0);
            var unattached = new AttackType("Unattached", missing);
            Check(unattached.Duration == TimeSpan.Zero && unattached.DPS == 0);
            Check(double.IsNaN(unattached.EncDPS) && double.IsNaN(unattached.CharDPS) && double.IsNaN(AttackExtDps(unattached)));
            var nullAttack = new AttackType("Null", null!);
            Throws<NullReferenceException>(() => _ = nullAttack.Duration);
            Throws<NullReferenceException>(() => _ = nullAttack.EncDPS);
            Throws<NullReferenceException>(() => _ = new CombatantData("Null", null!).Duration);

            var zero = Fixture();
            zero.Actor.AddCombatAction(Swing(0, 1));
            Check(zero.Actor.Duration == TimeSpan.Zero && double.IsPositiveInfinity(zero.Actor.DPS) && double.IsPositiveInfinity(zero.Actor.EncDPS));
            Check(double.IsNaN(zero.Actor.EncHPS));
            var zeroAttack = zero.Actor.AllOut["All"];
            Check(zeroAttack.DPS == 0 && zeroAttack.EncDPS == 0 && zeroAttack.CharDPS == 0);

            var single = Fixture();
            single.Encounter.StartTimes.Add(Epoch);
            single.Actor.AddCombatAction(Swing(2, 1, 300));
            single.Actor.AddCombatAction(Swing(0, 2));
            single.Actor.AddCombatAction(Swing(1, 3, 20, 3));
            Check(single.Actor.Duration == TimeSpan.FromSeconds(2) && single.Actor.DPS == 200 && single.Actor.EncDPS == 200 && single.Actor.ExtDPS == 200);
            Check(single.Actor.EncHPS == 10 && single.Actor.ExtHPS == 10);
            var singleAttack = single.Actor.AllOut["Strike"];
            Check(singleAttack.Duration == TimeSpan.FromSeconds(2) && singleAttack.DPS == 200 && singleAttack.EncDPS == 200 && singleAttack.CharDPS == 200 && AttackExtDps(singleAttack) == 200);
            Check(single.Actor.Items["Damage"].Duration == TimeSpan.FromSeconds(2) && single.Actor.Items["Damage"].DPS == 200);
            Check(single.Actor.Items["Healing"].DPS == 0 && single.Actor.Items["Healing"].EncDPS == 10 && single.Actor.Items["Healing"].CharDPS == 10);

            var multiple = Fixture();
            Add(multiple.Actor, (0, 0), (2, 1), (10, 2), (12, 3));
            multiple.Encounter.StartTimes = new() { Epoch, Epoch.AddSeconds(10) };
            multiple.Encounter.EndTimes = new() { Epoch.AddSeconds(2) };
            Check(ActorDuration(multiple.Actor) == TimeSpan.FromSeconds(4));
            var attack = multiple.Actor.AllOut["All"];
            var category = attack.Parent;
            Check(attack.Duration == TimeSpan.FromSeconds(4) && category.Duration == TimeSpan.FromSeconds(4));
            Check(multiple.Actor.DPS == 100 && multiple.Actor.EncDPS == 100 && attack.DPS == 100 && attack.EncDPS == 100);

            multiple.Encounter.EndTimes.Add(Epoch.AddSeconds(12));
            attack.Items[^1] = Swing(14, 3);
            Check(attack.Duration == TimeSpan.FromSeconds(4) && category.Duration == TimeSpan.FromSeconds(4) && multiple.Actor.Duration == TimeSpan.FromSeconds(4));
            attack.InvalidateCachedValues();
            Check(attack.Duration == TimeSpan.FromSeconds(6) && category.Duration == TimeSpan.FromSeconds(4));
            category.InvalidateCachedValues();
            Check(category.Duration == TimeSpan.FromSeconds(6) && multiple.Actor.Duration == TimeSpan.FromSeconds(4));
            multiple.Actor.InvalidateCachedValues();
            Check(ActorDuration(multiple.Actor) == TimeSpan.FromSeconds(6));

            var boundary = Fixture();
            Add(boundary.Actor, (0, 0), (2, 1), (5, 2), (10, 3), (14, 4));
            boundary.Encounter.StartTimes = new() { Epoch, Epoch.AddSeconds(10) };
            boundary.Encounter.EndTimes = new() { Epoch.AddSeconds(2), Epoch.AddSeconds(12) };
            // Original personal duration includes records in a segment gap before the next start.
            Check(ActorDuration(boundary.Actor) == TimeSpan.FromSeconds(9));
            Check(boundary.Actor.AllOut["All"].Duration == TimeSpan.FromSeconds(9));

            var noFirst = Fixture();
            Add(noFirst.Actor, (10, 1), (12, 2));
            noFirst.Encounter.StartTimes = new() { Epoch, Epoch.AddSeconds(10) };
            noFirst.Encounter.EndTimes = new() { Epoch.AddSeconds(2) };
            Check(ActorDuration(noFirst.Actor) == TimeSpan.FromSeconds(2) && noFirst.Actor.AllOut["All"].Duration == TimeSpan.FromSeconds(2));

            var reordered = Fixture();
            Add(reordered.Actor, (2, 0), (0, 1), (12, 2), (10, 3));
            reordered.Encounter.StartTimes = new() { Epoch, Epoch.AddSeconds(10) };
            reordered.Encounter.EndTimes = new() { Epoch.AddSeconds(2), Epoch.AddSeconds(12) };
            Check(ActorDuration(reordered.Actor) == TimeSpan.FromSeconds(-4));
            Check(reordered.Actor.AllOut["All"].Duration == TimeSpan.FromSeconds(-4));
            Check(reordered.Actor.DPS == -100 && reordered.Actor.AllOut["All"].DPS == 0);

            var emptySegments = Fixture();
            emptySegments.Encounter.StartTimes = new() { Epoch, Epoch.AddSeconds(10) };
            emptySegments.Encounter.EndTimes = new() { Epoch.AddSeconds(2), Epoch.AddSeconds(12) };
            var emptyCategory = new DamageTypeData(true, "Empty", emptySegments.Actor);
            Check(emptyCategory.Duration == TimeSpan.Zero);
            var emptyAttack = new AttackType("All", emptyCategory);
            emptyCategory.Items.Add("All", emptyAttack);
            emptySegments.Actor.AllOut.Add("All", emptyAttack);
            Throws<ArgumentOutOfRangeException>(() => _ = emptyAttack.Duration);
            Throws<ArgumentOutOfRangeException>(() => _ = emptyCategory.Duration);
            Throws<ArgumentOutOfRangeException>(() => _ = emptySegments.Actor.Duration);

            var malformed = Fixture();
            Add(malformed.Actor, (0, 0), (1, 1));
            malformed.Encounter.StartTimes = new() { Epoch, Epoch.AddSeconds(10) };
            Throws<ArgumentOutOfRangeException>(() => _ = malformed.Actor.AllOut["All"].Duration);
            Throws<ArgumentOutOfRangeException>(() => _ = malformed.Actor.Duration);
            malformed.Encounter.EndTimes = new() { Epoch.AddSeconds(2), Epoch.AddSeconds(12), Epoch.AddSeconds(20) };
            Check(malformed.Actor.AllOut["All"].Duration == TimeSpan.FromSeconds(1));
            Throws<ArgumentOutOfRangeException>(() => _ = malformed.Actor.Duration);

            var close = Fixture();
            Add(close.Actor, (0, 0), (2, 1), (10, 2), (12, 3));
            close.Encounter.StartTimes = new() { Epoch, Epoch.AddSeconds(10) };
            close.Encounter.EndTimes = new() { Epoch.AddSeconds(2) };
            Check(ActorDuration(close.Actor) == TimeSpan.FromSeconds(4));
            var closingAttack = close.Actor.AllOut["All"];
            Check(closingAttack.Duration == TimeSpan.FromSeconds(4));
            closingAttack.Items[^1] = Swing(14, 3);
            close.Encounter.EndCombat(false);
            close.Encounter.Trim();
            // Closing/trimming without a new action does not invalidate ACT's already committed personal duration.
            Check(close.Actor.Duration == TimeSpan.FromSeconds(4) && closingAttack.Duration == TimeSpan.FromSeconds(4));
            return "Combat durations";
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
            ActGlobals.disableIncrementalCaching = incremental;
        }
    }
}
