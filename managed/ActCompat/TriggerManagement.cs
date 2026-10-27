namespace Aftercast.ActCompat
{
    public sealed record TriggerDefinition(string Category, string Regex, int SoundType, string SoundData,
        bool Active, bool RestrictToCategoryZone, bool Timer, string TimerName, bool Tabbed);
    public sealed record TriggerItem(string Key, TriggerDefinition Definition);
    public sealed record TriggerPage(ulong Revision, int Total, bool OriginalLines, bool ProcessImports, TriggerItem[] Items);
}

namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;

    public partial class FormActMain
    {
        public bool ProcessImportCustomTriggers { get; set; }
        private ulong triggerRevision = 1;
        private TriggerItem[] lastTriggerItems = [];
        private bool lastOriginalLines, lastProcessImports;
        private string TriggerPath => Path.Combine(AppDataFolder.FullName, "CustomTriggers.xml");

        private TriggerItem[] TriggerSnapshot()
        {
            var items = CustomTriggers.Select(pair => new TriggerItem(pair.Key, Definition(pair.Value))).ToArray();
            if (!items.SequenceEqual(lastTriggerItems) || lastOriginalLines != OriginalLoglineCustomTriggers || lastProcessImports != ProcessImportCustomTriggers)
            {
                triggerRevision++;
                lastTriggerItems = items;
                lastOriginalLines = OriginalLoglineCustomTriggers;
                lastProcessImports = ProcessImportCustomTriggers;
            }
            return items;
        }
        private static TriggerDefinition Definition(CustomTrigger trigger) => new(trigger.Category, trigger.ShortRegexString,
            trigger.SoundType, trigger.SoundData, trigger.Active, trigger.RestrictToCategoryZone, trigger.Timer, trigger.TimerName, trigger.Tabbed);
        private static CustomTrigger CompileDefinition(TriggerDefinition definition)
        {
            static void Text(string value, int maximum, string name)
            {
                ArgumentNullException.ThrowIfNull(value, name);
                if (value.Length > maximum || value.Contains('\0')) throw new ArgumentException("Invalid trigger " + name);
            }
            Text(definition.Category, 512, "category"); Text(definition.Regex, 4096, "regex");
            Text(definition.SoundData, 4096, "sound data"); Text(definition.TimerName, 512, "timer/tab name");
            if (definition.SoundType is < 0 or > 3) throw new ArgumentException("Invalid trigger sound type");
            return new(definition.Regex, string.IsNullOrEmpty(definition.Category) ? " General" : definition.Category)
            {
                SoundType = definition.SoundType, SoundData = definition.SoundData, Active = definition.Active,
                RestrictToCategoryZone = definition.RestrictToCategoryZone, Timer = definition.Timer,
                TimerName = definition.TimerName, Tabbed = definition.Tabbed
            };
        }
        private void CheckTriggerRevision(ulong expected)
        {
            _ = TriggerSnapshot();
            if (expected != triggerRevision) throw new InvalidOperationException("Trigger definitions changed; refresh before saving");
        }
        public TriggerPage ReadTriggers(int offset, int limit)
        {
            if (offset < 0 || limit is < 1 or > 16) throw new ArgumentOutOfRangeException(nameof(limit));
            lock (triggerGate)
            {
                var items = TriggerSnapshot();
                return new(triggerRevision, items.Length, OriginalLoglineCustomTriggers, ProcessImportCustomTriggers, items.Skip(offset).Take(limit).ToArray());
            }
        }
        public ulong SaveTrigger(ulong expected, string? previousKey, TriggerDefinition definition)
        {
            ArgumentNullException.ThrowIfNull(definition);
            var trigger = CompileDefinition(definition); // Invalid patterns never alter the registry/file.
            lock (triggerGate)
            {
                CheckTriggerRevision(expected);
                if (previousKey is not null && !CustomTriggers.ContainsKey(previousKey)) throw new InvalidOperationException("Original trigger no longer exists");
                if (CustomTriggers.ContainsKey(trigger.Key) && trigger.Key != previousKey) throw new InvalidOperationException("A trigger with this category and regex already exists");
                var next = new SortedList<string, CustomTrigger>(CustomTriggers);
                if (previousKey is not null) next.Remove(previousKey);
                next[trigger.Key] = trigger;
                SaveCustomTriggersFile(TriggerPath, next.Values, OriginalLoglineCustomTriggers, ProcessImportCustomTriggers);
                CustomTriggers.Clear();
                foreach (var item in next) CustomTriggers.Add(item.Key, item.Value);
                if (previousKey is not null) RemoveTriggerResults(previousKey);
                RebuildActiveCustomTriggers();
                _ = TriggerSnapshot();
                return triggerRevision;
            }
        }
        public ulong DeleteTrigger(ulong expected, string key)
        {
            lock (triggerGate)
            {
                CheckTriggerRevision(expected);
                if (!CustomTriggers.ContainsKey(key)) throw new InvalidOperationException("Trigger no longer exists");
                var next = new SortedList<string, CustomTrigger>(CustomTriggers);
                next.Remove(key);
                SaveCustomTriggersFile(TriggerPath, next.Values, OriginalLoglineCustomTriggers, ProcessImportCustomTriggers);
                CustomTriggers.Remove(key);
                RemoveTriggerResults(key);
                RebuildActiveCustomTriggers();
                _ = TriggerSnapshot();
                return triggerRevision;
            }
        }
        public ulong SaveTriggerOptions(ulong expected, bool originalLines, bool processImports)
        {
            lock (triggerGate)
            {
                CheckTriggerRevision(expected);
                SaveCustomTriggersFile(TriggerPath, CustomTriggers.Values, originalLines, processImports);
                OriginalLoglineCustomTriggers = originalLines; ProcessImportCustomTriggers = processImports;
                _ = TriggerSnapshot();
                return triggerRevision;
            }
        }
        public ulong ReloadTriggers(ulong expected)
        {
            lock (triggerGate)
            {
                CheckTriggerRevision(expected);
                // Parse/compile fully before publishing. Explicit reload starts new result tabs.
                if (!File.Exists(TriggerPath)) throw new FileNotFoundException("No saved custom triggers", TriggerPath);
                LoadCustomTriggers(TriggerPath);
                ClearTriggerResults();
                _ = TriggerSnapshot();
                return triggerRevision;
            }
        }
    }
}
