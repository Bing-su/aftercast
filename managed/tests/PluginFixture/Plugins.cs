using System.Reflection;
using System.Windows.Forms;
using System.Windows.Forms.DataVisualization.Charting;
using Advanced_Combat_Tracker;

namespace Bxivact.PluginFixture;

// Construction must resolve deployment dependencies without calling InitPlugin.
public class GoodPlugin : IActPluginV1
{
    public int Value { get; } = PluginDependency.Value.Number;
    public GoodPlugin() { if (Value != 42) throw new Exception("Dependency returned the wrong value"); }
    public void InitPlugin(TabPage tab, Label label) => throw new Exception("InitPlugin must not run during construction");
    public void DeInitPlugin() => throw new Exception("Uninitialized plugins must not receive DeInitPlugin");
}
public class ThrowingPlugin : IActPluginV1
{
    public ThrowingPlugin() => throw new InvalidOperationException("fixture constructor failure");
    public void InitPlugin(TabPage tab, Label label) { }
    public void DeInitPlugin() { }
}
public class NeedsArgument(int value) : IActPluginV1
{
    public int Value => value;
    public void InitPlugin(TabPage tab, Label label) { }
    public void DeInitPlugin() { }
}
public class GenericPlugin<T> : IActPluginV1
{
    public void InitPlugin(TabPage tab, Label label) { }
    public void DeInitPlugin() { }
}
public class UiPlugin : IActPluginV1
{
    public TabPage Control { get; } = new();
    public UiPlugin() { _ = Control.Handle; }
    public void InitPlugin(TabPage tab, Label label) { }
    public void DeInitPlugin() { }
}

// Persist callback evidence for Rust assertions, including registration and cleanup after errors.
public class LifecyclePlugin(string mode) : IActPluginV1
{
    private static IActPluginV1? failedInstance;
    private string folder = "";
    private (TabPage Tab, Label Label)? controls;
    public void InitPlugin(TabPage tab, Label status)
    {
        // Compile against the genuine WinForms reference and execute against the headless adapter.
        using var settings = new UserControl { Text = "Settings" };
        // Bind original button and event signatures through the plugin's shared Forms context.
        using var checkBox = new CheckBox { UseVisualStyleBackColor = true };
        using var button = new Button { Text = "Reset" };
        var changes = 0;
        EventHandler changed = (_, _) => changes++;
        checkBox.CheckedChanged += changed;
        checkBox.Checked = true;
        checkBox.CheckedChanged -= changed;
        EventHandler click = (_, _) => throw new Exception("Unexpected native click");
        button.Click += click;
        button.Click -= click;
        if (!checkBox.Checked || changes != 1) throw new Exception("Button contract failed");
        // Designer timers share the plugin context and component container, without a hidden UI pump.
        using var components = new System.ComponentModel.Container();
        var timer = new System.Windows.Forms.Timer(components) { Enabled = true, Interval = 1000 };
        EventHandler tick = (_, _) => throw new Exception("Unexpected headless timer callback");
        timer.Tick += tick;
        timer.Tick -= tick;
        components.Dispose();
        if (timer.Enabled || timer.Container is not null) throw new Exception("Timer contract failed");
        // Use original ListView signatures through the plugin load context as well as the direct probe.
        using var list = new ListView { View = View.Details };
        list.Columns.AddRange(new[] { new ColumnHeader { Text = "Network" } });
        var row = list.Items.Add(new ListViewItem(new[] { "Capture", "0" }));
        row.Name = "capture";
        list.Items["capture"].SubItems[1].Text = "42";
        if (list.Items["capture"].SubItems[1].Text != "42" || list.SelectedItems.Count != 0)
            throw new Exception("ListView contract failed");
        // Resolve the separate chart assembly through the plugin context and share its Control base type.
        using var chart = new Chart();
        chart.Series.Add(new Series { Name = "Capture", XValueType = ChartValueType.DateTime });
        Control graph = chart;
        graph.Text = "Capture status";
        if (chart.Series["Capture"].XValueType != ChartValueType.DateTime || graph.Text != "Capture status")
            throw new Exception("Chart contract failed");
        if (settings.IsDisposed || settings.Text != "Settings") throw new Exception("UserControl contract failed");
        PluginDependency.Value.CheckPermissions();
        var dependency = Path.Combine(Path.GetDirectoryName(GetType().Assembly.Location)!, "Bxivact.PluginDependency.dll");
        var embedded = Assembly.Load(File.ReadAllBytes(dependency));
        embedded.GetType("Bxivact.PluginDependency.Value")!.GetMethod("CheckPermissions")!.Invoke(null, []);
        var root = ActGlobals.oFormActMain!;
        if (failedInstance is not null && root.PluginGetSelfData(failedInstance) is not null)
            throw new Exception("Failed plugin registration leaked");
        failedInstance = null;
        var data = root.PluginGetSelfData(this) ?? throw new Exception("Plugin registration is missing during InitPlugin");
        if (!ReferenceEquals(data.tpPluginSpace, tab)) throw new Exception("Wrong plugin tab");
        if (!ReferenceEquals(data.lblPluginStatus, status)) throw new Exception("Wrong status label");
        if (data.pluginFile.FullName != GetType().Assembly.Location) throw new Exception("Wrong plugin file");
        folder = root.AppDataFolder.FullName;
        controls = (tab, status);
        EventHandler disposed = (_, _) =>
        {
            File.AppendAllText(Path.Combine(folder, mode + ".disposed"), "disposed\n");
            if (mode == "deinit") throw new Exception("fixture disposal failure");
        };
        tab.Disposed += disposed;
        status.Disposed += disposed;
        File.AppendAllText(Path.Combine(folder, mode + ".init"), "init\n");
        status.Text = "Ready";
        if (mode == "background")
        {
            var thread = new Thread(() => root.WriteExceptionLog(new InvalidOperationException("unrelated background diagnostic"), "background"));
            thread.Start();
            thread.Join();
        }
        if (mode == "diagnostic-burst")
        {
            // Stress the bounded UTF-8 diagnostic snapshot from a worker, independently of InitPlugin's owner errors.
            var thread = new Thread(() =>
            {
                for (var index = 0; index < 40; index++)
                    root.WriteExceptionLog(new Exception("한글\0" + new string('한', 9000)), $"burst-{index}");
            });
            thread.Start();
            thread.Join();
        }
        if (mode == "long") status.Text = new string('x', 5000);
        if (mode == "throw")
        {
            failedInstance = this;
            throw new Exception("fixture initialization failure");
        }
        if (mode == "logged")
        {
            failedInstance = this;
            root.WriteExceptionLog(new InvalidOperationException("fixture caught failure"), "fixture");
        }
    }
    public void DeInitPlugin()
    {
        if (controls is not { } own || own.Tab.IsDisposed || own.Label.IsDisposed)
            throw new Exception("Controls were disposed before DeInitPlugin");
        File.AppendAllText(Path.Combine(folder, mode + ".deinit"), "deinit\n");
        if (mode == "deinit") throw new Exception("fixture shutdown failure");
    }
}
public class ReadyPlugin() : LifecyclePlugin("ready");
public class InitFailurePlugin() : LifecyclePlugin("throw");
public class LoggedFailurePlugin() : LifecyclePlugin("logged");
public class ShutdownFailurePlugin() : LifecyclePlugin("deinit");
public class LongStatusPlugin() : LifecyclePlugin("long");
public class BackgroundLogPlugin() : LifecyclePlugin("background");
public class DiagnosticBurstPlugin() : LifecyclePlugin("diagnostic-burst");
