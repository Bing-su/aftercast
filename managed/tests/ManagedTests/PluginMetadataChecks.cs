using System.Windows.Forms;
using System.Reflection;
using System.Runtime.Loader;
using Advanced_Combat_Tracker;
using Aftercast.Bridge;

internal static class PluginMetadataChecks
{
    private sealed class Fixture(bool closing) : IActPluginV1
    {
        public void InitPlugin(TabPage tab, Label label)
        {
            var root = ActGlobals.oFormActMain!;
            if (OperatingSystem.IsWindows()) CheckNativeParent(tab);
            if (root.IsActClosing || root.GetVersion() != new Version(3, 8, 5, 288) ||
                root.ActPlugins.Single().pluginObj != this || root.PluginGetSelfData(this) != root.ActPlugins.Single())
                throw new Exception("Plugin metadata is not available during initialization");
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
        }
        finally { directory.Delete(true); }
    }
}
