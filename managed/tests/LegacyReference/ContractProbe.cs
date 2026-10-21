using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

// Compile real member references against the original ACT metadata, not the replacement.
public static class ContractProbe
{
    private static void Check(bool condition) { if (!condition) throw new Exception("Original-reference Dnum contract probe failed"); }
    public static long Run()
    {
        var damage = new Dnum(123L);
        var labelled = new Dnum(456L, "explicit");
        Check(damage.Number == 123 && labelled.DamageString == "explicit");
        labelled.DamageString = "changed";
        Check(labelled.DamageString == "changed" && labelled.DamageString2 is null);
        labelled.DamageString2 = "suffix";
        Check(labelled.DamageString2 == "suffix");
        long number = damage;
        Dnum unknown = -11L, maximum = long.MaxValue, minimum = long.MinValue;
        Check(number == 123 && unknown.Number == -9);
        Check(maximum.Number == long.MaxValue && minimum.Number == -9);
        Check(new Dnum(long.MinValue).Number == long.MinValue);
        Check(Dnum.NoDamage.Number == 0 && Dnum.Miss.Number == -1 && Dnum.Unknown.Number == -9);
        Check(Dnum.Death.Number == -10 && Dnum.ThreatPosition.Number == -9);
        return number;
    }
}
