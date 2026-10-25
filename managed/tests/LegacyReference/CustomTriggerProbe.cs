using System.Reflection;
using System.Runtime.CompilerServices;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

// Compare the non-UI model and ACT's escaped share representation without starting its main window.
public static class CustomTriggerProbe
{
    public static string Run()
    {
        var trigger = new CustomTrigger(@"(?<YOU>Source)\s+#<(?<value>\d+)>", "Fixture")
        { SoundType = 3, SoundData = "Value ${value} & <alert>", RestrictToCategoryZone = true, Timer = true, TimerName = "Timer", Tabbed = true };
        foreach (var (type, expectedLabel) in new[] { (0, "None"), (1, "Beep"), (2, "Sound"), (3, "TTS") })
        {
            trigger.SoundType = type;
            if (trigger.SoundTypeString != expectedLabel) throw new Exception("Sound label differs: " + trigger.SoundTypeString);
        }
        trigger.SoundType = 3;
        var defaults = new CustomTrigger("x", 0, "", false, "", false);
        if (defaults.Category != " General" || defaults.TimerString != "<None>") throw new Exception("Trigger defaults differ: " + defaults.Category + "/" + defaults.TimerString);
        if (!trigger.Active || trigger.TabbedCurrentIndex != -1 || trigger.LastAudioAlert != DateTime.MinValue ||
            trigger.Key != "Fixture|" + trigger.ShortRegexString || trigger.TimerString != "Timer" || !trigger.RegEx.IsMatch("Source #<123>"))
            throw new Exception("Custom trigger model differs");
        trigger.TabbedAge = 100;
        if (trigger.TabbedAge != 10) throw new Exception("Trigger age is not capped");
        var expression = trigger.RegEx;
        trigger.ShortRegexString = "changed";
        if (!ReferenceEquals(expression, trigger.RegEx)) throw new Exception("ACT compiles expressions only at construction");
        trigger.ShortRegexString = expression.ToString();
        var root = (FormActMain)RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        GC.SuppressFinalize(root);
        var share = typeof(FormActMain).GetMethod("ShareCustomTriggerToXml", BindingFlags.NonPublic | BindingFlags.Instance)!;
        var xml = (string)share.Invoke(root, [trigger])!;
        const string expected = "<Trigger R=\"(?&lt;YOU&gt;Source)&#92;s+&#35;&lt;(?&lt;value&gt;\\d+)&gt;\" SD=\"Value ${value} &amp; &lt;alert&gt;\" ST=\"3\" CR=\"T\" C=\"Fixture\" T=\"T\" TN=\"Timer\" Ta=\"T\" />";
        if (xml != expected) throw new Exception("Trigger share XML differs: " + xml);
        return "Custom trigger models";
    }
}
