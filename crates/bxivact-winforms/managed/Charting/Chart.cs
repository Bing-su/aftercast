using System.ComponentModel;
using System.Drawing;

namespace System.Windows.Forms.DataVisualization.Charting;

// Resolve chart-bearing plugin types while retaining headless Control ownership and disposal.
public class Chart : Control, ISupportInitialize
{
    public SeriesCollection Series { get; } = new();
    public ChartAreaCollection ChartAreas { get; } = new();
    public LegendCollection Legends { get; } = new();
    public object? DataSource { get; set; }
    public ChartDashStyle BorderlineDashStyle { get; set; } = ChartDashStyle.NotSet;
    public Size Size { get; set; }
    // Native initialization batches invalidation; this metadata-only model has no invalidation pump.
    public void BeginInit() { }
    public void EndInit() { }
    public void DataBind() => throw new PlatformNotSupportedException("Linux chart data binding is not implemented");
}

public enum ChartValueType { Auto, Double, Single, Int32, Int64, UInt32, UInt64, String, DateTime, Date, Time, DateTimeOffset }
public enum ChartDashStyle { NotSet, Dash, DashDot, DashDotDot, Dot, Solid }

public abstract class ChartElement { }
public abstract class ChartNamedElement : ChartElement
{
    private string name = "";
    internal Func<string, bool>? NameAvailable;
    public virtual string Name
    {
        get => name;
        set
        {
            ArgumentNullException.ThrowIfNull(value);
            if (NameAvailable is not null && !NameAvailable(value))
                throw new ArgumentException("Chart element name already exists", nameof(value));
            name = value;
        }
    }
}

public class DataPointCustomProperties : ChartNamedElement
{
    public bool IsVisibleInLegend { get; set; } = true;
}

public class Series : DataPointCustomProperties
{
    public string XValueMember { get; set; } = "";
    public string YValueMembers { get; set; } = "";
    public ChartValueType XValueType { get; set; }
    public string ChartArea { get; set; } = "Default";
    public string Legend { get; set; } = "Default";
    public int YValuesPerPoint { get; set; } = 1;
}

public class ChartArea : ChartNamedElement
{
    public Axis AxisX { get; } = new();
    public Axis AxisY { get; } = new();
}
public class Axis : ChartElement
{
    public bool IsLabelAutoFit { get; set; } = true;
    public LabelStyle LabelStyle { get; } = new();
    public double Maximum { get; set; } = double.NaN;
}
public class LabelStyle : ChartElement { public string Format { get; set; } = ""; }
public class Legend : ChartNamedElement { public bool Enabled { get; set; } = true; }
