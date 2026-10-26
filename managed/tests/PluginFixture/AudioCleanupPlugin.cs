using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Aftercast.PluginFixture;

// Release a backend destructor only through the owner's pump, e.g. native audio cleanup needs STA work.
public sealed class AudioCleanupPlugin : IActPluginV1
{
    private bool stopped;
    public void InitPlugin(TabPage tab, Label label)
    {
        var context = SynchronizationContext.Current!;
        var folder = ActGlobals.oFormActMain!.AppDataFolder.FullName;
        void Poll(object? state)
        {
            if (stopped) return;
            if (File.Exists(Path.Combine(folder, "audio.retiring")))
                File.WriteAllText(Path.Combine(folder, "audio.released"), "released");
            else context.Post(Poll, null);
        }
        context.Post(Poll, null);
    }
    public void DeInitPlugin() => stopped = true;
}
