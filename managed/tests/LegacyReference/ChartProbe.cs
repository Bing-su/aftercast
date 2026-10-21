using System.ComponentModel;
using System.Windows.Forms;
using System.Windows.Forms.DataVisualization.Charting;

namespace Aftercast.LegacyReference;

// Bind real net48 chart MemberRefs, especially generic collection inheritance and base properties.
public static class ChartProbe
{
    public static string Run()
    {
        using var chart = new Chart();
        ((ISupportInitialize)chart).BeginInit();
        chart.BorderlineDashStyle = ChartDashStyle.Solid;
        chart.ChartAreas.Add(new ChartArea { Name = "Area" });
        chart.ChartAreas["Area"].AxisX.IsLabelAutoFit = false;
        chart.ChartAreas["Area"].AxisX.LabelStyle.Format = "HH:mm";
        chart.ChartAreas["Area"].AxisY.Maximum = 100;
        chart.Legends.Add(new Legend { Name = "Legend", Enabled = false });
        var series = new Series { Name = "Network", ChartArea = "Area", Legend = "Legend", IsVisibleInLegend = false, YValuesPerPoint = 1 };
        chart.Series.Add(series);
        chart.Series["Network"].XValueMember = "Time";
        chart.Series["Network"].YValueMembers = "Bytes";
        chart.Series["Network"].XValueType = ChartValueType.DateTime;
        chart.Size = new System.Drawing.Size(300, 200);
        chart.DataSource = new object();
        ((ISupportInitialize)chart).EndInit();
        Control control = chart;
        control.Text = "Status";
        if (chart.Series.Count != 1 || chart.Series["Network"].XValueType != ChartValueType.DateTime || control.Text != "Status")
            throw new Exception("Original chart reference did not bind to the headless model");
        try { chart.DataBind(); throw new Exception("Unsupported chart binding succeeded"); }
        catch (PlatformNotSupportedException) { }
        return chart.Series["Network"].Name;
    }
}
