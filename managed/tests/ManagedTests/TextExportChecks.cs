using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Testing;

internal static class TextExportChecks
{
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-text-export-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            var main = ActGlobals.oFormActMain!;
            CombatFixture.ConfigureCategories();
            var format = new TextExportFormatOptions("ab", "Name", false, false, "c");
            main.AddTextFormat(format);
            main.AddTextFormat(new("a", "Name", false, false, "bc"));
            if (main.TextExportFormats.Count != 1) throw new Exception("ACT format duplicates were not suppressed");
            var empty = main.ActiveZone.Items[0];
            if (main.GetTextExport(1, 0, 0) != "\n" || main.GetTextExport(empty, format) != "\n")
                throw new Exception("Empty encounter export did not retain ACT's final LF");
            main.RemoveTextFormat(0);
            var actor = new CombatantData("Source", empty);
            empty.Items.Add("SOURCE", actor);
            CombatantData.ExportVariables["fail"] = new("fail", "", "", (_, _) => throw new Exception("fixture formatter failure"));
            if (main.GetTextExport(empty, new("{fail}", "missing", false, false, "")) != "" || host.Diagnostics.Item1 != 1)
                throw new Exception("Formatter failure was lost or escaped the export boundary");
            if (main.GetTextExport(null!, null!) != "" || host.Diagnostics.Item1 != 2 ||
                main.GetTextExport(null!, format) != "" || host.Diagnostics.Item1 != 2)
                throw new Exception("Format validation no longer precedes the best-effort encounter copy");
            if (!OperatingSystem.IsWindows())
            {
                try { main.SendToClipboard("test", false); throw new Exception("Headless clipboard appeared to succeed"); }
                catch (PlatformNotSupportedException) { }
            }
        }
        finally { directory.Delete(true); }
        Console.WriteLine("Text format registration, empty output and callback failure checks passed.");
    }
}
