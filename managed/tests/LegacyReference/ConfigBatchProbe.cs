using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text;
using System.Xml;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class ConfigBatchProbe
{
    public static string Run()
    {
        var old = ActGlobals.oFormSpellTimers;
        var main = (FormActMain)RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        var timers = (FormSpellTimers)RuntimeHelpers.GetUninitializedObject(typeof(FormSpellTimers));
        GC.SuppressFinalize(main); GC.SuppressFinalize(timers);
        var flags = BindingFlags.Instance | BindingFlags.NonPublic;
        var triggers = new SortedList<string, CustomTrigger>();
        var trigger = new CustomTrigger("one", "Fixture") { Active = true, SoundType = 3, SoundData = "sound & value", Timer = true, TimerName = "Timer", Tabbed = true };
        triggers.Add(trigger.Key, trigger);
        typeof(FormActMain).GetField("<CustomTriggers>k__BackingField", flags)!.SetValue(main, triggers);
        var definitions = new SortedDictionary<string, TimerData>();
        var timer = new TimerData("One", "Fixture") { TimerValue = 45, WarningValue = 5, StartSoundData = "silent" };
        definitions.Add(timer.Key, timer);
        typeof(FormSpellTimers).GetField("timerDefs", flags)!.SetValue(timers, definitions);
        try
        {
            ActGlobals.oFormSpellTimers = timers;
            using var memory = new MemoryStream();
            using (var writer = new XmlTextWriter(memory, new UTF8Encoding(false)))
            {
                writer.WriteStartElement("Config");
                typeof(FormActMain).GetMethod("SaveXmlCustomTriggers", flags)!.Invoke(main, [writer]);
                typeof(FormActMain).GetMethod("SaveXmlSpellTimers", flags)!.Invoke(main, [writer, true, true]);
                writer.WriteEndElement(); writer.Flush();
            }
            var xml = new XmlDocument(); xml.LoadXml(Encoding.UTF8.GetString(memory.ToArray()));
            var t = (XmlElement)xml.SelectSingleNode("/Config/CustomTriggers/Trigger")!;
            var s = (XmlElement)xml.SelectSingleNode("/Config/SpellTimers/Spell")!;
            if (t.GetAttribute("Regex") != "one" || t.GetAttribute("Category") != "Fixture" || t.GetAttribute("SoundType") != "3" ||
                t.GetAttribute("SoundData") != "sound & value" || t.GetAttribute("Timer") != "True" || t.GetAttribute("Tabbed") != "True" ||
                s.GetAttribute("Name") != "One" || s.GetAttribute("Category") != "Fixture" || s.GetAttribute("Timer") != "45" ||
                s.GetAttribute("WarningValue") != "5" || s.GetAttribute("StartWav") != "silent" || s.GetAttribute("Checked") != "True")
                throw new Exception("ACT Config export attributes differ");
            return "Config trigger and spell export attributes";
        }
        finally { ActGlobals.oFormSpellTimers = old; }
    }
}
