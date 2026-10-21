using System.Globalization;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.Testing;

internal static class ExternalExportChecks
{
    internal static void Run()
    {
        if (!OperatingSystem.IsWindows()) throw new PlatformNotSupportedException("Native clipboard export needs Windows");
        // Restore the user's clipboard even when assertions fail; do not print its previous contents.
        var previous = Clipboard.GetDataObject();
        try
        {
            var main = ActGlobals.oFormActMain!;
            CombatFixture.ConfigureCategories();
            main.ActiveZone.Items[0].CharName = "Source";
            CombatantData.ColumnDefs["EncDPS"] = new("EncDPS", false, "FLOAT", "EncDPS", _ => "", _ => "",
                (a, b) => a.Damage.CompareTo(b.Damage));
            CombatantData.ExportVariables["n"] = new("n", "", "", (_, _) => "\n");
            CombatantData.ExportVariables["NAME5"] = new("NAME5", "", "", (a, _) => a.Name[..Math.Min(5, a.Name.Length)]);
            CombatantData.ExportVariables["encdps-*"] = new("encdps-*", "", "", (a, _) => a.Damage.ToString(CultureInfo.InvariantCulture));
            EncounterData.ExportVariables["duration"] = new("duration", "", "", (_, _, _) => "fixture duration");
            EncounterData.ExportVariables["title"] = new("title", "", "", (e, _, _) => e.Title);
            EncounterData.ExportVariables["encdps-*"] = new("encdps-*", "", "", (e, _, _) => e.Damage.ToString(CultureInfo.InvariantCulture));
            EncounterData.ExportVariables["maxhit-*"] = new("maxhit-*", "", "", (_, _, _) => "fixture maxhit");
            var total = 0L;
            foreach (var amount in new[] { 100L, 300L })
            {
                main.LastKnownTime = DateTime.UnixEpoch.AddSeconds(total);
                main.SetEncounter(main.LastKnownTime, "Source", "Target");
                main.AddCombatAction(new(1, false, "", new Dnum(amount), main.LastKnownTime, (int)amount,
                    "Hit", "Source", "Physical", "Target"));
                main.EndCombat(true);
                total += amount;
                var expected = $"(fixture duration) All: {total} fixture maxhit\nSourc | {total}\n";
                if (Clipboard.GetText() != expected) throw new Exception("External plugin did not export the cumulative All encounter");
            }
            main.BeforeClipboardSet += args => { args.ClipContent = "edited export"; args.CopyLocal = true; };
            main.SendToClipboard("initial export", false);
            if (Clipboard.GetText() != "edited export") throw new Exception("Clipboard hook edits were ignored");
            Console.WriteLine("Unmodified Export_All exported two ended encounters (100, then 400 cumulative damage); clipboard edits passed.");
        }
        finally
        {
            if (previous is null) Clipboard.Clear();
            else Clipboard.SetDataObject(previous, true);
        }
    }
}
