using System.Text.RegularExpressions;

namespace Advanced_Combat_Tracker;

public class CustomTrigger
{
    private string category = " General";
    private int tabbedAge;
    public CustomTrigger(string regex, string category)
    {
        ShortRegexString = regex;
        RegEx = Compile(regex);
        this.category = category;
    }
    public CustomTrigger(string regex, int soundType, string soundData, bool timer, string timerName, bool tabbed)
        : this(regex, " General")
    {
        SoundType = soundType; SoundData = soundData; Timer = timer; TimerName = timerName; Tabbed = tabbed;
    }
    // Bound user expressions so a pathological trigger, e.g. (a+)+$, cannot stall log ingestion indefinitely.
    private static Regex Compile(string regex) => new(regex, RegexOptions.Compiled, TimeSpan.FromMilliseconds(100));
    public string Category { get => string.IsNullOrEmpty(category) ? " General" : category; set => category = value; }
    public string Key => category + "|" + ShortRegexString;
    public bool RestrictToCategoryZone { get; set; }
    public DateTime LastAudioAlert { get; set; } = DateTime.MinValue;
    public int TabbedAge { get => Math.Min(tabbedAge, 10); set => tabbedAge = value; }
    public int TabbedCurrentIndex { get; set; } = -1;
    public string TimerName { get; set; } = "";
    public string SoundData { get; set; } = "";
    public string ShortRegexString { get; set; }
    public bool Active { get; set; } = true;
    public bool Timer { get; set; }
    public int SoundType { get; set; }
    public bool Tabbed { get; set; }
    public string SoundTypeString => SoundType switch { 1 => "Beep", 2 => "Sound", 3 => "TTS", _ => "None" };
    public string TimerString => Timer || Tabbed ? TimerName : "<None>";
    // ACT compiles at construction; editing ShortRegexString alone does not replace this expression.
    public Regex RegEx { get; }
}
