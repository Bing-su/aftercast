using System.Drawing;

namespace Advanced_Combat_Tracker;

public class TimerData(string name, string category) : IEquatable<TimerData>
{
    private string? categoryName = category;
    // shortcut: the default is ACT's built-in English category; use Trans when localized timer settings are supported.
    public string Category { get => string.IsNullOrEmpty(categoryName) ? " General" : categoryName; set => categoryName = value; }
    public string Name { get; set; } = name;
    public bool OnlyMasterTicks { get; set; }
    public bool RestrictToCategory { get; set; }
    public int RemoveValue { get; set; } = -15;
    public bool Panel1Display { get; set; } = true;
    public bool Panel2Display { get; set; }
    public Color FillColor { get; set; } = Color.Blue;
    public string Tooltip { get; set; } = "";
    public bool Modable { get; set; } = true;
    public bool ActiveInList { get; set; } = true;
    public bool RadialDisplay { get; set; } = true;
    public int TimerValue { get; set; } = 30;
    public int WarningValue { get; set; } = 10;
    public bool AbsoluteTiming { get; set; }
    public bool RestrictToMe { get; set; }
    public string StartSoundData { get; set; } = "";
    public string WarningSoundData { get; set; } = "";
    public string Key => Category.ToLower() + "|" + Name.ToLower();
    public override string ToString() => $"[{TimerValue}] {Name}";
    public bool Equals(TimerData? other) => Key.Equals(other!.Key);
    public override bool Equals(object? other) => Equals((TimerData?)other);
    public override int GetHashCode() => Key.GetHashCode();
}
