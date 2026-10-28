using System.Drawing;
using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Aftercast.PluginFixture;

public sealed class CommasPlugin : IActPluginV1
{
    private readonly ColumnsPlugin inner = new();
    public void InitPlugin(TabPage page, Label status)
    {
        inner.InitPlugin(page, status);
        MasterSwing.ColumnDefs["format"] = new("Format", true, "TEXT", "Format",
            action => action.Damage.ToString(), _ => "sql", (left, right) => left.TimeSorter.CompareTo(right.TimeSorter));
        MasterSwing.ColumnDefs["literal"] = new("Literal", true, "TEXT", "Literal",
            _ => "custom:1,234", _ => "sql", (_, _) => 0);
    }
    public void DeInitPlugin()
    {
        MasterSwing.ColumnDefs.Remove("format");
        MasterSwing.ColumnDefs.Remove("literal");
        inner.DeInitPlugin();
    }
}
