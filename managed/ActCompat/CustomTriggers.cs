using System.Xml;
using System.Security;
using System.Diagnostics;
using System.Text;
using Aftercast.ActCompat;

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
        lock (triggerGate) { CustomTriggers[trigger.Key] = trigger; RemoveTriggerResults(trigger.Key); }
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

    internal void ProcessCustomTriggers(string original, LogLineEventArgs info, bool isImport) =>
        ProcessCustomTriggerRecord(original, info, isImport, GlobalTimeSorter, info.logLine);

    private void ProcessCustomTriggerRecord(string original, LogLineEventArgs info, bool isImport, int sorter, string triggerText)
    {
        // ACT records tabbed results before suppressing import audio and live timer effects.
        if (IsActClosing || isImport && !ProcessImportCustomTriggers || triggerText.Length <= TimeStampLen + 1) return;
        RebuildActiveCustomTriggers();
        CustomTrigger[] triggers;
        lock (triggerGate) triggers = ActiveCustomTriggers.Values.ToArray();
        var text = OriginalLoglineCustomTriggers ? original : triggerText[TimeStampLen..];
        foreach (var trigger in triggers)
            try
            {
                var match = trigger.RegEx.Match(text);
                if (!match.Success) continue;
                var you = match.Groups["YOU"];
                if (trigger.RegEx.GroupNumberFromName("YOU") >= 0 && !you.Value.ToLowerInvariant().Contains((ActGlobals.charName ?? "").ToLowerInvariant())) continue;
                if (trigger.Tabbed) RecordTriggerResult(trigger, info, text, match, isImport, sorter);
                if (isImport) continue;
                if (trigger.Timer)
                {
                    var captures = trigger.RegEx.GetGroupNames().ToDictionary(name => name, name => match.Groups[name].Value);
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

    // Archive results are staged and committed only if matching completes within all budgets.
    private Func<long> archiveTriggerClock = Stopwatch.GetTimestamp;
    private sealed class StagedArchiveResults
    {
        internal readonly LinkedList<TriggerResult> Items = new();
        internal ulong Sequence, Omitted;
        internal int Characters;
        internal void Add(CustomTrigger trigger, LogLineEntry line, string text, System.Text.RegularExpressions.Match match)
        {
            var names = trigger.RegEx.GetGroupNames();
            long size = (long)trigger.Key.Length + trigger.Category.Length + trigger.TimerName.Length + text.Length;
            foreach (var name in names) size += name.Length + match.Groups[name].Length;
            var sequence = ++Sequence;
            if (size > 32768) { Omitted++; return; }
            var captures = names.Select(name => new TriggerCapture(name, match.Groups[name].Value)).ToArray();
            var result = new TriggerResult(sequence, trigger.Key, trigger.Category, trigger.TimerName, line.Time.Ticks, line.GlobalTimeSorter, true, text, captures);
            while (Items.Count >= 4096 || Characters + size > 4 * 1024 * 1024)
            { Characters -= ResultCharacters(Items.First!.Value); Items.RemoveFirst(); Omitted++; }
            Items.AddLast(result); Characters += (int)size;
        }
    }
    private StagedArchiveResults PrepareArchiveTriggers(IEnumerable<EncounterData> encounters)
    {
        var start = archiveTriggerClock();
        void CheckDeadline()
        {
            if (Stopwatch.GetElapsedTime(start, archiveTriggerClock()) > TimeSpan.FromSeconds(2))
                throw new TimeoutException("Archive trigger matching exceeded two seconds; no encounters or results were imported");
        }
        CustomTrigger[] triggers;
        lock (triggerGate)
        {
            if (CustomTriggers.Count > 1024) throw new InvalidDataException("Archive trigger matching supports at most 1024 registered triggers; no encounters or results were imported");
            var zone = CurrentZone.ToLower();
            triggers = CustomTriggers.Values.Where(trigger => trigger.Active &&
                (!trigger.RestrictToCategoryZone || zone.Contains(trigger.Category.ToLower()))).ToArray();
        }
        CheckDeadline();
        if (triggers.Length > 32) throw new InvalidDataException("Archive trigger matching supports at most 32 active triggers; no encounters or results were imported");
        var result = new StagedArchiveResults();
        long bytes = 0; int count = 0;
        foreach (var encounter in encounters)
            foreach (var line in encounter.LogLines)
            {
                CheckDeadline();
                if (++count > 10000) throw new InvalidDataException("Archive trigger matching supports at most 10000 log lines; no encounters or results were imported");
                bytes += Encoding.UTF8.GetByteCount(line.LogLine);
                if (bytes > 16 * 1024 * 1024) throw new InvalidDataException("Archive trigger matching supports at most 16 MiB of log text; no encounters or results were imported");
                if (line.LogLine.Length <= TimeStampLen + 1) continue;
                var text = OriginalLoglineCustomTriggers ? line.LogLine : line.LogLine[TimeStampLen..];
                foreach (var trigger in triggers)
                {
                    CheckDeadline();
                    System.Text.RegularExpressions.Match match;
                    try { match = trigger.RegEx.Match(text); }
                    catch (System.Text.RegularExpressions.RegexMatchTimeoutException)
                    { throw new TimeoutException("Archive trigger expression exceeded 100 milliseconds; no encounters or results were imported"); }
                    CheckDeadline();
                    if (!match.Success) continue;
                    var you = match.Groups["YOU"];
                    if (trigger.RegEx.GroupNumberFromName("YOU") >= 0 && !you.Value.ToLowerInvariant().Contains((ActGlobals.charName ?? "").ToLowerInvariant())) continue;
                    if (trigger.Tabbed) result.Add(trigger, line, text, match);
                }
            }
        CheckDeadline();
        return result;
    }
    private void CommitArchiveTriggers(StagedArchiveResults staged)
    {
        lock (triggerGate)
        {
            var baseline = triggerResultSequence;
            omittedTriggerResults += staged.Omitted;
            foreach (var item in staged.Items)
            {
                var size = ResultCharacters(item);
                while (triggerResults.Count >= 4096 || triggerResultCharacters + size > 4 * 1024 * 1024)
                { triggerResultCharacters -= ResultCharacters(triggerResults.First!.Value); triggerResults.RemoveFirst(); omittedTriggerResults++; }
                triggerResults.AddLast(item with { Sequence = baseline + item.Sequence });
                triggerResultCharacters += size;
            }
            triggerResultSequence += staged.Sequence;
        }
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
