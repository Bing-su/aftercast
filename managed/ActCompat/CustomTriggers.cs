using System.Xml;
using System.Security;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    private readonly object triggerGate = new();
    public SortedList<string, CustomTrigger> CustomTriggers { get; } = new();
    public SortedList<string, CustomTrigger> ActiveCustomTriggers { get; } = new();
    public bool OriginalLoglineCustomTriggers { get; set; }

    public void AddEditCustomTrigger(CustomTrigger trigger)
    {
        ArgumentNullException.ThrowIfNull(trigger);
        lock (triggerGate) CustomTriggers[trigger.Key] = trigger;
        RebuildActiveCustomTriggers();
    }

    public void RebuildActiveCustomTriggers()
    {
        var zone = CurrentZone.ToLower();
        lock (triggerGate)
        {
            ActiveCustomTriggers.Clear();
            foreach (var (key, trigger) in CustomTriggers)
                if (trigger.Active && (!trigger.RestrictToCategoryZone || zone.Contains(trigger.Category.ToLower())))
                    ActiveCustomTriggers.Add(key, trigger);
        }
    }

    internal void ProcessCustomTriggers(string original, LogLineEventArgs info, bool isImport)
    {
        // Imports have no audio or live timer side effects; tabbed result rendering belongs to the future UI.
        if (isImport || IsActClosing) return;
        RebuildActiveCustomTriggers();
        CustomTrigger[] triggers;
        lock (triggerGate) triggers = ActiveCustomTriggers.Values.ToArray();
        var text = OriginalLoglineCustomTriggers ? original : info.logLine;
        foreach (var trigger in triggers)
            try
            {
                var match = trigger.RegEx.Match(text);
                if (!match.Success) continue;
                var captures = trigger.RegEx.GetGroupNames().ToDictionary(name => name, name => match.Groups[name].Value);
                if (captures.TryGetValue("YOU", out var you) && !you.ToLowerInvariant().Contains((ActGlobals.charName ?? "").ToLowerInvariant())) continue;
                if (trigger.Timer)
                {
                    string Capture(string name) => captures.TryGetValue(name, out var value) && value.Length > 0 ? value : "None";
                    ActGlobals.oFormSpellTimers?.NotifySpell(Capture("attacker").ToLower(), trigger.TimerName, true, Capture("victim"), true, captures);
                }
                if (DateTime.Now - trigger.LastAudioAlert <= TimeSpan.FromSeconds(1)) continue;
                switch (trigger.SoundType)
                {
                    case 1: Beep(); break;
                    case 2: if (File.Exists(trigger.SoundData)) PlaySound(trigger.SoundData); else Beep(); break;
                    case 3: TTS(match.Result(trigger.SoundData)); break;
                }
                trigger.LastAudioAlert = DateTime.Now;
            }
            catch (Exception error) { WriteExceptionLog(error, "Custom trigger: " + trigger.Key); }
    }

    internal string ShareCustomTriggerToXml(CustomTrigger trigger)
    {
        static string Escape(string value) => SecurityElement.Escape(value)!;
        var regex = Escape(trigger.ShortRegexString).Replace("&apos;", "'").Replace("#", "&#35;").Replace(@"\\", "&#92;&#92;").Replace(@"\s", "&#92;s");
        return $"<Trigger R=\"{regex}\" SD=\"{Escape(trigger.SoundData)}\" ST=\"{trigger.SoundType}\" CR=\"{(trigger.RestrictToCategoryZone ? "T" : "F")}\" C=\"{Escape(trigger.Category)}\" T=\"{(trigger.Timer ? "T" : "F")}\" TN=\"{Escape(trigger.TimerName)}\" Ta=\"{(trigger.Tabbed ? "T" : "F")}\" />";
    }

    internal void SaveXmlCustomTriggers(XmlTextWriter writer)
    {
        writer.WriteStartElement("CustomTriggers");
        lock (triggerGate) foreach (var trigger in CustomTriggers.Values) WriteTrigger(writer, trigger);
        writer.WriteEndElement();
    }

    private static void WriteTrigger(XmlTextWriter writer, CustomTrigger trigger)
    {
        writer.WriteStartElement("Trigger");
        void Attribute(string name, object value) => writer.WriteAttributeString(name, value.ToString());
        Attribute("Active", trigger.Active);
        Attribute("Regex", trigger.ShortRegexString);
        Attribute("SoundData", trigger.SoundData);
        Attribute("SoundType", trigger.SoundType);
        Attribute("CategoryRestrict", trigger.RestrictToCategoryZone);
        Attribute("Category", trigger.Category);
        Attribute("Timer", trigger.Timer);
        Attribute("TimerName", trigger.TimerName);
        Attribute("Tabbed", trigger.Tabbed);
        writer.WriteEndElement();
    }
}
