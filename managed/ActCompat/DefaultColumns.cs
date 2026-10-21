namespace Advanced_Combat_Tracker;

internal static class DefaultColumns
{
    internal static void Initialize()
    {
        // FFXIV's IncToHit callback chains this ACT base column, e.g. one hit and one miss display "50.00".
        DamageTypeData.ColumnDefs.Add("ToHit", new("ToHit", false, "FLOAT", "ToHit",
            static data => data.ToHit.ToString(ActGlobals.mainTableShowCommas ? "#,0.00" : "0.00"),
            static data => data.ToHit.ToString()));
    }
}
