using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Bxivact.PluginFixture;

// Exercise worker-to-owner dispatch through the native host, including deinitialization with a pending Send.
public class PumpPlugin : IActPluginV1
{
    private LogLineEventDelegate? handler;
    private FormActMain.DateTimeLogParser? previousParser;
    private string folder = "";
    public void InitPlugin(TabPage tab, Label status)
    {
        var root = ActGlobals.oFormActMain!;
        folder = root.AppDataFolder.FullName;
        var owner = Environment.CurrentManagedThreadId;
        previousParser = root.GetDateTimeFromLog;
        root.GetDateTimeFromLog = _ => DateTime.UnixEpoch;
        var path = Path.Combine(folder, "pump.log");
        File.WriteAllText(path, "");
        handler = (_, info) =>
        {
            File.WriteAllText(Path.Combine(folder, info.originalLogLine + ".queued"), "queued");
            tab.Invoke((Action)(() =>
            {
                if (Environment.CurrentManagedThreadId != owner) throw new Exception("Callback escaped its owner thread");
                File.WriteAllText(Path.Combine(folder, info.originalLogLine + ".pumped"), "pumped");
            }));
            File.WriteAllText(Path.Combine(folder, info.originalLogLine + ".returned"), "returned");
        };
        root.BeforeLogLineRead += handler;
        root.LogFilePath = path;
        root.OpenLog(false, false);
        status.Text = "Ready";
    }
    public void DeInitPlugin()
    {
        var root = ActGlobals.oFormActMain!;
        root.BeforeLogLineRead -= handler;
        root.GetDateTimeFromLog = previousParser!;
        File.WriteAllText(Path.Combine(folder, "pump.deinit"), "deinit");
    }
}
