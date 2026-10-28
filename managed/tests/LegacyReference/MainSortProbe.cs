using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class MainSortProbe
{
    public static string Run()
    {
        var saved = MasterSwing.ColumnDefs;
        var primary = ActGlobals.aTSort;
        var secondary = ActGlobals.aTSort2;
        try
        {
            MasterSwing.ColumnDefs = new();
            ActGlobals.aTSort = ActGlobals.aTSort2 = "missing";
            var rows = Enumerable.Range(0, 24).Select(index => new MasterSwing(1, false, "None", new Dnum(1), DateTime.UnixEpoch, 0, index.ToString(), "A", "Damage", "B")).ToList();
            rows.Sort();
            rows.Reverse();
            return string.Join(",", rows.Select(row => row.AttackType));
        }
        finally { MasterSwing.ColumnDefs = saved; ActGlobals.aTSort = primary; ActGlobals.aTSort2 = secondary; }
    }
}
