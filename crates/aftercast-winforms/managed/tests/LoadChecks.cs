using System.Diagnostics;
using System.Text.Json;
using System.Windows.Forms;
using System.Windows.Forms.DataVisualization.Charting;

internal static class LoadChecks
{
    internal static void Run()
    {
        using var panel = new Panel();
        var label = new Label { Name = "target" };
        panel.Controls.Add(label);
        Measure("control_lookup", 100000, () =>
        {
            if (!ReferenceEquals(panel.Controls["TARGET"], label)) throw new Exception("Control lookup changed");
        });
        using var chart = new Chart();
        var series = new Series { Name = "Network" };
        chart.Series.Add(series);
        Measure("chart_rename_lookup", 100000, () =>
        {
            series.Name = "Network";
            if (!ReferenceEquals(chart.Series["Network"], series)) throw new Exception("Chart lookup changed");
        });
        using var tree = new TreeView { PathSeparator = "/" };
        var leaf = tree.Nodes.Add("node");
        for (var index = 1; index < 128; index++) leaf = leaf.Nodes.Add("node");
        var expected = string.Join('/', Enumerable.Repeat("node", 128));
        Measure("tree_path_128", 1000, () =>
        {
            if (leaf.FullPath != expected) throw new Exception("Tree path changed");
        });
        Measure("leaf_disposal", 100000, () => new Control().Dispose());
    }

    private static void Measure(string scenario, int iterations, Action operation)
    {
        for (var index = 0; index < 200; index++) operation();
        var before = GC.GetAllocatedBytesForCurrentThread();
        var started = Stopwatch.GetTimestamp();
        for (var index = 0; index < iterations; index++) operation();
        var elapsed = Stopwatch.GetElapsedTime(started).TotalMilliseconds;
        var allocated = GC.GetAllocatedBytesForCurrentThread() - before;
        Console.WriteLine(JsonSerializer.Serialize(new
        {
            scenario, iterations, allocatedBytes = allocated, elapsedMilliseconds = elapsed
        }));
    }
}
