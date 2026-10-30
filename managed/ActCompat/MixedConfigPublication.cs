using Aftercast.ActCompat;

namespace Advanced_Combat_Tracker
{
    public partial class FormActMain
    {
        internal void WithMixedTriggers(ulong revision, TriggerDefinition[] definitions, string staging,
            Action<byte[], Action, Action> commit)
        {
            var compiled = CompileTriggerBatch(definitions);
            lock (triggerGate)
            {
                CheckTriggerRevision(revision);
                var next = new SortedList<string, CustomTrigger>(CustomTriggers);
                foreach (var item in compiled)
                {
                    if (next.ContainsKey(item.Key)) throw new InvalidOperationException("A trigger in Config already exists");
                    next.Add(item.Key, item);
                }
                var path = Path.Combine(staging, "CustomTriggers.xml");
                SaveCustomTriggersFile(path, next.Values, OriginalLoglineCustomTriggers, ProcessImportCustomTriggers);
                commit(File.ReadAllBytes(path), () =>
                {
                    CustomTriggers.Clear();
                    foreach (var item in next) CustomTriggers.Add(item.Key, item.Value);
                }, () => { RebuildActiveCustomTriggers(); _ = TriggerSnapshot(); });
            }
        }
    }
    public partial class FormSpellTimers
    {
        internal void WithMixedTimers(ulong revision, TimerDefinition[] definitions, string staging,
            Action<byte[], Action, Action> commit)
        {
            var compiled = CompileTimerBatch(definitions);
            lock (gate)
            {
                ObjectDisposedException.ThrowIf(IsDisposed, this);
                CheckTimerRevision(revision);
                var next = new SortedDictionary<string, TimerData>(TimerDefs, TimerDefs.Comparer);
                foreach (var item in compiled)
                {
                    if (next.ContainsKey(item.Key)) throw new InvalidOperationException("A timer in Config already exists");
                    next.Add(item.Key, item);
                }
                var path = Path.Combine(staging, "SpellTimers.xml");
                WriteTimerFile(path, next.Values);
                commit(File.ReadAllBytes(path), () =>
                {
                    // Preserve existing registry objects and their active countdown frames.
                    foreach (var item in compiled) TimerDefs.Add(item.Key, item);
                }, () => { RebuildSpellTreeView(); _ = TimerSnapshot(); });
            }
        }
    }
}

namespace Aftercast.ActCompat
{
    public partial class HostEnvironment
    {
        public bool IsFaulted { get; private set; }
        public void ImportMixedConfig(string text, ulong triggerRevision, ulong timerRevision) =>
            ImportMixedConfigCore(text, triggerRevision, timerRevision, null);

        private void ImportMixedConfigCore(string text, ulong triggerRevision, ulong timerRevision, Action<string>? fault)
        {
            ObjectDisposedException.ThrowIf(disposed, this);
            var preview = PreviewMixedConfig(text);
            var directory = main.AppDataFolder.FullName;
            var staging = Path.Combine(directory, ".mixed-stage-" + Guid.NewGuid().ToString("N"));
            var committed = false;
            try
            {
                Directory.CreateDirectory(staging);
                RunWithLogReadingSuspended(() =>
                {
                    var beforeTriggers = MixedConfigFiles.ReadBeforeImage(directory, "CustomTriggers.xml");
                    var beforeTimers = MixedConfigFiles.ReadBeforeImage(directory, "SpellTimers.xml");
                    main.WithMixedTriggers(triggerRevision, preview.Triggers.Items.Select(item => item.Definition).ToArray(), staging,
                    (triggerBytes, publishTriggers, refreshTriggers) =>
                    timers.WithMixedTimers(timerRevision, preview.Timers.Items.Select(item => item.Definition).ToArray(), staging,
                    (timerBytes, publishTimers, refreshTimers) =>
                    {
                        fault?.Invoke("before-commit");
                        MixedConfigFiles.CommitCore(directory, beforeTriggers, beforeTimers, triggerBytes, timerBytes, step => fault?.Invoke("files:" + step));
                        committed = true;
                        fault?.Invoke("committed");
                        publishTriggers();
                        fault?.Invoke("triggers-published");
                        publishTimers();
                        fault?.Invoke("timers-published");
                        refreshTriggers();
                        refreshTimers();
                    }));
                });
                Directory.Delete(staging, true);
            }
            catch (Exception original)
            {
                var errors = new List<Exception> { original };
                if (committed || original is MixedConfigConflictException || File.Exists(Path.Combine(directory, ".mixed-config.json")))
                {
                    // Suspension has unwound. Retire this host before any further plugin work;
                    // normal Dispose saves would otherwise overwrite the committed pair.
                    IsFaulted = true;
                    try { Dispose(); }
                    catch (Exception cleanup) { errors.Add(cleanup); }
                }
                try { if (Directory.Exists(staging)) Directory.Delete(staging, true); }
                catch (Exception cleanup) { errors.Add(cleanup); }
                if (errors.Count > 1) throw new AggregateException("Mixed Config publication and cleanup failed", errors);
                throw;
            }
        }
    }
}
