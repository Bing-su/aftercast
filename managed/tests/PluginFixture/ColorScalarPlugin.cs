using System.Globalization;
using System.Windows.Forms;
using Advanced_Combat_Tracker;

namespace Aftercast.PluginFixture;

// Synthetic plugin: no sound, clipboard, external service or user data.
public sealed class ColorScalarPlugin : HierarchyXmlPlugin, IActPluginV1
{
    private FormActMain.ColorSettings settings=null!;
    private readonly List<string> events=[];
    private int owner;
    private readonly List<string> colorEvents=[];
    private void Ally(System.Drawing.Color value)
    {
        colorEvents.Add("ally:"+value.ToArgb().ToString(CultureInfo.InvariantCulture));
        if (value.ToArgb() == System.Drawing.Color.FromArgb(255, 17, 34, 51).ToArgb())
            settings.DataGridAllyText.ForeColorSetting = System.Drawing.Color.FromArgb(64, 8, 9, 10);
    }
    private void Personal(System.Drawing.Color value) => colorEvents.Add("personal:"+value.ToArgb().ToString(CultureInfo.InvariantCulture));
    private void Changed(object? sender, EventArgs args)
    {
        if (Environment.CurrentManagedThreadId != owner) throw new InvalidOperationException("color event wrong owner");
        events.Add(sender is CheckBox ? "invert" : sender is NumericUpDown ? "number" : throw new InvalidOperationException("unexpected sender"));
        if (sender is NumericUpDown numeric && numeric.Value == .12m) numeric.Value = .34m;
        if (sender is NumericUpDown precise && precise.Value == .13m) precise.Value = .345m;
        if (sender is NumericUpDown failure && failure.Value == .56m) throw new InvalidOperationException("synthetic color subscriber failure");
    }
    public new void InitPlugin(TabPage page, Label status)
    {
        base.InitPlugin(page,status); owner=Environment.CurrentManagedThreadId;
        settings=ActGlobals.oFormActMain!.ActColorSettings;
        settings.InvertColorsChanged+=Changed; settings.LuminosityOffsetChanged+=Changed; settings.SaturationOffsetChanged+=Changed;
        settings.DataGridAllyText.ForeColorSettingChanged+=Ally; settings.DataGridPersonalBackground.ForeColorSettingChanged+=Personal;
        CombatantData.ColumnDefs["color-controls-probe"]=new("Control probe",false,"TEXT","color_controls",_=>ControlState(),_=>ControlState(),(_,_)=>0);
        CombatantData.ColumnDefs["color-probe"]=new("Color probe",false,"TEXT","color_probe",_=>State(),_=>State(),(_,_)=>0);
    }
    private string ControlState() => string.Join("|",settings.DataGridAllyText.ForeColorSetting.ToArgb(),settings.DataGridPersonalBackground.ForeColorSetting.ToArgb(),string.Join(",",colorEvents));
    private string State() => string.Join("|",settings.InvertColors,settings.LuminosityOffset.ToString(CultureInfo.InvariantCulture),settings.SaturationOffset.ToString(CultureInfo.InvariantCulture),string.Join(",",events));
    public new void DeInitPlugin()
    {
        settings.InvertColorsChanged-=Changed; settings.LuminosityOffsetChanged-=Changed; settings.SaturationOffsetChanged-=Changed;
        settings.DataGridAllyText.ForeColorSettingChanged-=Ally; settings.DataGridPersonalBackground.ForeColorSettingChanged-=Personal;
        CombatantData.ColumnDefs.Remove("color-controls-probe"); CombatantData.ColumnDefs.Remove("color-probe"); base.DeInitPlugin();
    }
}
