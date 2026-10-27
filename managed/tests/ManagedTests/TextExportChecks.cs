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
            foreach (var text in new[] { "", "a", "\n", "a\n", "a\r\n\nb", new string('x', 20000),
                string.Join('\n', Enumerable.Repeat("한글", 255)), string.Join('\n', Enumerable.Repeat("한글", 256)),
                string.Join('\n', Enumerable.Repeat("한글", 257)) })
            {
                // Compare ACT's split semantics, e.g. trailing empty lines count toward the 256-entry limit.
                var expected = string.Concat(text.Split('\n').Take(256).Select(line => line + "\n"));
                if (main.GetTextExport(empty, new(text, "missing", false, false, "")) != expected)
                    throw new Exception("Text export framing changed");
            }
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
        TextReportChecks.Run();
        TableReportChecks.Run();
        HierarchyXmlChecks.Run();
    }
}
