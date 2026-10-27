namespace Aftercast.ActCompat
{
    public sealed record TriggerCapture(string Name, string Value);
    public sealed record TriggerResult(ulong Sequence, string Key, string Category, string TabName, long TimeTicks,
        int TimeSorter, bool IsImport, string Line, TriggerCapture[] Captures);
    public sealed record TriggerResultPage(ulong LatestSequence, ulong Omitted, TriggerResult[] Items);
}

namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;
    using System.Text.RegularExpressions;

    public partial class FormActMain
    {
        private readonly LinkedList<TriggerResult> triggerResults = new();
        private ulong triggerResultSequence, omittedTriggerResults;
        private int triggerResultCharacters;
        private static int ResultCharacters(TriggerResult result) => result.Key.Length + result.Category.Length +
            result.TabName.Length + result.Line.Length + result.Captures.Sum(capture => capture.Name.Length + capture.Value.Length);
        private void RecordTriggerResult(CustomTrigger trigger, LogLineEventArgs info, string text, Match match, bool import, int sorter)
        {
            // Check group lengths before materializing Group.Value strings, e.g. nested captures of a large line.
            var names = trigger.RegEx.GetGroupNames();
            long size = (long)trigger.Key.Length + trigger.Category.Length + trigger.TimerName.Length + text.Length;
            foreach (var name in names) size += name.Length + match.Groups[name].Length;
            lock (triggerGate)
            {
                var sequence = ++triggerResultSequence;
                if (size > 32768) { omittedTriggerResults++; return; }
                var captures = names.Select(name => new TriggerCapture(name, match.Groups[name].Value)).ToArray();
                var result = new TriggerResult(sequence, trigger.Key, trigger.Category, trigger.TimerName,
                    info.detectedTime.Ticks, sorter, import, text, captures);
                while (triggerResults.Count >= 4096 || triggerResultCharacters + size > 4 * 1024 * 1024)
                {
                    triggerResultCharacters -= ResultCharacters(triggerResults.First!.Value);
                    triggerResults.RemoveFirst(); omittedTriggerResults++;
                }
                triggerResults.AddLast(result); triggerResultCharacters += (int)size;
            }
        }
        public TriggerResultPage ReadTriggerResults(ulong after, int limit)
        {
            if (limit is < 1 or > 16) throw new ArgumentOutOfRangeException(nameof(limit));
            lock (triggerGate) return new(triggerResultSequence, omittedTriggerResults,
                triggerResults.Where(result => result.Sequence > after).Take(limit).ToArray());
        }
        public void ClearTriggerResults()
        {
            lock (triggerGate) { triggerResults.Clear(); triggerResultCharacters = 0; }
        }
        private void RemoveTriggerResults(string key)
        {
            // ACT removes the previous result tab when a trigger is replaced/deleted.
            for (var node = triggerResults.First; node is not null;)
            {
                var next = node.Next;
                if (node.Value.Key == key) { triggerResultCharacters -= ResultCharacters(node.Value); triggerResults.Remove(node); }
                node = next;
            }
        }
    }
}
