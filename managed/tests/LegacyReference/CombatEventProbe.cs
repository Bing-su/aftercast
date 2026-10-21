using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class CombatEventProbe
{
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Original-reference combat event probe failed");
    }

    public static string Run()
    {
        var damage = new Dnum(42);
        var token = new object();
        var tags = new Dictionary<string, object> { ["token"] = token };
        var action = new MasterSwing(1, true, "Special", damage, DateTime.UnixEpoch, 7,
            "Strike", "Source", "Physical", "Target") { Tags = tags };
        var info = new CombatActionEventArgs(action);
        Check(ReferenceEquals(info.combatAction, action) && !info.cancelAction);
        Check(info.swingType == 1 && info.critical && info.timeSorter == 7 && info.time == DateTime.UnixEpoch);
        Check(info.attacker == "Source" && info.victim == "Target" && info.theAttackType == "Strike");
        Check(info.theDamageType == "Physical" && info.special == "Special" && ReferenceEquals(info.damage, damage));
        Check(ReferenceEquals(info.tags, tags) && ReferenceEquals(info.tags["token"], token));
        var called = false;
        CombatActionDelegate callback = (isImport, payload) =>
        {
            called = isImport;
            payload.attacker = "Edited";
            payload.damage = new Dnum(99);
            payload.cancelAction = true;
            payload.tags["shared"] = 1;
        };
        callback(true, info);
        Check(called && info.attacker == "Edited" && info.damage.Number == 99 && info.cancelAction);
        // Constructing or editing event arguments does not itself commit scalar changes into the action.
        Check(action.Attacker == "Source" && action.Damage.Number == 42 && action.Tags.ContainsKey("shared"));
        info.tags = new();
        Check(!ReferenceEquals(info.tags, action.Tags));
        var nullFailed = false;
        try { _ = new CombatActionEventArgs(null!); }
        catch (NullReferenceException) { nullFailed = true; }
        Check(nullFailed);

        var zone = new ZoneData(DateTime.UnixEpoch, "Fixture", false, false, false);
        var encounter = new EncounterData("Source", "Fixture", zone);
        var toggle = new CombatToggleEventArgs(2, 3, encounter);
        Check(toggle.zoneDataIndex == 2 && toggle.encounterDataIndex == 3 && ReferenceEquals(toggle.encounter, encounter));
        Check(new CombatToggleEventArgs(-1, -2, null!).encounter is null);
        CombatToggleEventDelegate toggled = (isImport, payload) => called = !isImport && ReferenceEquals(payload.encounter, encounter);
        called = false;
        toggled(false, toggle);
        Check(called);
        return "Combat events";
    }
}
