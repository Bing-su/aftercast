using System.Text;
using System.Text.RegularExpressions;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    private readonly TextExportFormatOptions defaultTextFormat = new("{n}{NAME5} | {encdps-*}", "EncDPS", true, true,
        "({duration}) {title}: {encdps-*} {maxhit-*}");
    public List<TextExportFormatOptions> TextExportFormats { get; } = new();
    public Regex TextExportFormatterRegex { get; set; } = new("{(?<formatter>[^}:]+)(?::(?<extra>[^}]+))?}|(?<text>[^{]+)");
    public string TextExportFormatterCap1 { get; set; } = "{";
    public string TextExportFormatterCap2 { get; set; } = "}";
    public void AddTextFormat(TextExportFormatOptions format)
    {
        if (!TextExportFormats.Contains(format)) TextExportFormats.Add(format);
    }
    public void RemoveTextFormat(int index) => TextExportFormats.RemoveAt(index);
    public string GetTextExport(int zoneIndex, int encounterIndex, int formatIndex) =>
        GetTextExport(zoneIndex, encounterIndex, formatIndex < 0 ? defaultTextFormat : TextExportFormats[formatIndex]);
    public string GetTextExport(int zoneIndex, int encounterIndex, TextExportFormatOptions format) =>
        GetTextExport(ZoneList[zoneIndex].Items[encounterIndex], format);

    public string GetTextExport(EncounterData encounter, TextExportFormatOptions format)
    {
        try
        {
            var playerFormat = format.PlayerFormat;
            var alliesFormat = format.AlliesFormat;
            var sorting = format.Sorting;
            var onlyAllies = format.ShowOnlyAllies;
            var alliedInfo = format.ShowAlliedInfo;
            List<CombatantData> actors;
            try { actors = new(encounter.Items.Values); }
            catch { return string.Empty; }
            try { actors.Sort(new CombatantData.DualComparison(sorting, sorting)); actors.Reverse(); }
            catch (Exception error) { WriteExceptionLog(error, "Export Sorting"); }
            var allies = onlyAllies || alliedInfo
                ? encounter.GetIsSelective() && encounter.GetIgnoreEnemies() ? new(encounter.Items.Values) : encounter.GetAllies()
                : new List<CombatantData>();
            var output = new StringBuilder();
            if (alliedInfo && allies.Count > 0)
                Append(alliesFormat, (key, extra) => EncounterData.ExportVariables.TryGetValue(key, out var formatter)
                    ? formatter.GetExportString(encounter, allies, extra) : TextExportFormatterCap1 + key + TextExportFormatterCap2);
            foreach (var actor in actors)
            {
                if (actor.Name == "Unknown" || onlyAllies && allies.Count > 0 && !allies.Contains(actor)) continue;
                Append(playerFormat, (key, extra) => CombatantData.ExportVariables.TryGetValue(key, out var formatter)
                    ? formatter.GetExportString(actor, extra) : TextExportFormatterCap1 + key + TextExportFormatterCap2);
            }
            // ACT retains at most 256 LF-delimited entries, e.g. empty output becomes "\n" and "a\n" becomes "a\n\n".
            var lines = 0;
            var length = 0;
            foreach (var chunk in output.GetChunks())
                foreach (var character in chunk.Span)
                {
                    length++;
                    if (character == '\n' && ++lines == 256) return output.ToString(0, length);
                }
            return output.Append('\n').ToString();

            void Append(string template, Func<string, string, string> expand)
            {
                foreach (Match match in TextExportFormatterRegex.Matches(template))
                {
                    var key = match.Groups["formatter"].Value;
                    output.Append(key.Length == 0 ? match.Groups["text"].Value : expand(key, match.Groups["extra"].Value));
                }
            }
        }
        catch (Exception error) { WriteExceptionLog(error, string.Empty); return string.Empty; }
    }
}
