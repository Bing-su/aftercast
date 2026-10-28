using Aftercast.ActCompat;

namespace Aftercast.ActCompat
{
    public sealed record TimerBatchPreview(ulong Revision, TimerItem[] Items, string[] Conflicts);
}
namespace Advanced_Combat_Tracker
{
    public partial class FormSpellTimers
    {
        private TimerData[] CompileTimerBatch(TimerDefinition[] definitions)
        {
            ArgumentNullException.ThrowIfNull(definitions);
            if (definitions.Length is < 1 or > 32) throw new InvalidDataException("Select 1–32 timer definitions");
            var seen = new SortedSet<string>(TimerDefs.Comparer);
            var result = new List<TimerData>();
            foreach (var definition in definitions)
            {
                var item = CompileTimer(definition);
                if (!seen.Add(item.Key)) throw new InvalidDataException("Duplicate timer key in Config");
                result.Add(item);
            }
            return result.ToArray();
        }
        public TimerBatchPreview PreviewTimerConfig(string text)
        {
            var rows = ConfigBatchXml.Read(text, "SpellTimers", "Spell", "Name", "Category", "OnlyMasterTicks", "RestrictCategory", "RemoveValue", "Panel1", "Panel2", "FillColor", "Tooltip", "Modable", "Checked", "RadialDisplay", "Timer", "WarningValue", "Absolute", "Restrict", "StartWav", "WarningWav");
            var definitions = rows.Select(row => new TimerDefinition {
                Name = ConfigBatchXml.Required(row,"Name"), Category = ConfigBatchXml.Required(row,"Category"),
                OnlyMasterTicks = ConfigBatchXml.Bool(row,"OnlyMasterTicks",false), RestrictToCategory = ConfigBatchXml.Bool(row,"RestrictCategory",false),
                RemoveValue = ConfigBatchXml.Int(row,"RemoveValue",-15), Panel1Display = ConfigBatchXml.Bool(row,"Panel1",true), Panel2Display = ConfigBatchXml.Bool(row,"Panel2",false),
                FillColorArgb = ConfigBatchXml.Int(row,"FillColor",-16776961), Tooltip = ConfigBatchXml.Text(row,"Tooltip"), Modable = ConfigBatchXml.Bool(row,"Modable",true),
                ActiveInList = ConfigBatchXml.Bool(row,"Checked",true), RadialDisplay = ConfigBatchXml.Bool(row,"RadialDisplay",true), TimerValue = ConfigBatchXml.Int(row,"Timer",30),
                WarningValue = ConfigBatchXml.Int(row,"WarningValue",10), AbsoluteTiming = ConfigBatchXml.Bool(row,"Absolute",false), RestrictToMe = ConfigBatchXml.Bool(row,"Restrict",false),
                StartSoundData = ConfigBatchXml.Text(row,"StartWav"), WarningSoundData = ConfigBatchXml.Text(row,"WarningWav") }).ToArray();
            var compiled = CompileTimerBatch(definitions);
            lock (gate)
            {
                ObjectDisposedException.ThrowIf(IsDisposed,this); _ = TimerSnapshot();
                return new(timerRevision, compiled.Select(item => new TimerItem(item.Key,TimerDefinition.FromData(item))).ToArray(), compiled.Where(item => TimerDefs.ContainsKey(item.Key)).Select(item => item.Key).ToArray());
            }
        }
        public ulong SaveTimerBatch(string path, ulong revision, TimerDefinition[] definitions)
        {
            var compiled = CompileTimerBatch(definitions);
            lock (gate)
            {
                ObjectDisposedException.ThrowIf(IsDisposed,this); CheckTimerRevision(revision);
                var next = new SortedDictionary<string,TimerData>(TimerDefs,TimerDefs.Comparer);
                foreach (var item in compiled)
                {
                    if (next.ContainsKey(item.Key)) throw new InvalidOperationException("A timer in Config already exists");
                    next.Add(item.Key,item);
                }
                WriteTimerFile(path,next.Values);
                foreach (var item in compiled) TimerDefs.Add(item.Key,item);
                RebuildSpellTreeView(); _ = TimerSnapshot(); return timerRevision;
            }
        }
    }
}
