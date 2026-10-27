using System.Globalization;
using System.Text;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.Testing;
namespace Aftercast.PluginFixture;

// All files below are synthetic synchronization markers inside this test Host's temporary data directory.
public sealed class ClipboardImportPlugin : IActPluginV1
{
    private FormActMain root = null!;
    private FormActMain.DateTimeLogParser previous = null!;
    private Encoding encoding = null!;
    private string path = "";
    private DateTime time;
    private bool gated;
    public void InitPlugin(TabPage tab, Label status)
    {
        root = ActGlobals.oFormActMain!; previous = root.GetDateTimeFromLog;
        encoding = root.LogEncoding; path = root.LogFilePath; time = root.LastKnownTime;
        root.LogEncoding = Encoding.ASCII; root.LogFilePath = Path.Combine(root.AppDataFolder.FullName, "synthetic-live.log");
        root.LastKnownTime = DateTime.UnixEpoch.AddDays(1); root.TimeStampLen = 0;
        root.GetDateTimeFromLog = line => line.StartsWith("U|", StringComparison.Ordinal) ? DateTime.MinValue
            : DateTime.UnixEpoch.AddSeconds(int.Parse(line.TrimStart('\ufeff').Split('|')[0], CultureInfo.InvariantCulture));
        CombatFixture.ConfigureCategories(); root.BeforeLogLineRead += Read; status.Text = "Synthetic Unicode clipboard parser";
    }
    private void Read(bool import, LogLineEventArgs info)
    {
        if (!import || root.LogFilePath != "") throw new InvalidOperationException("Expected pathless import");
        var parts = info.originalLogLine.Split('|');
        root.SetEncounter(info.detectedTime, "Source", "Target");
        root.AddCombatAction(new(1, false, "", new Dnum(long.Parse(parts[1], CultureInfo.InvariantCulture)), info.detectedTime,
            root.GlobalTimeSorter, "Strike", "Source", "Physical", "Target"));
        if (!gated && parts.Length > 2 && parts[2] == "gate")
        {
            gated = true; var ready = Path.Combine(root.AppDataFolder.FullName, "clipboard.ready");
            var release = Path.Combine(root.AppDataFolder.FullName, "clipboard.release");
            File.WriteAllText(ready, "synthetic gate");
            if (!SpinWait.SpinUntil(() => File.Exists(release), TimeSpan.FromSeconds(10))) throw new TimeoutException("Clipboard test gate timed out");
        }
    }
    public void DeInitPlugin()
    {
        root.BeforeLogLineRead -= Read; root.GetDateTimeFromLog = previous;
        root.LogEncoding = encoding; root.LogFilePath = path; root.LastKnownTime = time;
    }
}
