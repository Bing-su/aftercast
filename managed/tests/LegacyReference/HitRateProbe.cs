using Advanced_Combat_Tracker;

namespace Bxivact.LegacyReference;

// Compile the same hit-rate and column failure contracts against genuine ACT and the replacement.
public static class HitRateProbe
{
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Original-reference hit-rate probe failed");
    }

    public static string Run()
    {
        var blockIsHit = ActGlobals.blockIsHit;
        var disableIncrementalCaching = ActGlobals.disableIncrementalCaching;
        var columns = DamageTypeData.ColumnDefs;
        try
        {
            ActGlobals.blockIsHit = true;
            ActGlobals.disableIncrementalCaching = false;
            var category = new DamageTypeData(true, "Fixture", null!);
            var attack = new AttackType("All", category);
            Check(float.IsNaN(attack.ToHit) && float.IsNaN(category.ToHit));
            category.Items.Add("All", attack);
            Check(float.IsNaN(category.ToHit));
            void Add(Dnum amount) => attack.AddCombatAction(new(1, false, "None", amount,
                DateTime.UnixEpoch, attack.Items.Count, "Strike", "Source", "Physical", "Target"));
            Add(new Dnum(100));
            Check(attack.ToHit == 100 && category.ToHit == 100);
            Add(Dnum.Miss);
            Check(attack.ToHit == 50 && category.ToHit == 50);
            Add(new Dnum(-5));
            Check(attack.Blocked == 1 && attack.ToHit == 1f / 3 * 100 && category.ToHit == 1f / 3 * 100);
            Add(Dnum.NoDamage);
            Add(Dnum.Death);
            Check(attack.Swings == 4 && attack.ToHit == 50 && category.ToHit == 50);
            ActGlobals.blockIsHit = false;
            Check(attack.ToHit == 50); // Changing policy alone does not invalidate ACT's existing hit cache.
            attack.InvalidateCachedValues(true);
            Check(attack.ToHit == 25 && category.ToHit == 25);

            // Mutable caller data can fail during either counter read; ToHit contains that failure as zero.
            var broken = new AttackType("Broken", null!) { Items = null! };
            Check(broken.ToHit == 0);
            category.Items["All"] = null!;
            Check(category.ToHit == 0);
            category.Items["All"] = attack;

            DamageTypeData.ColumnDefs = new();
            Check(category.GetColumnByName("missing") == "");
            var nullFailed = false;
            try { _ = category.GetColumnByName(null!); }
            catch (ArgumentNullException) { nullFailed = true; }
            Check(nullFailed);
            var callbackFailure = new InvalidOperationException("Fixture column callback failure");
            DamageTypeData.ColumnDefs.Add("Throw", new("Throw", false, "FLOAT", "Throw",
                _ => throw callbackFailure, _ => "unused"));
            Exception? observed = null;
            try { _ = category.GetColumnByName("Throw"); }
            catch (Exception error) { observed = error; }
            Check(ReferenceEquals(observed, callbackFailure));
            return "Hit rates";
        }
        finally
        {
            ActGlobals.blockIsHit = blockIsHit;
            ActGlobals.disableIncrementalCaching = disableIncrementalCaching;
            DamageTypeData.ColumnDefs = columns;
        }
    }
}
