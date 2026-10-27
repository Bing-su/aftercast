using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Aftercast.PluginFixture;

// Exercise external root generation changes using only the synthetic worker's private log.
public sealed class CompanionPlugin : IActPluginV1
{
    private readonly LiveCombatPlugin parser = new();
    private FormActMain root = null!;
    public void InitPlugin(TabPage tab, Label status)
    {
        parser.InitPlugin(tab, status);
        root = ActGlobals.oFormActMain!;
        root.LastKnownTime = DateTime.UnixEpoch;
        root.OnLogLineRead += Read;
    }
    private void Read(bool import, LogLineEventArgs info)
    {
        if (!import && info.originalLogLine.EndsWith("|external", StringComparison.Ordinal))
        {
            root.OpenLog(false, false);
            File.WriteAllText(Path.Combine(root.AppDataFolder.FullName, "companion.external"), "done");
        }
    }
    public void DeInitPlugin() { root.OnLogLineRead -= Read; parser.DeInitPlugin(); }
}
