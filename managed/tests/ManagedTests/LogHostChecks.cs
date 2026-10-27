using System.Collections.Concurrent;
using System.Text;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class LogHostChecks
{
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Log host contract check failed");
    }

    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-log-host-");
        try
        {
            using var environment = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            var path = Path.Combine(directory.FullName, "Network.log");
            File.WriteAllText(path, "existing\n", new UTF8Encoding(false));
            root.LogFilePath = path;
            root.LogFileFilter = "Network_*.log";
            root.GetDateTimeFromLog = line => line == "no-time" ? DateTime.MinValue : DateTime.UnixEpoch;
            var lines = new ConcurrentQueue<LogLineEventArgs>();
            var changes = 0;
            root.LogFileChanged += (import, file) => { Check(!import && file == path); changes++; };
            LogLineEventDelegate failure = (_, _) => throw new InvalidOperationException("subscriber failure");
            root.BeforeLogLineRead += failure;
            root.BeforeLogLineRead += (_, info) => info.logLine += " amended";
            root.OnLogLineRead += (_, info) => { Check(root.LastLogLine == info.logLine); lines.Enqueue(info); };
            root.OpenLog(false, false);
            Check(changes == 1 && root.LogFileFilter == "Network_*.log");
            File.AppendAllText(path, "first\nno-time\n", new UTF8Encoding(false));
            Check(SpinWait.SpinUntil(() => lines.Count == 2, TimeSpan.FromSeconds(5)));
            var received = lines.ToArray();
            Check(received[0].originalLogLine == "first" && received[0].logLine == "first amended");
            Check(received[1].originalLogLine == "no-time" && received[1].detectedTime == DateTime.UnixEpoch);
            Check(root.LastKnownTime == DateTime.UnixEpoch && environment.Diagnostics.Item1 == 2);
            root.BeforeLogLineRead -= failure;
            // Stop before deleting the directory; callbacks must never outlive the host's globals.
            environment.Dispose();
            Check(ActGlobals.oFormActMain is null);
            File.AppendAllText(path, "after stop\n");
            Check(lines.Count == 2);
        }
        finally { directory.Delete(true); }
        ReopenFromCallback();
        HistoricalZone();
        EncodingSnapshot();
    }

    private static void ReopenFromCallback()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-log-reopen-");
        try
        {
            using var environment = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            var first = Path.Combine(directory.FullName, "first.log");
            var second = Path.Combine(directory.FullName, "second.log");
            File.WriteAllText(first, "");
            File.WriteAllText(second, "");
            root.GetDateTimeFromLog = _ => DateTime.UnixEpoch;
            root.LogFilePath = first;
            var received = new ConcurrentQueue<string>();
            root.BeforeLogLineRead += (_, info) =>
            {
                if (info.logLine != "switch") return;
                // A subscriber can switch files without joining its own reader or delivering stale hooks.
                root.LogFilePath = second;
                root.OpenLog(false, false);
                File.AppendAllText(second, "new file\n");
            };
            root.OnLogLineRead += (_, info) => received.Enqueue(info.originalLogLine);
            root.OpenLog(false, false);
            File.AppendAllText(first, "switch\nstale\n");
            Check(SpinWait.SpinUntil(() => received.Count == 1, TimeSpan.FromSeconds(5)));
            environment.Dispose();
            Check(received.ToArray().SequenceEqual(new[] { "new file" }) && environment.Diagnostics.Item1 == 0);
        }
        finally { directory.Delete(true); }
    }

    private static void HistoricalZone()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-log-zone-");
        try
        {
            using var environment = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            var path = Path.Combine(directory.FullName, "zone.log");
            using (var output = File.Create(path))
            {
                output.Write(Encoding.UTF8.GetBytes("zone:old\nzone:한글\n"));
                var filler = Encoding.UTF8.GetBytes(new string('x', 4095) + "\n");
                for (var index = 0; index < 300; index++) output.Write(filler);
                // The trailing record is incomplete and must not influence the initial zone snapshot.
                output.Write(Encoding.UTF8.GetBytes("zone:incomplete"));
                output.WriteByte(0xea);
            }
            // Initial zone remains the default UTF-8 path even when forward capture selects Korean legacy text.
            root.LogEncoding = Encoding.GetEncoding(949);
            root.ZoneChangeRegex = new("^zone:(?<zone>.+)$");
            root.LogFilePath = path;
            root.OpenLog(true, false);
            Check(root.CurrentZone == "한글" && environment.Diagnostics.Item1 == 0);
            environment.Dispose();
        }
        finally { directory.Delete(true); }
    }
    private static void EncodingSnapshot()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-live-codec-");
        try
        {
            using var environment = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            var path = Path.Combine(directory.FullName, "selected.log");
            File.WriteAllBytes(path, []);
            root.LogFilePath = path;
            root.LogEncoding = Encoding.GetEncoding(949);
            root.GetDateTimeFromLog = _ => DateTime.UnixEpoch;
            var lines = new ConcurrentQueue<string>();
            root.OnLogLineRead += (_, line) => lines.Enqueue(line.originalLogLine);
            root.OpenLog(false, false);
            root.LogEncoding = Encoding.GetEncoding(28591);
            try { root.OpenLog(false, false); throw new Exception("Unsupported encoding accepted"); }
            catch (ArgumentException) { }
            // Rejected admission must leave the old reader alive with its original encoding snapshot.
            using (var output = new FileStream(path, FileMode.Append, FileAccess.Write, FileShare.ReadWrite | FileShare.Delete))
                output.Write(Encoding.GetEncoding(949).GetBytes("한글\n"));
            Check(SpinWait.SpinUntil(() => lines.Count == 1, TimeSpan.FromSeconds(5)));
            Check(lines.TryPeek(out var original) && original == "한글" && environment.Diagnostics.Item1 == 0);
        }
        finally { directory.Delete(true); }
    }

}
