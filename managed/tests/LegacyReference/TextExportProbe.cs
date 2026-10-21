using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text.RegularExpressions;
using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class TextExportProbe
{
    public static string Run()
    {
        // Bootstrap only the original export options, e.g. disabled selective export, without starting ACT's UI.
        var main = (FormActMain)RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        GC.SuppressFinalize(main);
        var previous = ActGlobals.oFormActMain;
        var columns = CombatantData.ColumnDefs;
        var playerFormats = CombatantData.ExportVariables;
        var groupFormats = EncounterData.ExportVariables;
        var controls = new List<Control>();
        try
        {
            ActGlobals.oFormActMain = main;
            foreach (var entry in new[] { ("opTextExports", "cbExportFilterSpace"), ("opSelectiveParsing", "rbSParseExport") })
            {
                var field = typeof(FormActMain).GetField(entry.Item1, BindingFlags.Instance | BindingFlags.NonPublic);
                if (field is null) continue;
                var options = RuntimeHelpers.GetUninitializedObject(field.FieldType);
                GC.SuppressFinalize(options);
                field.SetValue(main, options);
                var controlField = field.FieldType.GetField(entry.Item2, BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public)!;
                var control = (Control)Activator.CreateInstance(controlField.FieldType)!;
                controls.Add(control);
                controlField.SetValue(options, control);
            }
            main.TextExportFormatterRegex = new Regex("{(?<formatter>[^}:]+)(?::(?<extra>[^}]+))?}|(?<text>[^{]+)");
            main.TextExportFormatterCap1 = "{";
            main.TextExportFormatterCap2 = "}";
            CombatantData.ColumnDefs = new()
            {
                ["Name"] = new("Name", true, "TEXT", "Name", a => a.Name, a => a.Name,
                    (a, b) => string.CompareOrdinal(a.Name, b.Name))
            };
            CombatantData.ExportVariables = new()
            {
                ["name"] = new("name", "", "", (a, _) => a.Name),
                ["extra"] = new("extra", "", "", (_, extra) => extra),
                ["n"] = new("n", "", "", (_, _) => "\n")
            };
            EncounterData.ExportVariables = new() { ["group"] = new("group", "", "", (_, allies, _) => allies.Count.ToString()) };
            var zone = new ZoneData(DateTime.UnixEpoch, "Fixture", false, false, false);
            var encounter = new EncounterData("Alpha", "Fixture", zone);
            foreach (var name in new[] { "Alpha", "Unknown", "Zed" }) encounter.Items.Add(name, new CombatantData(name, encounter));
            void Check(string actual, string expected)
            {
                if (actual != expected) throw new Exception($"Text export mismatch: {actual.Replace("\n", "\\n")} != {expected.Replace("\n", "\\n")}");
            }
            Check(main.GetTextExport(encounter, new("[{name}:{extra:X}:{missing:discard}]", "Name", false, false, "")),
                "[Zed:X:{missing}][Alpha:X:{missing}]\n");
            Check(main.GetTextExport(encounter, new("{name}{n}", "Name", true, false, "")), "Zed\nAlpha\n\n");
            encounter.SetAllies(new() { encounter.Items["Alpha"] });
            Check(main.GetTextExport(encounter, new("{name}", "Name", true, true, "group:{group};")), "group:1;Alpha\n");
            Check(main.GetTextExport(encounter, new("", "Name", false, false, "")), "\n");
            var capped = main.GetTextExport(encounter, new(string.Join("\n", Enumerable.Repeat("line", 300)), "Name", false, false, ""));
            if (capped.Count(c => c == '\n') != 256) throw new Exception("Text export line cap mismatch");
            var first = new TextExportFormatOptions("ab", "sort", false, false, "c");
            var collision = new TextExportFormatOptions("a", "sort", false, false, "bc");
            if (!first.Equals(collision) || first.ToString() != "(sort) \"ab\"") throw new Exception("ACT format equality mismatch");
            return "Text exports";
        }
        finally
        {
            foreach (var control in controls) control.Dispose();
            ActGlobals.oFormActMain = previous;
            CombatantData.ColumnDefs = columns;
            CombatantData.ExportVariables = playerFormats;
            EncounterData.ExportVariables = groupFormats;
        }
    }
}
