using System.Globalization;
using Aftercast.Testing;
using Advanced_Combat_Tracker;
using System.Windows.Forms;

namespace Aftercast.PluginFixture;

// Synthetic parser for recast snapshots; accepts only explicitly imported fixture text.
public class RecastImportPlugin : IActPluginV1
{
    private FormActMain? root;
    private FormActMain.DateTimeLogParser? previous;
    public void InitPlugin(TabPage tab, Label status)
    {
        root = ActGlobals.oFormActMain!;
        root.TimeStampLen = 0;
        previous = root.GetDateTimeFromLog;
        root.GetDateTimeFromLog = line => DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0], CultureInfo.InvariantCulture));
        CombatFixture.ConfigureCategories();
        CombatantData.DamageSwingTypes.Add(1);
        root.BeforeLogLineRead += Read;
        status.Text = "Synthetic recast parser ready";
    }
    private void Read(bool import, LogLineEventArgs info)
    {
        if (!import) throw new InvalidOperationException("Synthetic import only");
        var fields = info.logLine.Split('|');
        var amount = long.Parse(fields[1], CultureInfo.InvariantCulture);
        root!.SetEncounter(info.detectedTime, fields[3], fields[4]);
        root.AddCombatAction(new(1, false, "", new Dnum(amount), info.detectedTime,
            root.GlobalTimeSorter, fields[2], fields[3], "Physical", fields[4]));
    }
    public void DeInitPlugin()
    {
        root!.BeforeLogLineRead -= Read;
        root.GetDateTimeFromLog = previous!;
    }
}
