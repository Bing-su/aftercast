using System.Collections.Concurrent;
using System.Runtime.CompilerServices;
using System.Text;
using Advanced_Combat_Tracker;
using Bxivact.ActCompat;

internal static class OriginalLogCaptureChecks
{
    private const string First = "first 한글\0record";
    private static readonly DateTime Epoch = DateTime.UnixEpoch;

    private static void Check(bool condition, [CallerArgumentExpression(nameof(condition))] string? expression = null)
    {
        if (!condition) throw new Exception("Original log capture check failed: " + expression);
    }

    private sealed class Fixture : IDisposable
    {
        internal readonly DirectoryInfo Directory = System.IO.Directory.CreateTempSubdirectory("bxivact-original-capture-");
        internal readonly HostEnvironment Host;
        internal readonly FormActMain Root;
        internal string Path => System.IO.Path.Combine(Directory.FullName, "원본.log");
        internal Fixture()
        {
            Host = new(Directory.FullName);
            Root = ActGlobals.oFormActMain!;
            Root.GetDateTimeFromLog = line => line == First ? Epoch : DateTime.MinValue;
            Root.LastKnownTime = Epoch.AddDays(1);
            Root.GlobalTimeSorter = 40;
        }
        public void Dispose()
        {
            try { Host.Dispose(); }
            finally { Directory.Delete(true); }
        }
    }

    internal static void Run()
    {
        HostLifetimeChecks.RunBounded(ImportCapture);
        HostLifetimeChecks.RunBounded(LiveCapture);
        HostLifetimeChecks.RunBounded(ReentrantSourceChange);
        Console.WriteLine("Immutable original-record capture, observer isolation and source snapshots passed.");
    }

    private static void ImportCapture()
    {
        using var fixture = new Fixture();
        File.WriteAllText(fixture.Path, First + "\n\nno-time\n", new UTF8Encoding(false));
        var records = new List<OriginalLogRecord>();
        fixture.Host.OriginalLogRecordRead += _ => throw new InvalidOperationException("Optional import capture failure");
        fixture.Host.OriginalLogRecordRead += record =>
        {
            records.Add(record);
            if (record.Line == First)
            {
                fixture.Root.LogFilePath = "observer replacement path";
                fixture.Root.GlobalTimeSorter = 900;
            }
        };
        fixture.Host.OriginalLogRecordRead += record =>
        {
            Check(ReferenceEquals(record, records[^1]) && record.Path == fixture.Path);
            Check(record.IsImport && record.Line is First or "no-time");
        };
        var before = 0;
        fixture.Root.BeforeLogLineRead += (import, info) =>
        {
            Check(import && records.Count == before + 1 && records[^1].Line == info.originalLogLine);
            info.logLine = "mutable replacement";
            before++;
        };
        fixture.Root.OnLogLineRead += (_, info) => Check(info.logLine == "mutable replacement");
        var result = fixture.Host.ImportLog(fixture.Path);
        Check(result.LinesRead == 3 && result.RecordsDispatched == 2 && before == 2);
        Check(records.Count == 2 && records[0] == new OriginalLogRecord(true, fixture.Path, First, Epoch, 41));
        Check(records[1] == new OriginalLogRecord(true, fixture.Path, "no-time", DateTime.MinValue, 901));
        Check(fixture.Host.Diagnostics.Item1 == 2 && fixture.Host.RecentDiagnostics.All(message => message.Contains("Original log record observer", StringComparison.Ordinal)));
        Check(records[0].Line.Contains('\0') && (records[0] with { Line = "copy" }).Line != records[0].Line);
    }

    private static void LiveCapture()
    {
        using var fixture = new Fixture();
        File.WriteAllBytes(fixture.Path, Array.Empty<byte>());
        fixture.Root.LogFilePath = fixture.Path;
        var records = new ConcurrentQueue<OriginalLogRecord>();
        using var parsed = new ManualResetEventSlim();
        using var afterUnsubscribe = new ManualResetEventSlim();
        Action<OriginalLogRecord> failing = _ => throw new InvalidOperationException("Optional live capture failure");
        Action<OriginalLogRecord> collect = records.Enqueue;
        fixture.Host.OriginalLogRecordRead += failing;
        fixture.Host.OriginalLogRecordRead += collect;
        var before = 0;
        fixture.Root.BeforeLogLineRead += (import, info) =>
        {
            if (info.originalLogLine == "after unsubscribe") return;
            Check(!import);
            if (info.originalLogLine.Length != 0)
            {
                Check(records.Count == before + 1 && records.Last().Line == info.originalLogLine);
                before++;
            }
            if (info.originalLogLine == First) fixture.Root.GlobalTimeSorter = 1000;
            info.logLine = "mutable live replacement";
        };
        fixture.Root.OnLogLineRead += (_, info) =>
        {
            if (info.originalLogLine == "no-time") { Check(info.logLine == "mutable live replacement"); parsed.Set(); }
            if (info.originalLogLine == "after unsubscribe") afterUnsubscribe.Set();
        };
        fixture.Root.OpenLog(false, false);
        File.AppendAllText(fixture.Path, First + "\n\nno-time\n", new UTF8Encoding(false));
        Check(parsed.Wait(TimeSpan.FromSeconds(5)));
        var snapshot = records.ToArray();
        Check(snapshot.Length == 2 && before == 2);
        Check(snapshot[0] == new OriginalLogRecord(false, fixture.Path, First, Epoch, 41));
        // Live records retain LastKnownTime on a missing timestamp; the intervening empty line has no capture.
        Check(snapshot[1] == new OriginalLogRecord(false, fixture.Path, "no-time", Epoch, 1002));
        fixture.Host.OriginalLogRecordRead -= failing;
        fixture.Host.OriginalLogRecordRead -= collect;
        File.AppendAllText(fixture.Path, "after unsubscribe\n", new UTF8Encoding(false));
        Check(afterUnsubscribe.Wait(TimeSpan.FromSeconds(5)));
        fixture.Host.StopLogReading();
        Check(records.Count == 2 && fixture.Host.Diagnostics.Item1 == 2);
    }

    private static void ReentrantSourceChange()
    {
        using var fixture = new Fixture();
        var nextPath = System.IO.Path.Combine(fixture.Directory.FullName, "next.log");
        File.WriteAllBytes(fixture.Path, Array.Empty<byte>());
        File.WriteAllBytes(nextPath, Array.Empty<byte>());
        fixture.Root.LogFilePath = fixture.Path;
        var records = new ConcurrentQueue<OriginalLogRecord>();
        using var firstCaptured = new ManualResetEventSlim();
        using var nextCaptured = new ManualResetEventSlim();
        fixture.Host.OriginalLogRecordRead += record =>
        {
            if (record.Line != First) return;
            fixture.Root.LogFilePath = nextPath;
            fixture.Root.OpenLog(false, false);
            fixture.Root.GlobalTimeSorter = 999;
        };
        fixture.Host.OriginalLogRecordRead += record =>
        {
            records.Enqueue(record);
            if (record.Line == First) firstCaptured.Set(); else nextCaptured.Set();
        };
        fixture.Root.OpenLog(false, false);
        File.AppendAllText(fixture.Path, First + "\n", new UTF8Encoding(false));
        Check(firstCaptured.Wait(TimeSpan.FromSeconds(5)));
        File.AppendAllText(nextPath, "next\n", new UTF8Encoding(false));
        Check(nextCaptured.Wait(TimeSpan.FromSeconds(5)));
        fixture.Host.StopLogReading();
        var snapshot = records.ToArray();
        Check(snapshot.Length == 2);
        Check(snapshot[0] == new OriginalLogRecord(false, fixture.Path, First, Epoch, 41));
        Check(snapshot[1] == new OriginalLogRecord(false, nextPath, "next", Epoch, 1000));
        Check(fixture.Host.Diagnostics.Item1 == 0);
    }
}
