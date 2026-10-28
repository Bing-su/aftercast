using Aftercast.ActCompat;

namespace Aftercast.ActCompat
{
    public sealed record TriggerBatchPreview(ulong Revision, TriggerItem[] Items, string[] Conflicts);
}
namespace Advanced_Combat_Tracker
{
    public partial class FormActMain
    {
        private static CustomTrigger[] CompileTriggerBatch(TriggerDefinition[] definitions)
        {
            ArgumentNullException.ThrowIfNull(definitions);
            if (definitions.Length is < 1 or > 32) throw new InvalidDataException("Select 1–32 trigger definitions");
            var seen = new SortedSet<string>();
            var result = new List<CustomTrigger>();
            foreach (var definition in definitions)
            {
                CustomTrigger item;
                try { item = CompileDefinition(definition); }
                catch (System.Text.RegularExpressions.RegexParseException) { throw new InvalidDataException("Config contains an invalid regular expression"); }
                if (!seen.Add(item.Key)) throw new InvalidDataException("Duplicate trigger key in Config");
                result.Add(item);
            }
            return result.ToArray();
        }
        public TriggerBatchPreview PreviewTriggerConfig(string text)
        {
            var rows = ConfigBatchXml.Read(text, "CustomTriggers", "Trigger", "Active", "Regex", "SoundData", "SoundType", "CategoryRestrict", "Category", "Timer", "TimerName", "Tabbed");
            var definitions = rows.Select(row => new TriggerDefinition(ConfigBatchXml.Required(row,"Category"), ConfigBatchXml.Required(row,"Regex"),
                ConfigBatchXml.Int(row,"SoundType",0), ConfigBatchXml.Text(row,"SoundData"), ConfigBatchXml.Bool(row,"Active",true),
                ConfigBatchXml.Bool(row,"CategoryRestrict",false), ConfigBatchXml.Bool(row,"Timer",false), ConfigBatchXml.Text(row,"TimerName"), ConfigBatchXml.Bool(row,"Tabbed",false))).ToArray();
            var compiled = CompileTriggerBatch(definitions);
            lock (triggerGate)
            {
                _ = TriggerSnapshot();
                return new(triggerRevision, compiled.Select(item => new TriggerItem(item.Key,Definition(item))).ToArray(), compiled.Where(item => CustomTriggers.ContainsKey(item.Key)).Select(item => item.Key).ToArray());
            }
        }
        public ulong SaveTriggerBatch(ulong revision, TriggerDefinition[] definitions)
        {
            var compiled = CompileTriggerBatch(definitions);
            lock (triggerGate)
            {
                CheckTriggerRevision(revision);
                var next = new SortedList<string,CustomTrigger>(CustomTriggers);
                foreach (var item in compiled)
                {
                    if (next.ContainsKey(item.Key)) throw new InvalidOperationException("A trigger in Config already exists");
                    next.Add(item.Key,item);
                }
                SaveCustomTriggersFile(TriggerPath,next.Values,OriginalLoglineCustomTriggers,ProcessImportCustomTriggers);
                CustomTriggers.Clear(); foreach (var item in next) CustomTriggers.Add(item.Key,item.Value);
                RebuildActiveCustomTriggers(); _ = TriggerSnapshot(); return triggerRevision;
            }
        }
    }
}
