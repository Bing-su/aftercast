namespace Advanced_Combat_Tracker;

public partial class AttackType
{
    public float ToHit
    {
        get
        {
            // ACT catches malformed caller-owned data here; ordinary empty data still yields floating-point NaN.
            try { return (float)Hits / Swings * 100f; }
            catch (Exception) { return 0; }
        }
    }
}

public partial class DamageTypeData
{
    public float ToHit
    {
        get
        {
            // Preserve category-level failure containment as well as ACT's single-precision arithmetic.
            try { return (float)Hits / Swings * 100f; }
            catch (Exception) { return 0; }
        }
    }
}

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
