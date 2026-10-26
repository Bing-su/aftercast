using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Aftercast.PluginFixture;

// Never release these callbacks: only killing the isolated worker can finish their lifetime.
public abstract class StuckPlugin(string mode) : IActPluginV1
{
    protected void Block()
    {
        File.WriteAllText(Path.Combine(ActGlobals.oFormActMain!.AppDataFolder.FullName, "stuck.entered"), mode);
        Thread.Sleep(Timeout.Infinite);
    }
    public void InitPlugin(TabPage tab, Label label)
    {
        if (mode == "init") Block();
        if (mode == "posted") SynchronizationContext.Current!.Post(_ => Block(), null);
        if (mode == "import") ActGlobals.oFormActMain!.BeforeLogLineRead += (_, _) => Block();
        if (mode == "action")
        {
            var parser = new ImportPlugin();
            parser.InitPlugin(tab, label);
            ActGlobals.oFormActMain!.AfterCombatAction += (_, _) => Block();
        }
    }
    public void DeInitPlugin() { if (mode == "deinit") Block(); }
}
public sealed class StuckInitPlugin() : StuckPlugin("init");
public sealed class StuckPostedPlugin() : StuckPlugin("posted");
public sealed class StuckImportPlugin() : StuckPlugin("import");
public sealed class StuckActionPlugin() : StuckPlugin("action");
public sealed class StuckDeinitPlugin() : StuckPlugin("deinit");
public sealed class StuckDisposePlugin() : StuckPlugin("dispose"), IDisposable
{
    public void Dispose() => Block();
}
