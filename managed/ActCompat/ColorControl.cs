using System.Drawing;
using System.Windows.Forms;

namespace Advanced_Combat_Tracker;

// ACT's color selector model; it provides no rendered settings surface on Linux.
public class ColorControl : UserControl
{
    public delegate void ColorSettingEventDelegate(Color newColor);
    private Color color = Color.Empty;
    public event ColorSettingEventDelegate? ForeColorSettingChanged;
    public Color ForeColorSetting
    {
        get => color;
        set { if (color == value) return; color=value; ForeColorSettingChanged?.Invoke(value); }
    }
    protected override void Dispose(bool disposing)
    {
        if (disposing) ForeColorSettingChanged=null;
        base.Dispose(disposing);
    }
}
