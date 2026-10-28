using System.Reflection;
using Aftercast.ActCompat;
using Advanced_Combat_Tracker;

internal static class TriggerManagementChecks
{
    private static void Check(bool value, string message) { if (!value) throw new InvalidOperationException(message); }
    private static void Reject(Action action)
    {
        try { action(); }
        catch (Exception error) when (error is ArgumentException or InvalidOperationException or IOException or System.Xml.XmlException) { return; }
        throw new Exception("Expected rejected trigger operation");
    }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-trigger-management-");
        var path = Path.Combine(directory.FullName, "CustomTriggers.xml");
        try
        {
            using (var host = new HostEnvironment(directory.FullName))
            {
                var root = ActGlobals.oFormActMain!;
                root.TimeStampLen = 2;
                host.ConfigureSpeech(true);
                ActGlobals.charName = "Source";
                root.CurrentZone = "Fixture Zone";
                var definition = new TriggerDefinition("Fixture", @"^(?<YOU>Source) (?<value>\d+)$", 3,
                    "${value}", true, true, true, "Results", true);
                ActGlobals.oFormSpellTimers!.AddEditTimerDef(new TimerData("Results", "Fixture") { TimerValue = 10 });
                ActGlobals.oFormSpellTimers.RebuildSpellTreeView();
                var revision = root.ReadTriggers(0,16).Revision;
                Reject(() => root.ReloadTriggers(revision));
                revision = root.SaveTrigger(revision, null, definition);
                var originalKey = root.ReadTriggers(0,16).Items.Single().Key;
                var bytes = File.ReadAllBytes(path);
                Reject(() => root.SaveTrigger(revision, null, definition));
                Reject(() => root.SaveTrigger(revision, originalKey, definition with { Regex = "[" }));
                Reject(() => root.SaveTrigger(revision - 1, originalKey, definition));
                Check(File.ReadAllBytes(path).SequenceEqual(bytes), "Rejected edits changed persisted definitions");
                // Archive matching is independently enabled and runs only after a complete decode.
                var archive = Path.Combine(directory.FullName, "synthetic.xml");
                File.WriteAllText(archive, "<Encounters><Encounter Name=\"fixture\"><LogLine Type=\"1\" Time=\"2020-01-01T00:00:00\" Line=\"00Source 41\" /></Encounter></Encounters>");
                host.ArchiveOperation(1, archive, 0, false);
                Check(root.ReadTriggerResults(0,16).Items.Length == 0, "Default archive import ran triggers");
                File.WriteAllText(archive, "<Encounters><Encounter Name=\"broken\"><LogLine Type=\"1\" Time=\"2020-01-01T00:00:00\" Line=\"00Source 41\" /><Unexpected /></Encounter></Encounters>");
                Reject(() => host.ArchiveOperation(1, archive, 0, false, processTriggers: true));
                Check(root.ReadTriggerResults(0,16).Items.Length == 0, "Malformed archive emitted a partial result");
                File.WriteAllText(archive, "<Encounters><Encounter Name=\"fixture\"><LogLine Type=\"1\" Time=\"2020-01-01T00:00:00\" Line=\"00Source 41\" /></Encounter></Encounters>");
                host.ArchiveOperation(1, archive, 0, false, processTriggers: true);
                var archived = root.ReadTriggerResults(0,16).Items.Single();
                Check(archived.IsImport && archived.Line == "Source 41" && host.DrainSpeech().Length == 0 &&
                    ActGlobals.oFormSpellTimers.GetTimerFrames().Count == 0, "Archive matching lost result or ran import effects");
                root.ClearTriggerResults();
                revision = root.SaveTriggerOptions(revision, false, true);
                Process(root, "original", "00Source 42", true);
                var result = root.ReadTriggerResults(0,16).Items.Single();
                Check(result.Line == "Source 42" && result.IsImport && result.Captures.Single(c => c.Name == "value").Value == "42", "Import result lost captures or timestamp stripping");
                Check(host.DrainSpeech().Length == 0 && ActGlobals.oFormSpellTimers.GetTimerFrames().Count == 0,
                    "Import played audio or started a timer");
                Process(root, "original", "00Source 43", false);
                Check(host.DrainSpeech().Length != 0 && ActGlobals.oFormSpellTimers.GetTimerFrames().Count == 1,
                    "Live trigger effects stopped working");
                Process(root, "original", "00Source 44", false);
                Check(root.ReadTriggerResults(0,16).Items.Length == 3 && host.DrainSpeech().Length == 0,
                    "Audio cooldown incorrectly suppressed result records");
                var beforeLive = root.ReadTriggerResults(0,16).LatestSequence;
                var livePath = Path.Combine(directory.FullName, "live.log");
                File.WriteAllText(livePath, "");
                int capturedSorter = 0;
                host.OriginalLogRecordRead += record => capturedSorter = record.TimeSorter;
                root.GetDateTimeFromLog = _ => DateTime.UnixEpoch;
                root.BeforeLogLineRead += (_, info) => info.logLine = "00Source 77";
                root.OnLogLineRead += (_, info) => { info.logLine = "00Wrong 0"; root.GlobalTimeSorter = 999; };
                root.LogFilePath = livePath; root.OpenLog(false,false);
                File.AppendAllText(livePath,"raw input\n");
                var deadline = DateTime.UtcNow.AddSeconds(3);
                TriggerResult[] received;
                while ((received = root.ReadTriggerResults(beforeLive,16).Items).Length == 0 && DateTime.UtcNow < deadline) Thread.Sleep(10);
                Check(received.Length == 1 && received[0].Line == "Source 77" && received[0].TimeSorter == capturedSorter && capturedSorter != 999,
                    "Trigger text or source identity was mutated by OnLogLineRead");
                revision = root.SaveTrigger(revision, originalKey, definition with { Category = "Renamed", RestrictToCategoryZone = false });
                Check(root.ReadTriggerResults(0,16).Items.Length == 0, "Rename retained an old result tab");
                Check(root.ReadTriggers(0,16).Total == 1, "Rename left duplicate definitions");
                var key = root.ReadTriggers(0,16).Items.Single().Key;
                // Failed disk replacement leaves the in-memory compiled trigger and saved file unchanged.
                var beforeFailure = File.ReadAllBytes(path);
                File.Move(path, path + ".backup"); Directory.CreateDirectory(path);
                Reject(() => root.SaveTrigger(revision, key, definition with { Category = "Failure" }));
                Check(root.ReadTriggers(0,16).Items.Single().Key == key, "Write failure published the edit");
                Directory.Delete(path); File.Move(path + ".backup", path);
                Check(File.ReadAllBytes(path).SequenceEqual(beforeFailure), "Failed edit changed the saved file");
                var any = definition with { Category = "Bounds", Regex = "(?<value>.*)", SoundType = 0, Timer = false, RestrictToCategoryZone = false };
                revision = root.SaveTrigger(revision, null, any);
                for (var index=0; index<4100; index++) Process(root, "", "00Source 1", true);
                var bounded = root.ReadTriggerResults(0,16);
                Check(bounded.Omitted > 0 && bounded.Items.Length == 16 && bounded.Items[0].Sequence > 1, "Result retention did not report eviction");
                var after = bounded.LatestSequence;
                Process(root, "", "00" + new string('x', 40000), true);
                Check(root.ReadTriggerResults(after,16).Items.Length == 0 && root.ReadTriggerResults(after,16).Omitted > bounded.Omitted,
                    "Oversized capture record was not omitted atomically");
                root.ClearTriggerResults();
                Check(root.ReadTriggerResults(0,16).Items.Length == 0, "Results were not cleared");
                revision = root.DeleteTrigger(revision, key);
                Check(root.ReadTriggers(0,16).Total == 1, "Delete did not update registry");
                _ = root.ReloadTriggers(revision);
            }
            using (var restarted = new HostEnvironment(directory.FullName))
            {
                var root = ActGlobals.oFormActMain!;
                var page = root.ReadTriggers(0,16);
                Check(page.Total == 1 && page.ProcessImports && !page.OriginalLines && root.ReadTriggerResults(0,16).Items.Length == 0,
                    "Restart lost definitions/options or restored transient results");
            }
            Console.WriteLine("Trigger atomic CRUD, failures, import/live effects, result bounds and restart checks passed.");
        }
        finally { directory.Delete(true); }
    }
    private static void Process(FormActMain root, string original, string processed, bool import) =>
        typeof(FormActMain).GetMethod("ProcessCustomTriggers", BindingFlags.Instance | BindingFlags.NonPublic)!
            .Invoke(root, new object[] { original, new LogLineEventArgs(processed,0,DateTime.UnixEpoch,"Fixture",false), import });
}
