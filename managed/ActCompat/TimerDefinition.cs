using System.Drawing;
using Advanced_Combat_Tracker;
using System.Xml.Serialization;

namespace Aftercast.ActCompat;

// Match ACT's Spell XML names; host JSON uses the descriptive property names.
public sealed record TimerDefinition
{
    [XmlAttribute("Name")] public string Name { get; init; } = "";
    [XmlAttribute("Category")] public string Category { get; init; } = " General";
    [XmlAttribute("OnlyMasterTicks")] public bool OnlyMasterTicks { get; init; } = false;
    [XmlAttribute("RestrictCategory")] public bool RestrictToCategory { get; init; } = false;
    [XmlAttribute("RemoveValue")] public int RemoveValue { get; init; } = -15;
    [XmlAttribute("Panel1")] public bool Panel1Display { get; init; } = true;
    [XmlAttribute("Panel2")] public bool Panel2Display { get; init; } = false;
    [XmlAttribute("FillColor")] public int FillColorArgb { get; init; } = -16776961;
    [XmlAttribute("Tooltip")] public string Tooltip { get; init; } = "";
    [XmlAttribute("Modable")] public bool Modable { get; init; } = true;
    [XmlAttribute("Checked")] public bool ActiveInList { get; init; } = true;
    [XmlAttribute("RadialDisplay")] public bool RadialDisplay { get; init; } = true;
    [XmlAttribute("Timer")] public int TimerValue { get; init; } = 30;
    [XmlAttribute("WarningValue")] public int WarningValue { get; init; } = 10;
    [XmlAttribute("Absolute")] public bool AbsoluteTiming { get; init; } = false;
    [XmlAttribute("Restrict")] public bool RestrictToMe { get; init; } = false;
    [XmlAttribute("StartWav")] public string StartSoundData { get; init; } = "";
    [XmlAttribute("WarningWav")] public string WarningSoundData { get; init; } = "";
    internal TimerData ToData()
    {
        var data = new TimerData(Name,Category);
        Apply(data);
        return data;
    }
    internal void Apply(TimerData data)
    {
        data.Name = Name;
        data.Category = Category;
        data.OnlyMasterTicks = OnlyMasterTicks;
        data.RestrictToCategory = RestrictToCategory;
        data.RemoveValue = RemoveValue;
        data.Panel1Display = Panel1Display;
        data.Panel2Display = Panel2Display;
        data.FillColor = Color.FromArgb(FillColorArgb);
        data.Tooltip = Tooltip;
        data.Modable = Modable;
        data.ActiveInList = ActiveInList;
        data.RadialDisplay = RadialDisplay;
        data.TimerValue = TimerValue;
        data.WarningValue = WarningValue;
        data.AbsoluteTiming = AbsoluteTiming;
        data.RestrictToMe = RestrictToMe;
        data.StartSoundData = StartSoundData;
        data.WarningSoundData = WarningSoundData;
    }
    internal static TimerDefinition FromData(TimerData data) => new()
    {
        Name = data.Name,
        Category = data.Category,
        OnlyMasterTicks = data.OnlyMasterTicks,
        RestrictToCategory = data.RestrictToCategory,
        RemoveValue = data.RemoveValue,
        Panel1Display = data.Panel1Display,
        Panel2Display = data.Panel2Display,
        FillColorArgb = data.FillColor.ToArgb(),
        Tooltip = data.Tooltip,
        Modable = data.Modable,
        ActiveInList = data.ActiveInList,
        RadialDisplay = data.RadialDisplay,
        TimerValue = data.TimerValue,
        WarningValue = data.WarningValue,
        AbsoluteTiming = data.AbsoluteTiming,
        RestrictToMe = data.RestrictToMe,
        StartSoundData = data.StartSoundData,
        WarningSoundData = data.WarningSoundData,
    };
}

public sealed record TimerItem(string Key, TimerDefinition Definition);
public sealed record SavedTimer(ulong Revision, TimerItem Item);
public sealed record TimerPage(ulong Revision, int Total, int RetainedFrames, TimerItem[] Items);

[XmlRoot("SpellTimers")]
public sealed class TimerDefinitionsFile
{
    [XmlElement("Spell")] public TimerDefinition[] Items { get; set; } = [];
}
