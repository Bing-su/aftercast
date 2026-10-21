using Aftercast.ActCompat;
using Aftercast.Bridge;

internal static class OriginalCorpusMessageChecks
{
    private sealed class MessageSink
    {
        public Action<DateTime, string> ProcessException { get; set; } = null!;
    }

    internal static void Run()
    {
        foreach (var records in new[] { 2, 3 })
        {
            // Capacity two has no loss at the lower input bound, then exactly one dropped record at three.
            var buffer = new LogBuffer();
            var subscription = buffer.Subscribe(2, 8192);
            for (var iteration = 0; iteration < 3; iteration++)
            {
                for (var index = 0; index < records; index++)
                    buffer.Capture(new OriginalLogRecord(true, "fixture.log", "fixture", DateTime.UnixEpoch, index));
                OriginalCorpusChecks.CheckDroppedPrefix(capacity => buffer.Drain(subscription, capacity), iteration, records);
            }
        }
        var sample = Path.GetTempFileName();
        try
        {
            // Match import framing: BOM, empty records, CRLF and an unterminated final record.
            File.WriteAllText(sample, "\ufefffirst\r\n\r\n한글");
            if (OriginalCorpusChecks.CountRecords(sample) != (3L, 2L))
                throw new Exception("Local log record counts differ from import framing.");
            File.WriteAllText(sample, "251|2026-10-16T12:34:56.7890000+09:00|Recorded|warning|fixture\n" +
                "00|2026-10-16T12:34:56.7890000+09:00|Not a diagnostic|fixture\n");
            // A UTC host's parser converts the source +09:00 time to 03:34; do not reuse its wall clock.
            var recorded = OriginalCorpusChecks.RecordedMessages(sample, _ => new DateTime(2026, 10, 16, 3, 34, 56, 789));
            if (recorded.Count != 1 || recorded["[03:34:56.789] Recorded|warning"] != 3)
                throw new Exception("Historical messages lost their timestamp or import count.");
        }
        finally { File.Delete(sample); }
        var directory = Path.Combine(Path.GetTempPath(), "aftercast corpus messages");
        var plugin = Path.Combine(directory, "FFXIV_ACT_Plugin.dll");
        string Message(string body) => "[12:34:56.789] " + body;
        var baseline = new[]
        {
            Message("FFXIV_ACT_Plugin Version: 3.0.3.3"),
            Message("Selected Language ID: English"),
            Message("Selected Process ID: 0")
        };
        string Renamed(string source, string target) => Message($"Renamed [{source}] to [{target}]].");
        var source = Path.Combine(directory, "deucalion-1.5.0.distrib.dll");
        var target = Path.Combine(directory, "deucalion-1.5.0.dll");
        OriginalCorpusChecks.CheckMessages(baseline, plugin);
        // Windows has no mandatory startup notices, and imported notices need not arrive version-first.
        OriginalCorpusChecks.CheckMessages([], plugin, requireStartupMessages: false);
        OriginalCorpusChecks.CheckMessages(baseline.Reverse().ToArray(), plugin, requireStartupMessages: false);
        var uiMessages = new List<string>();
        var sink = new MessageSink { ProcessException = (_, message) => { if (uiMessages.Count < 100) uiMessages.Add(message); } };
        var original = sink.ProcessException;
        using (var capture = new OriginalParserMessageCapture(sink, []))
        {
            foreach (var message in baseline) sink.ProcessException(DateTime.UnixEpoch, message[15..]);
            for (var index = 0; index < 100; index++) sink.ProcessException(DateTime.UnixEpoch, "Detected Process ID: 0");
            sink.ProcessException(DateTime.UnixEpoch, "System.InvalidOperationException: late parser failure");
            if (uiMessages.Count != 100 || capture.Snapshot().Length != 104)
                throw new Exception("Parser capture lost messages after the UI filled.");
            var rejected = false;
            try { OriginalCorpusChecks.CheckMessages(capture.Snapshot(), plugin); }
            catch (Exception error) when (error.Message.Contains("Unexpected original parser message:", StringComparison.Ordinal))
            {
                rejected = error.Message.Contains("late parser failure", StringComparison.Ordinal);
            }
            if (!rejected) throw new Exception("A parser exception after the UI limit was accepted.");
        }
        if (sink.ProcessException != original) throw new Exception("Parser capture did not restore the original callback.");
        var historical = Message("Recorded warning");
        OriginalCorpusChecks.CheckMessages([.. baseline, historical], plugin,
            recordedMessages: new() { [historical] = 1 });
        foreach (var unrecorded in new[] { Message("Different warning"), "[12:34:56.790] Recorded warning", historical })
        {
            try
            {
                // Exhausted counts and changed timestamps/text must not hide a newly generated parser failure.
                OriginalCorpusChecks.CheckMessages([.. baseline, unrecorded], plugin,
                    recordedMessages: new() { [historical] = unrecorded == historical ? 0 : 1 });
            }
            catch (Exception error) when (error.Message.Contains("Unexpected original parser message:", StringComparison.Ordinal)) { continue; }
            throw new Exception("Historical diagnostic matching accepted an unrecorded message.");
        }
        // A pristine trusted deployment renames its bundled DLL once; subsequent starts omit this notice.
        OriginalCorpusChecks.CheckMessages([.. baseline, Renamed(source, target)], plugin);
        OriginalCorpusChecks.CheckMessages([.. baseline,
            Renamed(Path.Combine(directory, "deucalion-1.5.1.distrib.dll"), Path.Combine(directory, "deucalion-1.5.1.dll"))],
            plugin, "1.5.1");
        OriginalCorpusChecks.CheckMessages([.. baseline,
            Message("SimulatedDamage Timeout with no ticks/GroundAoE: Removing status fixture")], plugin);
        foreach (var unexpected in new[]
        {
            Renamed(Path.Combine(directory, "other.distrib.dll"), target),
            Renamed(source, Path.Combine(directory, "other.dll")),
            Renamed(Path.Combine(directory, "other", "deucalion-1.5.0.distrib.dll"), target),
            Message("System.InvalidOperationException: parser failed")
        })
        {
            try { OriginalCorpusChecks.CheckMessages([.. baseline, unexpected], plugin); }
            catch (Exception error) when (error.Message.Contains("Unexpected original parser message: " + unexpected,
                StringComparison.Ordinal)) { continue; }
            throw new Exception("Corpus diagnostics accepted an unknown message or lost its original text.");
        }
        // Synthetic and local input must never inherit the pinned corpus's simulated-damage warning allowance.
        try
        {
            OriginalCorpusChecks.CheckMessages([.. baseline,
                Message("SimulatedDamage Timeout with no ticks/GroundAoE: Removing status fixture")],
                plugin, allowSimulatedDamageWarnings: false);
            throw new Exception("Synthetic diagnostics accepted a simulated-damage warning.");
        }
        catch (Exception error) when (error.Message.Contains("Unexpected original parser message:", StringComparison.Ordinal)) { }
        Console.WriteLine("Corpus diagnostics accept the exact deployment rename and retain unknown-message failures.");
    }
}
