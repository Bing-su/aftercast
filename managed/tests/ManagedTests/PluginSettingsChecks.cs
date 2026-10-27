using System.Reflection;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.Bridge;

internal static class PluginSettingsChecks
{
    private sealed class Fixture(bool fail = false) : IActPluginV1, IDisposable
    {
        internal TabPage? Tab;
        internal int Deinitialized, Disposed;
        public void InitPlugin(TabPage tab, Label status)
        {
            Tab = tab;
            tab.Controls.Add(new Label { Text = "Fixture settings" });
            if (fail) throw new InvalidOperationException("settings fixture failure");
            status.Text = "Ready";
        }
        public void DeInitPlugin() => Deinitialized++;
        public void Dispose() => Disposed++;
    }
    private static void Check(bool condition, [System.Runtime.CompilerServices.CallerArgumentExpression(nameof(condition))] string? expression = null)
    { if (!condition) throw new Exception("Plugin settings check failed: " + expression); }
    private static void Reject(Action action)
    { try { action(); } catch (InvalidOperationException) { return; } catch (PlatformNotSupportedException) { return; } throw new Exception("Invalid plugin settings operation accepted"); }

    internal static void Run() => HostLifetimeChecks.RunBounded(() =>
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-plugin-settings-");
        var session = new PluginSession();
        try
        {
            session.Configure(directory.FullName);
            var capability = OperatingSystem.IsWindows() ? 1 : 0;
            var failed = new Fixture(true);
            var failedId = session.Add(Path.Combine(directory.FullName, "failed.dll"), failed);
            Reject(() => session.Initialize(failedId));
            Check(session.Status(failedId) == ((int)PluginState.Failed, capability));
            Reject(() => session.ShowSettings(failedId));
            Reject(() => session.Initialize(failedId));
            Check(failed.Tab!.IsDisposed && failed.Disposed == 1 && failed.Deinitialized == 0);
            Form? window = null;
            for (var count = 0; count < 3; count++)
            {
                var plugin = new Fixture();
                var id = session.Add(Path.Combine(directory.FullName, "good.dll"), plugin);
                Check(session.Status(id) == ((int)PluginState.Constructed, capability));
                Reject(() => session.ShowSettings(id));
                Check(session.Initialize(id) == "Ready");
                Check(session.Status(id) == ((int)PluginState.Initialized, OperatingSystem.IsWindows() ? 2 : 0));
                if (OperatingSystem.IsWindows()) window = NativeWindowContract(session, id, plugin.Tab!);
                else
                {
                    try { session.ShowSettings(id); throw new Exception("Headless settings claimed support"); }
                    catch (PlatformNotSupportedException error) { Check(error.Message.Contains("Windows", StringComparison.Ordinal)); }
                }
                session.Deinitialize(id);
                Check(session.Status(id) == ((int)PluginState.Stopped, capability));
                Reject(() => session.ShowSettings(id));
                Reject(() => session.Initialize(id));
                Check(plugin.Tab!.IsDisposed && plugin.Disposed == 1 && plugin.Deinitialized == 1);
            }
            session.Stop();
            Check(ActGlobals.oFormActMain is null && (window is null || window.IsDisposed));
            Check(failed.Disposed == 1 && failed.Deinitialized == 0);
            Console.WriteLine("Plugin settings capability, fresh-instance retry, repeated lifecycle and owner disposal passed.");
        }
        finally { session.Stop(); directory.Delete(true); }
    });

    private static Form NativeWindowContract(PluginSession session, long id, TabPage page)
    {
        var root = ActGlobals.oFormActMain!;
        Form Window() => (Form)typeof(FormActMain).GetField("pluginSettingsWindow", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(root)!;
        session.ShowSettings(id);
        var first = Window();
        Check(first.Visible && !first.IsDisposed && ReferenceEquals(((TabControl)page.Parent!).SelectedTab, page));
        session.ShowSettings(id);
        Check(ReferenceEquals(first, Window()));
        first.Close();
        Check(!first.IsDisposed && !first.Visible && session.Status(id).State == (int)PluginState.Initialized);
        session.ShowSettings(id);
        Check(ReferenceEquals(first, Window()) && first.Visible);
        page.Parent!.Controls.Remove(page);
        Check(session.Status(id).Settings == 1);
        Reject(() => session.ShowSettings(id));
        return first;
    }
}
