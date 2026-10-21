using System.Drawing;
using System.Security.Cryptography;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

// A null original UI root can fail only at the final profiling notification after real ranking/trim work.
public static class EncounterFinalizationProbe
{
    private static void Check(bool condition, [System.Runtime.CompilerServices.CallerArgumentExpression(nameof(condition))] string? expression = null)
    {
        if (!condition) throw new Exception("Original-reference encounter finalization probe failed: " + expression);
    }

    private static bool AllowOriginalDebugFailure(Action action)
    {
        try { action(); return false; }
        catch (NullReferenceException error) when (typeof(EncounterData).Module.ModuleVersionId ==
            new Guid("94f61cd2-1f88-4a7c-98d1-0646897b1fb4") && ActGlobals.oFormActMain is null &&
            error.TargetSite?.DeclaringType == typeof(EncounterData) && error.TargetSite.Name == "GetStrongestEnemy" &&
            new System.Diagnostics.StackTrace(error).GetFrame(0)?.GetILOffset() == 248 &&
            Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(typeof(EncounterData).Assembly.Location))).ToLowerInvariant() ==
                "934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657")
        {
            return true;
        }
    }

    public static string Run()
    {
        var outgoing = CombatantData.OutgoingDamageTypeDataObjects;
        var incoming = CombatantData.IncomingDamageTypeDataObjects;
        var outgoingLinks = CombatantData.SwingTypeToDamageTypeDataLinksOutgoing;
        var incomingLinks = CombatantData.SwingTypeToDamageTypeDataLinksIncoming;
        var names = (CombatantData.DamageTypeDataOutgoingDamage, CombatantData.DamageTypeDataOutgoingHealing,
            CombatantData.DamageTypeDataIncomingDamage);
        var globals = (ActGlobals.charName, ActGlobals.restrictToAll, ActGlobals.longDuration);
        try
        {
            CombatantData.DamageTypeDataOutgoingDamage = "Damage out";
            CombatantData.DamageTypeDataOutgoingHealing = "Healing out";
            CombatantData.DamageTypeDataIncomingDamage = "Damage in";
            CombatantData.OutgoingDamageTypeDataObjects = new()
            {
                ["Damage out"] = new("Damage", -1, Color.Red),
                ["Healing out"] = new("Healing", 1, Color.Green),
                ["All out"] = new("All", 0, Color.Transparent)
            };
            CombatantData.IncomingDamageTypeDataObjects = new()
            {
                ["Damage in"] = new("Damage taken", -1, Color.Red),
                ["All in"] = new("All", 0, Color.Transparent)
            };
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing = new() { [1] = new() { "Damage out" } };
            CombatantData.SwingTypeToDamageTypeDataLinksIncoming = new() { [1] = new() { "Damage in" } };
            ActGlobals.charName = "Player";
            ActGlobals.restrictToAll = ActGlobals.longDuration = false;
            var epoch = DateTime.UnixEpoch;
            var zone = new ZoneData(epoch, "Zone", false, false, false);
            var encounter = new EncounterData("Player", "Zone", zone);
            zone.Items.Add(encounter);
            zone.ActiveEncounter = encounter;
            zone.AddCombatAction(new(1, false, "Fixture", new Dnum(100), epoch, 1, "Strike", "Player", "Physical", "Big"));
            zone.AddCombatAction(new(1, false, "Fixture", new Dnum(80), epoch.AddSeconds(1), 2, "Strike", "Player", "Physical", "Small"));
            zone.AddCombatAction(new(1, false, "Fixture", Dnum.Death, epoch.AddSeconds(2), 3, "Killing", "Player", "Physical", "Small"));
            var player = encounter.GetCombatant("Player")!;
            var small = encounter.GetCombatant("Small")!;
            encounter.SetAllies(new() { player });
            Check(small.Deaths == 1 && encounter.GetCombatant("Big")!.Deaths == 0);
            Check(player.Equals(new CombatantData("PLAYER", encounter)));
            string? strongest = null;
            var debugFailed = AllowOriginalDebugFailure(() => strongest = encounter.GetStrongestEnemy("unused argument"));
            if (!debugFailed) Check(strongest == "Big");
            // Leave enough slack for the framework's TrimExcess threshold to actually reduce capacity.
            encounter.Items.Capacity = 32;
            player.AllOut["All"].Items.Capacity = 32;
            var finalizationDebugFailed = AllowOriginalDebugFailure(() => encounter.EndCombat(true));
            Check(!encounter.Active && encounter.EndTimes.SequenceEqual(new[] { epoch.AddSeconds(2) }));
            Check(encounter.Items.Capacity == encounter.Items.Count);
            Check(player.AllOut["All"].Items.Capacity == player.AllOut["All"].Items.Count);
            if (!finalizationDebugFailed) Check(encounter.Title == "Big");
            else Check(encounter.Title == "Encounter");

            var empty = new EncounterData("Absent", "Zone", zone);
            Check(empty.GetStrongestEnemy("ignored") == "Encounter");
            var zeroZone = new ZoneData(epoch, "Zone", false, false, false);
            var zero = new EncounterData("Absent", "Zone", zeroZone);
            zero.StartTimes.Add(epoch);
            zero.EndCombat(true);
            Check(zero.Title == "Encounter" && zero.EndTimes.SequenceEqual(new[] { epoch }));
            return "Encounter finalization";
        }
        finally
        {
            CombatantData.OutgoingDamageTypeDataObjects = outgoing;
            CombatantData.IncomingDamageTypeDataObjects = incoming;
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing = outgoingLinks;
            CombatantData.SwingTypeToDamageTypeDataLinksIncoming = incomingLinks;
            CombatantData.DamageTypeDataOutgoingDamage = names.Item1;
            CombatantData.DamageTypeDataOutgoingHealing = names.Item2;
            CombatantData.DamageTypeDataIncomingDamage = names.Item3;
            ActGlobals.charName = globals.Item1;
            ActGlobals.restrictToAll = globals.Item2;
            ActGlobals.longDuration = globals.Item3;
        }
    }
}
