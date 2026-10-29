using System.Windows.Forms;
using System.Drawing;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    private readonly ColorControl colorAlly = new() { ForeColorSetting = Color.ForestGreen };
    private readonly ColorControl colorPersonal = new() { ForeColorSetting = Color.FromArgb(200,255,200) };
    private readonly CheckBox colorInvert = new();
    private readonly NumericUpDown colorLuminosity = new() { Minimum = -1m, Maximum = 1m };
    private readonly NumericUpDown colorSaturation = new() { Minimum = -1m, Maximum = 1m };

    // Original API exposes a nested ColorSettings instance and forwards handlers to its controls.
    public ColorSettings ActColorSettings { get; } = new();
    public class ColorSettings
    {
        private static FormActMain Main => ActGlobals.oFormActMain ?? throw new InvalidOperationException("ACT host is unavailable");
        public ColorControl DataGridAllyText => Main.colorAlly;
        public ColorControl DataGridPersonalBackground => Main.colorPersonal;
        public bool InvertColors => Main.colorInvert.Checked;
        public decimal LuminosityOffset => Main.colorLuminosity.Value;
        public decimal SaturationOffset => Main.colorSaturation.Value;
        public event EventHandler InvertColorsChanged { add => Main.colorInvert.CheckedChanged += value; remove => Main.colorInvert.CheckedChanged -= value; }
        public event EventHandler LuminosityOffsetChanged { add => Main.colorLuminosity.ValueChanged += value; remove => Main.colorLuminosity.ValueChanged -= value; }
        public event EventHandler SaturationOffsetChanged { add => Main.colorSaturation.ValueChanged += value; remove => Main.colorSaturation.ValueChanged -= value; }
    }
    internal void DisposeColorScalars()
    {
        var errors = new List<Exception>();
        foreach (var control in new Control[] { colorInvert, colorLuminosity, colorSaturation, colorAlly, colorPersonal })
            try { control.Dispose(); } catch (Exception error) { errors.Add(error); }
        if (errors.Count != 0) throw new AggregateException("Color controls disposal failed", errors);
    }
    internal void ApplyColorControls(Color ally, Color personal)
    {
        if (Environment.CurrentManagedThreadId != ownerThread) throw new InvalidOperationException("Color settings require the owner thread");
        ObjectDisposedException.ThrowIf(actionsClosed,this);
        if ((!ally.IsEmpty && ally.A != 255) || (!personal.IsEmpty && personal.A != 255)) throw new ArgumentException("Colors must be opaque or empty");
        colorAlly.ForeColorSetting = ally;
        colorPersonal.ForeColorSetting = personal;
    }
    internal void ApplyColorScalars(bool invert, decimal luminosity, decimal saturation)
    {
        if (Environment.CurrentManagedThreadId != ownerThread) throw new InvalidOperationException("Color settings require the owner thread");
        ObjectDisposedException.ThrowIf(actionsClosed,this);
        if (luminosity is < -1m or > 1m || saturation is < -1m or > 1m ||
            decimal.Round(luminosity,2) != luminosity || decimal.Round(saturation,2) != saturation)
            throw new ArgumentOutOfRangeException(nameof(luminosity),"Offsets must be hundredths between -1 and 1");
        colorInvert.Checked = invert;
        colorLuminosity.Value = luminosity;
        colorSaturation.Value = saturation;
    }
}
