using Aftercast.Testing;
using System.Drawing;
using System.Globalization;
using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Aftercast.PluginFixture;

// Exercise the Rust import boundary using an fixture plugin with deterministic actions.
public class ImportPlugin : IActPluginV1
{
    protected virtual bool Live => false;
    private FormActMain? root;
    private FormActMain.DateTimeLogParser? previousParser;
    public void InitPlugin(TabPage tab, Label status)
    {
        root = ActGlobals.oFormActMain!;
        previousParser = root.GetDateTimeFromLog;
        root.GetDateTimeFromLog = line => DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0], CultureInfo.InvariantCulture));
        CombatFixture.ConfigureCategories();
        root.BeforeLogLineRead += Read;
        if (Live)
        {
            root.LogFilePath = Path.Combine(root.AppDataFolder.FullName, "live.log");
            File.WriteAllText(root.LogFilePath, "");
            root.OpenLog(false, false);
        }
        status.Text = "Import fixture ready";
    }
    private void Read(bool import, LogLineEventArgs info)
    {
        if (!import && !Live) throw new InvalidOperationException("Fixture expects import delivery");
        var amount = long.Parse(info.logLine.Split('|')[1], CultureInfo.InvariantCulture);
        root!.SetEncounter(info.detectedTime, "Source", "Target");
        root.AddCombatAction(new(1, false, "None", new Dnum(amount), info.detectedTime,
            root.GlobalTimeSorter, "Strike", "Source", "Physical", "Target"));
        // Let native tests wait for admission rather than assume observing raw input means an action was enqueued.
        if (Live) File.WriteAllText(Path.Combine(root.AppDataFolder.FullName, "live.accepted"), info.logLine);
    }
    public void DeInitPlugin()
    {
        root!.BeforeLogLineRead -= Read;
        root.GetDateTimeFromLog = previousParser!;
    }
}

public class LiveCombatPlugin : ImportPlugin
{
    protected override bool Live => true;
}
