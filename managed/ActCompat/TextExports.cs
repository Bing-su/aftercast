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
        try { return BuildTextExport(encounter, format, null); }
        catch (Exception error) { WriteExceptionLog(error, string.Empty); return string.Empty; }
    }
    // The public ACT API retains its best-effort behavior; owned reports reuse framing with explicit limits/errors.
    private string BuildTextExport(EncounterData encounter, TextExportFormatOptions format, TextReportExecution? execution)
    {
        var playerFormat = format.PlayerFormat;
        var alliesFormat = format.AlliesFormat;
        var sorting = format.Sorting;
        var onlyAllies = format.ShowOnlyAllies;
        var alliedInfo = format.ShowAlliedInfo;
        List<CombatantData> actors;
        try { actors = new(encounter.Items.Values); }
        catch when (execution is null) { return string.Empty; }
        execution?.ValidateActors(actors.Count);
        try { actors.Sort(execution is null ? new CombatantData.DualComparison(sorting, sorting) : execution); actors.Reverse(); }
        catch (Exception error) when (execution is null) { WriteExceptionLog(error, "Export Sorting"); }
        var allies = onlyAllies || alliedInfo
            ? encounter.GetIsSelective() && encounter.GetIgnoreEnemies() ? new(encounter.Items.Values) : encounter.GetAllies()
            : new List<CombatantData>();
        if(onlyAllies||alliedInfo) execution?.BindAllies(allies,encounter.GetIsSelective()&&encounter.GetIgnoreEnemies());
        execution?.Check();
        var output = new StringBuilder();
        if (alliedInfo && allies.Count > 0)
            Append(alliesFormat, (key, extra) => execution is not null ? execution.Allied(key, encounter, allies, extra)
                : EncounterData.ExportVariables.TryGetValue(key, out var formatter)
                    ? formatter.GetExportString(encounter, allies, extra) : TextExportFormatterCap1 + key + TextExportFormatterCap2);
        foreach (var actor in actors)
        {
            execution?.Check();
            if (actor.Name == "Unknown" || onlyAllies && allies.Count > 0 && !allies.Contains(actor)) continue;
            Append(playerFormat, (key, extra) => execution is not null ? execution.Player(key, actor, extra)
                : CombatantData.ExportVariables.TryGetValue(key, out var formatter)
                    ? formatter.GetExportString(actor, extra) : TextExportFormatterCap1 + key + TextExportFormatterCap2);
        }
        // ACT retains at most 256 LF-delimited entries, including trailing empty entries.
        var lines = 0; var length = 0;
        foreach (var chunk in output.GetChunks())
            foreach (var character in chunk.Span)
            {
                length++;
                if (character == '\n' && ++lines == 256) return output.ToString(0, length);
            }
        execution?.Append("\n");
        return output.Append('\n').ToString();

        void Append(string template, Func<string, string, string> expand)
        {
            foreach (Match match in (execution?.Parser ?? TextExportFormatterRegex).Matches(template))
            {
                execution?.Check();
                var key = match.Groups["formatter"].Value;
                var value = key.Length == 0 ? match.Groups["text"].Value : expand(key, match.Groups["extra"].Value);
                execution?.Append(value); output.Append(value);
            }
        }
    }
}
