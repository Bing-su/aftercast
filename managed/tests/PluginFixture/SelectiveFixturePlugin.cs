using System.Globalization;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.Testing;

namespace Aftercast.PluginFixture;

public sealed class SelectiveFixturePlugin : IActPluginV1
{
    private FormActMain root = null!;
    public void InitPlugin(TabPage page, Label status)
    {
        root = ActGlobals.oFormActMain!;
        status.Text = root.SelectiveListGetMode().ToString();
        root.TimeStampLen = 0;
        root.GetDateTimeFromLog = line => DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0], CultureInfo.InvariantCulture));
        CombatFixture.ConfigureCategories();
        CombatantData.ColumnDefs["selective-name"] = new("Name", true, "TEXT", "name", actor => actor.Name, actor => actor.Name,
            (left, right) => string.CompareOrdinal(left.Name, right.Name));
        CombatantData.ExportVariables["selective-name"] = new("selective-name", "", "", (actor, _) => actor.Name);
        root.BeforeLogLineRead += Read;
    }
    private void Read(bool import, LogLineEventArgs line)
    {
        var values = line.logLine.Split('|');
        if (root.SetEncounter(line.detectedTime, values[1], values[2]))
            root.AddCombatAction(new MasterSwing(1, false, "None", new Dnum(100), line.detectedTime,
                root.GlobalTimeSorter, "Strike", values[1], "Physical", values[2]));
    }
    public void DeInitPlugin()
    {
        root.BeforeLogLineRead -= Read;
        CombatantData.ColumnDefs.Remove("selective-name");
        CombatantData.ExportVariables.Remove("selective-name");
    }
}
