using System.Globalization;
using Aftercast.Testing;
using Advanced_Combat_Tracker;
using System.Windows.Forms;

namespace Aftercast.PluginFixture;

// Synthetic parser for owned avoidance worker contracts; never reads live input.
public class AvoidanceImportPlugin : IActPluginV1
{
    private FormActMain? root;
    private FormActMain.DateTimeLogParser? previousParser;
    public void InitPlugin(TabPage tab, Label status)
    {
        root = ActGlobals.oFormActMain!;
        root.TimeStampLen = 0;
        previousParser = root.GetDateTimeFromLog;
        root.GetDateTimeFromLog = line => DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0], CultureInfo.InvariantCulture));
        CombatFixture.ConfigureCategories();
        CombatantData.DamageSwingTypes.Add(1);
        CombatantData.SwingTypeToDamageTypeDataLinksOutgoing[1] = new() { "Damage", "Damage" };
        root.BeforeLogLineRead += Read;
        status.Text = "Synthetic avoidance parser ready";
    }
    private void Read(bool import, LogLineEventArgs info)
    {
        if (!import) throw new InvalidOperationException("Synthetic import only");
        var parts = info.logLine.Split('|');
        var amount = long.Parse(parts[1], CultureInfo.InvariantCulture);
        root!.SetEncounter(info.detectedTime, "Source", parts[2]);
        var label = parts.Length > 3 ? parts[3] : "";
        root.AddCombatAction(new(1, false, "", new Dnum(amount, label), info.detectedTime,
            root.GlobalTimeSorter, "Strike", "Source", "Physical", parts[2]));
    }
    public void DeInitPlugin()
    {
        root!.BeforeLogLineRead -= Read;
        root.GetDateTimeFromLog = previousParser!;
    }
}
