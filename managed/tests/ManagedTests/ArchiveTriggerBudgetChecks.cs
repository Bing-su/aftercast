using System.Diagnostics;
using System.Reflection;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class ArchiveTriggerBudgetChecks
{
    private static void Check(bool condition, string message) { if (!condition) throw new InvalidOperationException(message); }
    private static void Reject(Action action, string text)
    {
        try { action(); }
        catch (Exception error) when (error.ToString().Contains(text, StringComparison.OrdinalIgnoreCase)) { return; }
        throw new Exception("Expected error mentioning " + text);
    }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-archive-budget-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            root.TimeStampLen = 0;
            root.AddEditCustomTrigger(new CustomTrigger("^ok$", "fixture") { Tabbed = true, Timer = true, SoundType = 3, SoundData = "no sound" });
            var path = Path.Combine(directory.FullName, "synthetic.xml");
            string Xml(string lines) => "<Encounters><Encounter Name=\"fixture\">" + lines + "</Encounter></Encounters>";
            string Line(string value) => "<LogLine Type=\"1\" Time=\"2020-01-01T00:00:00\" Line=\"" + value + "\" />";
            void Save(string lines) => File.WriteAllText(path, Xml(lines));
            int Count() => host.ListEncounters(0, 100).Total;
            void Unchanged(int count, ulong sequence)
            {
                Check(Count() == count && root.ReadTriggerResults(0,16).LatestSequence == sequence, "Budget failure published partial encounters or results");
                Check(host.DrainSpeech().Length == 0 && ActGlobals.oFormSpellTimers!.GetTimerFrames().Count == 0, "Archive matching caused live effects");
            }
            Save(Line("ok"));
            host.ArchiveOperation(1, path, 0, false, processTriggers: true);
            Check(root.ReadTriggerResults(0,16).Items.Length == 1, "Seed result missing");
            int before = Count(); ulong sequence = root.ReadTriggerResults(0,16).LatestSequence;
            var sorter = root.GlobalTimeSorter;
            var clock = typeof(FormActMain).GetField("archiveTriggerClock", BindingFlags.Instance | BindingFlags.NonPublic)!;
            var calls = 0;
            clock.SetValue(root, (Func<long>)(() => ++calls <= 5 ? 0 : Stopwatch.Frequency * 3));
            Save(Line("ok") + Line("ok"));
            Reject(() => host.ArchiveOperation(1, path, 0, false, processTriggers: true), "two seconds");
            Unchanged(before, sequence);
            Check(root.GlobalTimeSorter == sorter, "Budget failure advanced sorter");
            clock.SetValue(root, (Func<long>)Stopwatch.GetTimestamp);
            Save(string.Concat(Enumerable.Repeat(Line("ok"), 10001)));
            Reject(() => host.ArchiveOperation(1, path, 0, false, processTriggers: true), "10000");
            Unchanged(before, sequence);
            Save(Line(new string('x', 16 * 1024 * 1024 + 1)));
            Reject(() => host.ArchiveOperation(1, path, 0, false, processTriggers: true), "16 MiB");
            Unchanged(before, sequence);
            for (var i = 0; i < 32; i++) root.AddEditCustomTrigger(new CustomTrigger("^other" + i + "$", "fixture"));
            Save(Line("ok"));
            Reject(() => host.ArchiveOperation(1, path, 0, false, processTriggers: true), "32 active");
            Unchanged(before, sequence);
            root.CustomTriggers.Clear();
            for (var i = 0; i < 1025; i++) { var trigger = new CustomTrigger("^inactive" + i + "$", "fixture") { Active = false }; root.CustomTriggers.Add(trigger.Key, trigger); }
            Reject(() => host.ArchiveOperation(1, path, 0, false, processTriggers: true), "1024 registered");
            Unchanged(before, sequence);
            root.CustomTriggers.Clear();
            root.AddEditCustomTrigger(new CustomTrigger("(a+)+$", "fixture") { Tabbed = true });
            Save(Line(new string('a', 30000) + "!"));
            Reject(() => host.ArchiveOperation(1, path, 0, false, processTriggers: true), "100 milliseconds");
            Unchanged(before, sequence);
            root.CustomTriggers.Clear();
            root.AddEditCustomTrigger(new CustomTrigger("^ok$", "fixture") { Tabbed = true });
            Save(Line("ok"));
            host.ArchiveOperation(1, path, 0, false, processTriggers: true);
            Check(Count() == before + 1 && root.ReadTriggerResults(sequence,16).Items.Length == 1, "Retry after rejection failed");
            var initial = root.ReadTriggerResults(0,16);
            Save(string.Concat(Enumerable.Repeat(Line("ok"), 4100)));
            host.ArchiveOperation(1, path, 0, false, processTriggers: true);
            var retained = root.ReadTriggerResults(0,16);
            Check(retained.LatestSequence == initial.LatestSequence + 4100 && retained.Omitted >= initial.Omitted + 4,
                "Staged retention lost sequence or omission counts");
            Console.WriteLine("Archive trigger atomic budgets, partial staging, regex timeout, retries and side-effect suppression passed.");
        }
        finally { directory.Delete(true); }
    }
}
