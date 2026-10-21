using System.ComponentModel;
using System.Windows.Forms;
using System.Windows.Forms.DataVisualization.Charting;

internal static class ChartContracts
{
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Headless chart contract check failed");
    }
    private static void Rejected(Action action)
    {
        try { action(); throw new Exception("Invalid chart collection operation succeeded"); }
        catch (ArgumentException) { }
    }
    internal static void Run()
    {
        var assembly = typeof(Chart).Assembly;
        Check(assembly.GetName().Name == "System.Windows.Forms.DataVisualization");
        Check(assembly.GetName().Version == new Version(4, 0, 0, 0));
        Check(assembly.GetName().GetPublicKeyToken()!.Length == 0);
        Check(typeof(Chart).BaseType == typeof(Control));
        Check(assembly.GetReferencedAssemblies().All(reference => reference.Name is not ("Advanced Combat Tracker" or "FSharp.Core")));
        using var chart = new Chart();
        Check(chart.BorderlineDashStyle == ChartDashStyle.NotSet);
        var initializer = (ISupportInitialize)chart;
        initializer.BeginInit();
        var series = new Series { Name = "Network", XValueMember = "Time", YValueMembers = "Bytes", XValueType = ChartValueType.DateTime };
        chart.Series.Add(series);
        Check(ReferenceEquals(chart.Series["Network"], series));
        series.Name = "Renamed";
        Check(ReferenceEquals(chart.Series["Renamed"], series));
        Rejected(() => _ = chart.Series["Network"]);
        Rejected(() => chart.Series.Add(new Series { Name = "Renamed" }));
        using var second = new Chart();
        Rejected(() => second.Series.Add(series));
        Rejected(() => chart.Series.Add(null!));
        var other = new Series { Name = "Other" };
        chart.Series.Add(other);
        Rejected(() => other.Name = "Renamed");
        Check(other.Name == "Other" && chart.Series.Count == 2);
        chart.Series.Remove(series);
        second.Series.Add(series);
        chart.Series.Add(new Series());
        chart.Series.Add(new Series());
        Check(chart.Series[1].Name != chart.Series[2].Name);
        var replacement = new Series { Name = "Other" };
        chart.Series[0] = replacement;
        second.Series.Add(other);
        chart.Series.Clear();
        replacement.Name = "Replacement";
        second.Series.Add(replacement);
        chart.ChartAreas.Add(new ChartArea { Name = "Area" });
        chart.ChartAreas["Area"].AxisX.LabelStyle.Format = "HH:mm";
        chart.ChartAreas["Area"].AxisY.Maximum = 1024;
        chart.Legends.Add(new Legend { Name = "Legend", Enabled = false });
        chart.DataSource = new object();
        initializer.EndInit();
        Check(chart.ChartAreas["Area"].AxisX.LabelStyle.Format == "HH:mm" && chart.ChartAreas[0].AxisY.Maximum == 1024);
        Check(!chart.Legends["Legend"].Enabled && chart.DataSource is not null);
        try { chart.DataBind(); throw new Exception("Unsupported data binding succeeded"); }
        catch (PlatformNotSupportedException) { }
        var disposed = 0;
        chart.Disposed += (_, _) => disposed++;
        chart.Dispose();
        chart.Dispose();
        Check(chart.IsDisposed && disposed == 1);
        Console.WriteLine("Independent chart identity, metadata, name ownership and disposal checks passed.");
    }
}
