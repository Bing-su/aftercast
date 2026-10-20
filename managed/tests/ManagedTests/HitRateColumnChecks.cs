using System.Globalization;
using Advanced_Combat_Tracker;
using Bxivact.ActCompat;

internal static class HitRateColumnChecks
{
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("bxivact-hit-rate-");
        var commas = ActGlobals.mainTableShowCommas;
        var blockIsHit = ActGlobals.blockIsHit;
        var culture = CultureInfo.CurrentCulture;
        try
        {
            // Exercise the public host boundary: each acquired host installs its own base-column registrations.
            using var host = new HostEnvironment(directory.FullName);
            var column = DamageTypeData.ColumnDefs["ToHit"];
            Check(DamageTypeData.ColumnDefs.Count == 1 && column.Label == "ToHit" && !column.DefaultVisible);
            Check(column.SqlDataType == "FLOAT" && column.SqlDataName == "ToHit");
            var category = new DamageTypeData(true, "Fixture", null!);
            foreach (var cultureName in new[] { "en-US", "fr-FR" })
            {
                CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo(cultureName);
                Check(category.GetColumnByName("ToHit") == float.NaN.ToString());
                Check(column.GetSqlData(category) == float.NaN.ToString());
            }
            ActGlobals.blockIsHit = true;
            category.AddCombatAction(new(1, false, "None", new Dnum(100), DateTime.UnixEpoch, 1,
                "Strike", "Source", "Physical", "Target"), "All");
            category.AddCombatAction(new(1, false, "None", Dnum.Miss, DateTime.UnixEpoch, 2,
                "Strike", "Source", "Physical", "Target"), "All");
            foreach (var cultureName in new[] { "en-US", "fr-FR" })
            foreach (var showCommas in new[] { false, true })
            {
                CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo(cultureName);
                ActGlobals.mainTableShowCommas = showCommas;
                // These are ACT's genuine cell examples; the displayed rate has no literal percent suffix.
                Check(category.GetColumnByName("ToHit") == (cultureName == "en-US" ? "50.00" : "50,00"));
                Check(column.GetSqlData(category) == "50");
            }
            Check(category.GetColumnByName("tohit") == "");
        }
        finally
        {
            ActGlobals.mainTableShowCommas = commas;
            ActGlobals.blockIsHit = blockIsHit;
            CultureInfo.CurrentCulture = culture;
            directory.Delete(true);
        }
    }

    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Base hit-rate column contract failed");
    }
}
