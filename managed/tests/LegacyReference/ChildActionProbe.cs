using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class ChildActionProbe
{
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Original-reference child action probe failed");
    }

    private static MasterSwing Swing(Dnum amount, int seconds = 0, bool critical = false) =>
        new(1, critical, "", amount, DateTime.UnixEpoch.AddSeconds(seconds), seconds, "Strike", "Source", "Physical", "Target");

    public static string Run()
    {
        var blockIsHit = ActGlobals.blockIsHit;
        var disableIncrementalCaching = ActGlobals.disableIncrementalCaching;
        try
        {
            ActGlobals.blockIsHit = true;
            ActGlobals.disableIncrementalCaching = false;
            var category = new DamageTypeData(true, "Fixture category", null!);
            Check(category.Damage == 0 && category.Hits == 0 && category.CritHits == 0 && category.Swings == 0 && category.Misses == 0 && category.Blocked == 0);
            Check(category.StartTime == DateTime.MaxValue && category.EndTime == DateTime.MinValue);
            var first = Swing(new Dnum(100), 2);
            var second = Swing(new Dnum(300), 0, true);
            category.AddCombatAction(first, "Strike");
            Check(category.Items.Count == 1 && category.Damage == 0);
            category.AddCombatAction(first, "All");
            category.AddCombatAction(second, "Strike");
            category.AddCombatAction(second, "All");
            Check(category.Items.Count == 2 && ReferenceEquals(category.Items["Strike"].Parent, category));
            Check(ReferenceEquals(category.Items["All"].Items[0], first));
            Check(category.Damage == 400 && category.Hits == 2 && category.CritHits == 1 && category.Swings == 2 && category.Misses == 0);
            Check(category.StartTime == DateTime.UnixEpoch && category.EndTime == DateTime.UnixEpoch.AddSeconds(2));

            var flags = new AttackType("Flags", null!);
            foreach (var amount in new[] { new Dnum(0), Dnum.Miss, new Dnum(-2), Dnum.Death, new Dnum(-1, "custom miss"), new Dnum(-10, "custom death") })
                flags.AddCombatAction(Swing(amount, critical: true));
            Check(flags.Damage == 0 && flags.Hits == 1 && flags.CritHits == 1 && flags.Swings == 5 && flags.Misses == 1 && flags.Blocked == 2);
            ActGlobals.blockIsHit = false;
            Check(flags.Hits == 1 && flags.CritHits == 1);
            flags.InvalidateCachedValues(true);
            Check(flags.Hits == 0 && flags.CritHits == 0);
            var killing = new AttackType("Killing", null!);
            killing.AddCombatAction(Swing(Dnum.Death));
            Check(killing.Swings == 1);

            var cached = new AttackType("Cached", null!);
            cached.AddCombatAction(Swing(new Dnum(5), 2));
            Check(cached.Damage == 5 && cached.StartTime == DateTime.UnixEpoch.AddSeconds(2));
            cached.Items[0] = Swing(new Dnum(50));
            cached.InvalidateCachedValues();
            cached.InvalidateCachedValues(false);
            Check(cached.Damage == 5 && cached.StartTime == DateTime.UnixEpoch.AddSeconds(2));
            cached.InvalidateCachedValues(true);
            Check(cached.Damage == 50 && cached.StartTime == DateTime.UnixEpoch);
            cached.AddCombatAction(Swing(new Dnum(4)));
            Check(cached.Damage == 54);
            cached.Items[0] = Swing(new Dnum(2));
            ActGlobals.disableIncrementalCaching = true;
            cached.AddCombatAction(Swing(new Dnum(3)));
            Check(cached.Damage == 9);

            ActGlobals.disableIncrementalCaching = false;
            category.Items["All"].Items[0] = Swing(new Dnum(200));
            category.InvalidateCachedValues(true);
            Check(category.Damage == 400);
            ActGlobals.disableIncrementalCaching = true;
            category.InvalidateCachedValues(true);
            Check(category.Damage == 500);

            ActGlobals.disableIncrementalCaching = false;
            var overflow = new AttackType("Overflow", null!);
            overflow.AddCombatAction(Swing(new Dnum(long.MaxValue)));
            overflow.AddCombatAction(Swing(new Dnum(1)));
            Check(overflow.Damage == long.MinValue);
            overflow.AddCombatAction(Swing(new Dnum(1)));
            Check(overflow.Damage == 1);
            return "Child actions";
        }
        finally
        {
            ActGlobals.blockIsHit = blockIsHit;
            ActGlobals.disableIncrementalCaching = disableIncrementalCaching;
        }
    }
}
