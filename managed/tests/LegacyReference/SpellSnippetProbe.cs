using System.Reflection;
using System.Runtime.CompilerServices;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class SpellSnippetProbe
{
    public static string Run()
    {
        var old = ActGlobals.oFormSpellTimers;
        var main = (FormActMain)RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        var timers = (FormSpellTimers)RuntimeHelpers.GetUninitializedObject(typeof(FormSpellTimers));
        GC.SuppressFinalize(main); GC.SuppressFinalize(timers);
        var flags = BindingFlags.Instance | BindingFlags.NonPublic;
        typeof(FormSpellTimers).GetField("timerDefs", flags)!.SetValue(timers, new SortedDictionary<string, TimerData>());
        try
        {
            ActGlobals.oFormSpellTimers = timers;
            var method = typeof(FormActMain).GetMethod("ShareXmlToSpell", flags)!;
            TimerData Convert(Dictionary<string,string> values) => (TimerData)method.Invoke(main, [values])!;
            void Check(bool ok) { if (!ok) throw new Exception("ACT Spell snippet contract differs"); }
            var defaults = Convert(new() { ["N"] = "Foo", ["C"] = "" });
            Check(defaults.Category == " General" && defaults.TimerValue == 30 && defaults.WarningValue == 10 && defaults.RemoveValue == -15 && defaults.Modable && defaults.RadialDisplay && defaults.Panel1Display);
            var timer = Convert(new() { ["N"]="FoO", ["C"]="Fixture", ["T"]="41", ["R"]="T", ["OM"]="T", ["A"]="T", ["WV"]="3", ["RD"]="F", ["M"]="F", ["Tt"]="<note>", ["SS"]="speak & test", ["WS"]="warn", ["FC"]="-1", ["RV"]="-7", ["RC"]="T" });
            Check(timer.Name == "FoO" && timer.Category == "Fixture" && timer.TimerValue == 41 && timer.RestrictToMe && timer.OnlyMasterTicks && timer.AbsoluteTiming && timer.WarningValue == 3 && !timer.RadialDisplay && !timer.Modable && timer.Tooltip == "<note>" && timer.StartSoundData == "speak & test" && timer.WarningSoundData == "warn" && timer.FillColor.ToArgb() == -1 && timer.RemoveValue == -7 && timer.RestrictToCategory);
            var invalid = Convert(new() { ["N"]="Bad", ["C"]="", ["T"]="bad", ["WV"]="bad", ["RV"]="bad", ["FC"]="bad" });
            Check(invalid.TimerValue == 30 && invalid.WarningValue == 10 && invalid.RemoveValue == -15 && invalid.FillColor.ToArgb() == -16776961);
            return "Spell snippet attributes and defaults";
        }
        finally { ActGlobals.oFormSpellTimers = old; }
    }
}
