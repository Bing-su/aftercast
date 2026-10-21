using System.Collections.Concurrent;
using System.Diagnostics;
using System.Runtime.ExceptionServices;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.Bridge;

// Record incomplete startup honestly: zero host diagnostics alone does not establish plugin readiness.
internal static class OverlayStackChecks
{
    internal static void Run(string ffxiv, string overlay, string cactbot, string state)
    {
        if (!OperatingSystem.IsWindows()) throw new PlatformNotSupportedException("This stack observation requires Windows");
        // Surface native Timer failures in the report instead of opening WinForms' unhandled-exception dialog.
        Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException);
        Platform.Prepare();
        state = Path.GetFullPath(state);
        Directory.CreateDirectory(state);
        var stages = new List<object>();
        var exceptions = new List<string>();
        var dialogs = new ConcurrentQueue<string>();
        EventHandler<FirstChanceExceptionEventArgs> observer = (_, e) =>
        {
            if (e.Exception is FileNotFoundException or FileLoadException or TypeLoadException or MissingMethodException or BadImageFormatException or PlatformNotSupportedException)
                lock (exceptions) if (exceptions.Count < 40) exceptions.Add(e.Exception.ToString());
        };
        var ownerThread = GetCurrentThreadId();
        var seenDialogs = new ConcurrentDictionary<IntPtr, bool>();
        using var closer = new System.Threading.Timer(_ => EnumThreadWindows(ownerThread, (window, _) =>
        {
            var className = new StringBuilder(64);
            GetClassName(window, className, className.Capacity);
            // Only native message boxes owned by this test thread; some OverlayPlugin errors have an empty caption.
            if (className.ToString() != "#32770") return true;
            if (!seenDialogs.TryAdd(window, true)) return true;
            var text = new List<string>();
            EnumChildWindows(window, (child, _) => { text.Add(ReadText(child)); return true; }, IntPtr.Zero);
            dialogs.Enqueue(string.Join("\n", text));
            PostMessage(window, 0x10, IntPtr.Zero, IntPtr.Zero);
            return true;
        }, IntPtr.Zero), null, 50, 50);
        var session = new PluginSession();
        session.Configure(state);
        AppDomain.CurrentDomain.FirstChanceException += observer;
        try
        {
            foreach (var (path, type) in new[] { (ffxiv, "FFXIV_ACT_Plugin.FFXIV_ACT_Plugin"), (overlay, "RainbowMage.OverlayPlugin.PluginLoader"), (cactbot, "Cactbot.PluginLoader") })
            {
                lock (exceptions) exceptions.Clear();
                var startDialog = dialogs.Count;
                var absolute = Path.GetFullPath(path);
                string? status = null, failure = null;
                var phase = "construct";
                try
                {
                    var plugin = session.Construct(absolute, type);
                    var id = session.Add(absolute, plugin);
                    phase = "initialize";
                    status = session.Initialize(id);
                    var label = ActGlobals.oFormActMain!.PluginGetSelfData((IActPluginV1)plugin)!.lblPluginStatus;
                    var elapsed = Stopwatch.StartNew();
                    phase = "pump";
                    // CEF is downloaded asynchronously; a brief generic lifecycle pump cannot establish readiness.
                    while (elapsed.Elapsed < TimeSpan.FromSeconds(2) || type == "RainbowMage.OverlayPlugin.PluginLoader" &&
                        elapsed.Elapsed < TimeSpan.FromSeconds(60) && label.Text == status && dialogs.Count == startDialog)
                    { session.Pump(); Thread.Sleep(10); }
                    status = label.Text;
                }
                catch (Exception error) { failure = error.ToString(); }
                string[] observed;
                lock (exceptions) observed = exceptions.ToArray();
                stages.Add(new { path = absolute, type, sha256 = Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(absolute))),
                    status, failure, failurePhase = failure is null ? null : phase,
                    dialogs = dialogs.Skip(startDialog).ToArray(), exceptions = observed });
                Console.WriteLine($"{type}: status={status}; failure={failure?.Split('\n')[0]}; dialogs={dialogs.Count - startDialog}");
            }
        }
        finally
        {
            stages.Add(new { hostDiagnostics = session.DiagnosticsSnapshot });
            try { session.Stop(); }
            catch (Exception error) { stages.Add(new { shutdownFailure = error.ToString() }); }
            AppDomain.CurrentDomain.FirstChanceException -= observer;
            var report = Path.Combine(state, "stack-observation.json");
            File.WriteAllText(report, JsonSerializer.Serialize(new { stages, scope = "Startup observation only; inspect status, failures and dialogs before claiming readiness" }, new JsonSerializerOptions { WriteIndented = true }));
            Console.WriteLine("Stack observation saved: " + report);
        }
    }
    private static string ReadText(IntPtr window)
    {
        var text = new StringBuilder(8192);
        GetWindowText(window, text, text.Capacity);
        return text.ToString();
    }
    private delegate bool WindowCallback(IntPtr window, IntPtr state);
    [DllImport("user32.dll")] private static extern bool EnumThreadWindows(uint thread, WindowCallback callback, IntPtr state);
    [DllImport("user32.dll")] private static extern bool EnumChildWindows(IntPtr parent, WindowCallback callback, IntPtr state);
    [DllImport("kernel32.dll")] private static extern uint GetCurrentThreadId();
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetWindowText(IntPtr window, StringBuilder text, int count);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetClassName(IntPtr window, StringBuilder text, int count);
    [DllImport("user32.dll")] private static extern bool PostMessage(IntPtr window, uint message, IntPtr wparam, IntPtr lparam);
}
