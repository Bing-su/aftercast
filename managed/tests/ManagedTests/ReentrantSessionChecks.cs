using System.Reflection;
using System.Runtime.InteropServices;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Bxivact.Bridge;

internal static class ReentrantSessionChecks
{
    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception("Reentrant session check failed: " + message);
    }

    private sealed class Plugin(string path, Action stop) : IActPluginV1
    {
        internal readonly ManualResetEventSlim FirstDone = new();
        internal readonly ManualResetEventSlim SecondDone = new();
        internal int Deinitialized;
        internal bool Rejected;
        private FormActMain? root;
        private LogLineEventDelegate? handler;

        public void InitPlugin(TabPage tab, Label status)
        {
            root = ActGlobals.oFormActMain!;
            root.LogFilePath = path;
            root.GetDateTimeFromLog = _ => DateTime.UnixEpoch;
            handler = (_, info) =>
            {
                if (info.logLine == "second") { SecondDone.Set(); return; }
                tab.Invoke((Action)(() =>
                {
                    try { stop(); throw new Exception("Reentrant Stop unexpectedly succeeded"); }
                    catch (InvalidOperationException) { Rejected = true; }
                    Check(ReferenceEquals(root, ActGlobals.oFormActMain) && !root.IsDisposed && !tab.IsDisposed,
                        "rejected Stop changed globals or disposed controls");
                    Check(root.PluginGetSelfData(this) is not null, "rejected Stop removed plugin registration");
                }));
                FirstDone.Set();
            };
            root.BeforeLogLineRead += handler;
            root.OpenLog(false, false);
        }

        public void DeInitPlugin()
        {
            Deinitialized++;
            root!.BeforeLogLineRead -= handler;
        }
    }

    internal static void Run(long activeSession)
    {
        if (!OperatingSystem.IsLinux()) return;
        CheckSession(new PluginSession(), null);
        // Exercise the actual ABI session without adding a production seam or closing its outer lifecycle.
        var shared = (Lazy<PluginSession>)typeof(EntryPoint).GetField("plugins", BindingFlags.NonPublic | BindingFlags.Static)!.GetValue(null)!;
        CheckSession(shared.Value, activeSession);
        Console.WriteLine("Direct and ABI reentrant Stop rejection preserves registrations, globals and later log records.");
    }

    private static void CheckSession(PluginSession session, long? activeSession)
    {
        var directory = Directory.CreateTempSubdirectory("bxivact-reentrant-session-");
        var memory = Marshal.AllocHGlobal(24);
        Plugin? plugin = null;
        try
        {
            var path = Path.Combine(directory.FullName, "Network.log");
            File.WriteAllText(path, "");
            session.Configure(directory.FullName);
            void Stop()
            {
                if (activeSession is not { } id) { session.Stop(); return; }
                Marshal.WriteInt32(memory, 0, 24);
                Marshal.WriteInt32(memory, 4, 1);
                Marshal.WriteInt32(memory, 8, 3);
                Marshal.WriteInt32(memory, 12, 0);
                Marshal.WriteInt64(memory, 16, id);
                Check(EntryPoint.Invoke(memory, 24) == -1, "ABI Stop did not reject the dispatched wait");
                // Normalize the boundary status into the same assertion path as direct Stop.
                throw new InvalidOperationException("ABI Stop rejected the dispatched wait");
            }
            plugin = new Plugin(path, Stop);
            var instance = session.Add(path, plugin);
            session.Initialize(instance);
            File.AppendAllText(path, "first\n");
            Check(SpinWait.SpinUntil(() => { session.Pump(); return plugin.FirstDone.IsSet; }, TimeSpan.FromSeconds(5)),
                "reentrant Stop stranded the first reader callback");
            Check(plugin.Rejected, "Stop was not rejected");
            if (activeSession is not null)
            {
                Marshal.WriteInt32(memory, 8, 2);
                Check(EntryPoint.Invoke(memory, 24) == 0, "ABI rejection cleared the active session");
            }
            File.AppendAllText(path, "second\n");
            Check(plugin.SecondDone.Wait(TimeSpan.FromSeconds(5)), "rejected Stop prevented the next log record");
            session.Stop();
            Check(plugin.Deinitialized == 1 && ActGlobals.oFormActMain is null, "ordinary Stop failed after rejection");
        }
        finally
        {
            session.Stop();
            plugin?.FirstDone.Dispose();
            plugin?.SecondDone.Dispose();
            Marshal.FreeHGlobal(memory);
            directory.Delete(true);
        }
    }
}
