using System.Windows.Forms;
using System.Reflection;
using System.Runtime.Loader;
using Advanced_Combat_Tracker;
using Aftercast.Bridge;

internal static class PluginMetadataChecks
{
    private sealed class DisposableFixture(bool fail) : IActPluginV1, IDisposable
    {
        internal int Disposals, Deinitializations;
        public void InitPlugin(TabPage tab, Label label) { if (fail) throw new InvalidOperationException("fixture initialization failure"); }
        public void DeInitPlugin() => Deinitializations++;
        public void Dispose()
        {
            if (Deinitializations != (fail ? 0 : 1)) throw new Exception("Dispose ran before DeInitPlugin completed");
            Disposals++;
        }
    }
    private sealed class ReadyFixture : IActPluginV1
    {
        public void InitPlugin(TabPage tab, Label label)
        {
            if (ActGlobals.oFormActMain!.InitActDone) throw new Exception("Plugin startup was marked complete before initialization");
        }
        public void DeInitPlugin()
        {
            if (ActGlobals.oFormActMain!.IsActClosing && ActGlobals.oFormActMain.InitActDone)
                throw new Exception("Startup readiness survived shutdown");
        }
    }
    private sealed class Fixture(bool closing) : IActPluginV1
    {
        public void InitPlugin(TabPage tab, Label label)
        {
            var root = ActGlobals.oFormActMain!;
            if (OperatingSystem.IsWindows()) CheckNativeParent(tab);
            if (root.IsActClosing || root.GetVersion() != new Version(3, 8, 5, 288) ||
                root.ActPlugins.Single().pluginObj != this || root.PluginGetSelfData(this) != root.ActPlugins.Single())
                throw new Exception("Plugin metadata is not available during initialization");
            var data = root.PluginGetSelfData(this)!;
            if (!data.cbEnabled.Checked || data.lblPluginTitle.Text != data.pluginFile.Name)
                throw new Exception("Plugin enable/title controls were not initialized");
        }
        private static void CheckNativeParent(TabPage tab)
        {
            if (tab.Parent is not TabControl) throw new Exception("Native plugin settings pages require a TabControl parent");
        }
        public void DeInitPlugin()
        {
            var root = ActGlobals.oFormActMain!;
            if (root.IsActClosing != closing || root.ActPlugins.Single().pluginObj != this)
                throw new Exception("Plugin metadata or closing state was cleared before DeInitPlugin");
            if (root.PluginGetSelfData(this)!.cbEnabled.Checked) throw new Exception("Plugin stayed enabled during DeInitPlugin");
        }
    }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-plugin-metadata-");
        try
        {
            // Plugin contexts are process-owned and retain file handles; stage their inputs under disposable build output.
            var inputs = Directory.CreateDirectory(Path.GetFullPath(Path.Combine("target", "plugin-libs-fixture", Guid.NewGuid().ToString("N"))));
            var entry = Path.Combine(inputs.FullName, "entry.dll");
            File.Copy(typeof(PluginMetadataChecks).Assembly.Location, entry);
            var libraries = Directory.CreateDirectory(Path.Combine(inputs.FullName, "libs"));
            var dependency = Path.Combine(libraries.FullName, "Aftercast.Bridge.dll");
            File.Copy(typeof(PluginSession).Assembly.Location, dependency);
            var context = new PluginContext(entry, new byte[32]);
            var loaded = context.LoadFromAssemblyName(AssemblyName.GetAssemblyName(dependency));
            if (loaded.Location != dependency || AssemblyLoadContext.GetLoadContext(loaded) != context)
                throw new Exception("Shipped libs dependency did not retain its plugin load context");
            foreach (var closing in new[] { false, true })
            {
                var session = new PluginSession();
                session.Configure(directory.FullName);
                try
                {
                    var id = session.Add(typeof(Fixture).Assembly.Location, new Fixture(closing));
                    session.Initialize(id);
                    if (!closing)
                    {
                        session.Deinitialize(id);
                        if (ActGlobals.oFormActMain!.ActPlugins.Count != 0) throw new Exception("Plugin list retained a removed plugin");
                    }
                }
                finally { session.Stop(); }
            }
            var batch = new PluginSession();
            batch.Configure(directory.FullName);
            try
            {
                var first = batch.Add(typeof(ReadyFixture).Assembly.Location, new ReadyFixture());
                var second = batch.Add(typeof(ReadyFixture).Assembly.Location, new ReadyFixture());
                batch.Initialize(first);
                batch.Pump();
                if (ActGlobals.oFormActMain!.InitActDone) throw new Exception("Pending plugin did not hold the startup boundary");
                batch.Initialize(second);
                if (ActGlobals.oFormActMain.InitActDone) throw new Exception("Initialize marked readiness before the owner pump");
                batch.Pump();
                if (!ActGlobals.oFormActMain.InitActDone) throw new Exception("Completed batch did not become ready");
                var later = batch.Add(typeof(ReadyFixture).Assembly.Location, new ReadyFixture());
                if (ActGlobals.oFormActMain.InitActDone) throw new Exception("New plugin did not reopen startup admission");
                batch.Initialize(later);
                batch.Pump();
                if (!ActGlobals.oFormActMain.InitActDone) throw new Exception("Later plugin batch did not complete");
            }
            finally { batch.Stop(); }
            foreach (var fail in new[] { false, true })
            {
                var lifetime = new PluginSession();
                lifetime.Configure(directory.FullName);
                var fixture = new DisposableFixture(fail);
                try
                {
                    var id = lifetime.Add(typeof(DisposableFixture).Assembly.Location, fixture);
                    try { lifetime.Initialize(id); if (fail) throw new Exception("Fixture failure was lost"); }
                    catch (InvalidOperationException) when (fail) { }
                    if (!fail) lifetime.Deinitialize(id);
                }
                finally { lifetime.Stop(); }
                if (fixture.Disposals != 1 || fixture.Deinitializations != (fail ? 0 : 1))
                    throw new Exception("Plugin Dispose/DeInit order or at-most-once ownership failed");
            }
        }
        finally { directory.Delete(true); }
    }
}
