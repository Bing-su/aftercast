using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.Bridge;
using Aftercast.Testing;

internal sealed class ExternalBenchmarkChecks
{
    private int beforeLog, onLog, beforeAction, afterAction;
    private readonly FormActMain root = ActGlobals.oFormActMain!;
    private void BeforeLog(bool import, LogLineEventArgs info) { beforeLog++; info.logLine += " edited"; }
    private void OnLog(bool import, LogLineEventArgs info)
    {
        onLog++;
        if (root.LastLogLine != info.logLine || !info.logLine.EndsWith(" edited"))
            throw new Exception("LastLogLine must follow BeforeLogLineRead edits before OnLogLineRead");
        root.SetEncounter(info.detectedTime, "Source", "Target");
        root.AddCombatAction(1, false, "None", "Source", "Hit", new Dnum(100), info.detectedTime, onLog, "Target", "Physical");
    }
    private void BeforeAction(bool import, CombatActionEventArgs info) { beforeAction++; Thread.Sleep(5); }
    private void AfterAction(bool import, CombatActionEventArgs info) { afterAction++; }
    internal static void Run(PluginSession session, IActPluginV1 plugin)
    {
        if (!OperatingSystem.IsWindows()) throw new PlatformNotSupportedException("Benchmark workflow requires native Windows controls");
        new ExternalBenchmarkChecks().Exercise(session, plugin);
    }
    private void Exercise(PluginSession session, IActPluginV1 plugin)
    {
        // Benchmark supplies instrumentation, so the fixture supplies the parser's category registrations.
        CombatFixture.ConfigureCategories();
        var tab = root.PluginGetSelfData(plugin)!.tpPluginSpace;
        var enabled = (CheckBox)tab.Controls.Find("cbEnabled", true).Single();
        var table = (ListView)tab.Controls.Find("lvBench", true).Single();
        var log = (RichTextBox)tab.Controls.Find("rtbLog", true).Single();
        ((NumericUpDown)tab.Controls.Find("nudLogThreshold", true).Single()).Value = 1;
        _ = root.Handle; _ = log.Handle;
        root.BeforeLogLineRead += BeforeLog; root.OnLogLineRead += OnLog;
        root.BeforeCombatAction += BeforeAction; root.AfterCombatAction += AfterAction;
        var path = Path.Combine(root.AppDataFolder.FullName, "benchmark-fixture.log");
        File.WriteAllText(path, "Fixture raw record\n");
        root.GetDateTimeFromLog = _ => new DateTime(2026, 1, 1);
        try
        {
            SetEnabled(enabled, true, "ENABLED");
            session.ImportLog(path);
            var elapsed = Stopwatch.StartNew();
            while (elapsed.Elapsed < TimeSpan.FromSeconds(2)) { session.Pump(); Thread.Sleep(10); }
            if (table.Items.Count != 4 || table.Items.Cast<ListViewItem>().Any(row => row.SubItems[4].Text != "1" || row.SubItems[7].Text != "0") ||
                !log.Text.Contains("BeforeCombatAction") || !log.Text.Contains("Fixture raw record edited"))
                throw new Exception("Benchmark did not count four handlers or append its slow-handler log");
            SetEnabled(enabled, false, "DISABLED");
            session.ImportLog(path);
            if (beforeLog != 2 || onLog != 2 || beforeAction != 2 || afterAction != 2 ||
                table.Items.Cast<ListViewItem>().Any(row => row.SubItems[4].Text != "1"))
                throw new Exception("Benchmark did not restore handlers exactly once");
            Console.WriteLine("Unmodified Benchmark counted four handlers, logged a slow action and restored all handlers through its native checkbox.");
        }
        finally
        {
            if (enabled.Checked) SetEnabled(enabled, false, "DISABLED");
            root.BeforeLogLineRead -= BeforeLog; root.OnLogLineRead -= OnLog;
            root.BeforeCombatAction -= BeforeAction; root.AfterCombatAction -= AfterAction;
        }
    }
    private static void SetEnabled(CheckBox control, bool value, string caption)
    {
        var closed = false;
        using var closer = new System.Windows.Forms.Timer { Interval = 20 };
        // Dismiss only this test thread's known plugin dialog; do not bypass the plugin's event handler.
        closer.Tick += (_, _) => EnumThreadWindows(GetCurrentThreadId(), (window, _) =>
        {
            var title = new StringBuilder(128);
            GetWindowText(window, title, title.Capacity);
            if (title.ToString() == caption) { PostMessage(window, 0x111, new IntPtr(1), IntPtr.Zero); closed = true; }
            return true;
        }, IntPtr.Zero);
        closer.Start();
        control.Checked = value;
        if (!closed) throw new Exception("Benchmark confirmation dialog did not run");
    }
    private delegate bool WindowCallback(IntPtr window, IntPtr state);
    [DllImport("user32.dll")] private static extern bool EnumThreadWindows(uint thread, WindowCallback callback, IntPtr state);
    [DllImport("kernel32.dll")] private static extern uint GetCurrentThreadId();
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetWindowText(IntPtr window, StringBuilder text, int count);
    [DllImport("user32.dll")] private static extern bool PostMessage(IntPtr window, uint message, IntPtr wparam, IntPtr lparam);
}
