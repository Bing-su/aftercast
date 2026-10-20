using System.Runtime.CompilerServices;
using Advanced_Combat_Tracker;

namespace Bxivact.LegacyReference;

// Exercise original metadata signatures without inventing aggregate encounter state.
public static class MasterSwingProbe
{
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Original-reference MasterSwing probe failed");
    }

    public static int Run()
    {
        var damage = new Dnum(1234567890123);
        var time = new DateTime(2026, 10, 20, 1, 2, 3, DateTimeKind.Utc);
        var action = new MasterSwing(7, true, "Direct Hit", damage, time, 42, "Attack", "Attacker", "Damage", "Victim");
        Check(action.SwingType == 7 && action.Critical && action.Special == "Direct Hit");
        Check(ReferenceEquals(action.Damage, damage) && action.Damage.Number == 1234567890123);
        Check(action.Time == time && action.Time.Kind == DateTimeKind.Utc && action.TimeSorter == 42);
        Check(action.AttackType == "Attack" && action.Attacker == "Attacker" && action.DamageType == "Damage" && action.Victim == "Victim");
        Check(action.ParentEncounter is null && action.Tags.Count == 0);
        var other = new MasterSwing(1, false, "None", damage, time, 43, "Other", "Source", "Type", "Target");
        Check(!ReferenceEquals(action.Tags, other.Tags));
        action.Critical = false;
        Check(!action.Critical);
        var encounter = (EncounterData)RuntimeHelpers.GetUninitializedObject(typeof(EncounterData));
        action.ParentEncounter = encounter;
        Check(ReferenceEquals(action.ParentEncounter, encounter));
        var marker = new object();
        var tags = new Dictionary<string, object> { ["marker"] = marker };
        action.Tags = tags;
        tags["later"] = 2;
        Check(ReferenceEquals(action.Tags, tags) && ReferenceEquals(action.Tags["marker"], marker) && (int)action.Tags["later"] == 2);
        var main = ActGlobals.oFormActMain!;
        var original = main.GlobalTimeSorter;
        try
        {
            main.GlobalTimeSorter = int.MaxValue;
            Check(main.GlobalTimeSorter == int.MaxValue);
            main.GlobalTimeSorter = int.MinValue;
            Check(main.GlobalTimeSorter == int.MinValue);
        }
        finally { main.GlobalTimeSorter = original; }
        return action.TimeSorter;
    }
}
