using System.Collections.Concurrent;
using System.Globalization;
using System.Text;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class LiveCompanionChecks
{
    private static readonly DateTime Epoch = DateTime.UnixEpoch;
    private static void Check(bool value, string message)
    {
        if (!value) throw new Exception("Live companion: " + message);
    }
    private static void Wait(Func<bool> ready, string message) => Check(SpinWait.SpinUntil(ready, TimeSpan.FromSeconds(5)), message);
    private static void Append(string path, byte[] bytes)
    {
        using var stream = new FileStream(path, FileMode.Append, FileAccess.Write, FileShare.ReadWrite | FileShare.Delete);
        stream.Write(bytes); stream.Flush(true);
    }
    private static void Append(string path, string line) => Append(path, Encoding.UTF8.GetBytes(line));
    private sealed class Fixture : IDisposable
    {
        internal readonly DirectoryInfo Directory = System.IO.Directory.CreateTempSubdirectory("aftercast-live-companions-");
        internal readonly HostEnvironment Host;
        internal readonly FormActMain Root;
        internal readonly ConcurrentQueue<(string Name, string Line, DateTime Time, DateTime Clock, int Thread)> Seen = new();
        internal readonly ConcurrentQueue<OriginalLogRecord> Captured = new();
        internal readonly ConcurrentQueue<int> ParserThreads = new();
        internal string Main => Path.Combine(Directory.FullName, "main.log");
        internal string Side(string name) => Root.GetCompanionFilePath(Main, name);
        internal Fixture(params string[] names)
        {
            Host = new(Directory.FullName); Root = ActGlobals.oFormActMain!;
            File.WriteAllText(Main, "99|historical main\n", new UTF8Encoding(false));
            System.IO.Directory.CreateDirectory(Path.Combine(Directory.FullName, "CompanionLogs"));
            foreach (var name in names) File.WriteAllBytes(Side(name), []);
            Root.LogFilePath = Main; Root.LastKnownTime = Epoch;
            Root.GetDateTimeFromLog = line =>
            {
                ParserThreads.Enqueue(Environment.CurrentManagedThreadId);
                return line.StartsWith("undated", StringComparison.Ordinal) ? DateTime.MinValue :
                    Epoch.AddSeconds(int.Parse(line.Split('|')[0], CultureInfo.InvariantCulture));
            };
            Root.BeforeLogLineRead += (import, info) => { Check(!import, "unexpected import callback"); info.logLine += " edited"; };
            Root.OnLogLineRead += (_, info) =>
            {
                Check(info.logLine.EndsWith(" edited", StringComparison.Ordinal), "mutable hook order");
                Seen.Enqueue((info.companionLogName, info.originalLogLine, info.detectedTime, Root.LastKnownTime, Environment.CurrentManagedThreadId));
            };
            Host.OriginalLogRecordRead += Captured.Enqueue;
        }
        internal LiveLogSourceStatus Status(string name) => Host.ReadLiveLogSources().Single(source => source.Name == name);
        public void Dispose() { try { Host.Dispose(); } finally { Directory.Delete(true); } }
    }
    internal static void Run()
    {
        HostLifetimeChecks.RunBounded(ClockPauseAndNames);
        HostLifetimeChecks.RunBounded(PausedImageChanges);
        HostLifetimeChecks.RunBounded(FailurePublication);
        HostLifetimeChecks.RunBounded(TimeoutAndRecovery);
        HostLifetimeChecks.RunBounded(EligibleTieResetsWait);
        HostLifetimeChecks.RunBounded(ReentrantAndImport);
        HostLifetimeChecks.RunBounded(BomAndPartialAdmission);
        HostLifetimeChecks.RunBounded(PartialReaderOpenFailure);
        HostLifetimeChecks.RunBounded(BufferBudgetFailure);
        Console.WriteLine("Live companion append/clock gates, ready-head ties, late rows, bounded pause, images, timeout, cleanup and reentrancy passed.");
    }
    private static void ClockPauseAndNames()
    {
        using var fixture = new Fixture("alpha", "beta");
        fixture.Root.ReadThreadLock = true;
        fixture.Host.OpenLogWithCompanions([new("beta"), new("alpha", new(949))], maxClockWait: TimeSpan.FromMilliseconds(180));
        Append(fixture.Side("beta"), Encoding.Unicode.GetPreamble().Concat(Encoding.Unicode.GetBytes("0|日本\r\n")).ToArray());
        Append(fixture.Side("alpha"), Encoding.GetEncoding(949).GetBytes("0|한글\n"));
        Append(fixture.Main, "0|main\n");
        Wait(() => fixture.Host.ReadLiveLogSources().All(source => source.Queued == 1), "paused sources did not retain bounded input");
        Check(fixture.Seen.IsEmpty, "paused group dispatched raw data");
        fixture.Root.ReadThreadLock = false;
        Wait(() => fixture.Seen.Count == 3, "same-time ready heads not delivered");
        Check(fixture.Seen.Select(row => row.Name).SequenceEqual(["", "alpha", "beta"]), "ready-head tie policy");
        Check(fixture.Seen.Select(row => row.Thread).Distinct().Count() == 1 && fixture.ParserThreads.Distinct().Single() == fixture.Seen.First().Thread,
            "parser and raw callbacks used different live dispatch contexts");
        Check(fixture.Captured.All(record => record.Path == (record.CompanionLogName.Length == 0 ? fixture.Main : fixture.Side(record.CompanionLogName)) && !record.Line.Contains("edited", StringComparison.Ordinal)), "original source/name capture");
        Check(fixture.Captured.Select(record => record.TimeSorter).Distinct().Count() == 3, "same timestamp sorter collision");
        Append(fixture.Side("alpha"), Encoding.GetEncoding(949).GetBytes("10|future\n"));
        Wait(() => fixture.Status("alpha").State == "waiting_for_clock", "future source did not wait for main clock");
        Check(fixture.Seen.Count == 3, "future companion ran before a clock gate");
        fixture.Root.ReadThreadLock = true;
        Thread.Sleep(300);
        Check(fixture.Status("alpha").State == "paused", "pause consumed the clock timeout");
        Append(fixture.Main, "11|advance\n");
        Wait(() => fixture.Status("").Queued == 1, "main append not retained while paused");
        fixture.Root.ReadThreadLock = false;
        Wait(() => fixture.Seen.Count == 5, "main lookahead did not release companion");
        Append(fixture.Side("alpha"), Encoding.GetEncoding(949).GetBytes("1|late\n"));
        Wait(() => fixture.Seen.Count == 6, "late complete record was lost");
        var late = fixture.Seen.Last();
        Check(late.Line == "1|late" && late.Time == Epoch.AddSeconds(1) && late.Clock == Epoch.AddSeconds(11), "late companion rewound clock or lost raw time");
        fixture.Host.Dispose();
        Append(fixture.Main, "12|stopped\n");
        Check(fixture.Seen.Count == 6 && ActGlobals.oFormActMain is null, "callback after returned stop");
        using var exclusive = new FileStream(fixture.Side("alpha"), FileMode.Open, FileAccess.ReadWrite, FileShare.None);
    }
    private static void PausedImageChanges()
    {
        using var fixture = new Fixture("side");
        fixture.Root.ReadThreadLock = true;
        fixture.Host.OpenLogWithCompanions([new("side")]);
        Append(fixture.Side("side"), "100|stale-a\n100|stale-b\n100|stale-c\n");
        Wait(() => fixture.Status("side").Queued == 1, "old generation did not queue");
        File.Move(fixture.Side("side"), fixture.Side("side") + ".old");
        File.WriteAllText(fixture.Side("side"), "0|rotated\n", Encoding.Unicode);
        fixture.Root.ReadThreadLock = false;
        Wait(() => fixture.Seen.Count == 1, "rotation could not invalidate an old future head");
        Check(fixture.Seen.Single().Line == "0|rotated" && fixture.Status("side").Generation >= 2 && fixture.Status("side").Dropped >= 1,
            "rotation kept stale records or codec state");
        fixture.Root.ReadThreadLock = true;
        Append(fixture.Side("side"), Encoding.Unicode.GetBytes("100|stale-again-a\n100|stale-again-b\n100|stale-again-c\n"));
        Wait(() => fixture.Status("side").Queued == 1, "truncation fixture did not queue");
        using (var stream = new FileStream(fixture.Side("side"), FileMode.Open, FileAccess.Write, FileShare.ReadWrite | FileShare.Delete))
        { stream.SetLength(0); stream.Write(Encoding.UTF8.GetBytes("0|fresh\n")); stream.Flush(true); }
        fixture.Root.ReadThreadLock = false;
        Wait(() => fixture.Seen.Count == 2, "truncation did not reset a blocked source");
        Check(fixture.Seen.Last().Line == "0|fresh" && fixture.Host.RecentDiagnostics.Any(text => text.Contains("invalidated", StringComparison.Ordinal) || text.Contains("generation", StringComparison.Ordinal)), "image loss was hidden");
    }
    private sealed class PausedMessage(ManualResetEventSlim entered, ManualResetEventSlim release) : IOException
    {
        public override string Message
        {
            get
            {
                entered.Set();
                Check(release.Wait(TimeSpan.FromSeconds(5)), "failure message was not released");
                return "synthetic terminal reader failure";
            }
        }
    }
    private static void FailurePublication()
    {
        using var fixture = new Fixture("side");
        fixture.Host.OpenLogWithCompanions([new("side")]);
        var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
        var group = fixture.Root.GetType().GetField("follower", flags)!.GetValue(fixture.Root)!;
        var sources = (Array)group.GetType().GetField("sources", flags)!.GetValue(group)!;
        var source = sources.GetValue(1)!;
        var fail = group.GetType().GetMethod("Fail", flags)!;
        using var entered = new ManualResetEventSlim();
        using var release = new ManualResetEventSlim();
        // Pause the actual failure path before it can obtain its diagnostic: no scheduler timing assumption.
        var failure = Task.Run(() => fail.Invoke(group, [source, new PausedMessage(entered, release)]));
        try
        {
            Check(entered.Wait(TimeSpan.FromSeconds(5)), "failure did not reach diagnostic capture");
            var pending = fixture.Status("side");
            Check(pending.State != "failed", "failed state escaped before its diagnostic was available");
        }
        finally
        {
            release.Set();
            Check(failure.Wait(TimeSpan.FromSeconds(5)), "failure publication did not finish");
        }
        var published = fixture.Status("side");
        Check(published.State == "failed" && published.LastError == "synthetic terminal reader failure", "terminal diagnostic not published together");
        // Later recoverable diagnostics cannot replace the reason for the terminal state, even with empty text.
        group.GetType().GetMethod("Report", flags)!.Invoke(group, [new IOException(""), source]);
        Check(fixture.Status("side").LastError == published.LastError, "later report erased terminal cause");
        fixture.Host.OpenLogWithCompanions([new("side")]);
        Check(fixture.Status("side").State == "following" && fixture.Status("side").LastError.Length == 0, "reopen retained old failure");
        group = fixture.Root.GetType().GetField("follower", flags)!.GetValue(fixture.Root)!;
        sources = (Array)group.GetType().GetField("sources", flags)!.GetValue(group)!;
        source = sources.GetValue(1)!;
        fail.Invoke(group, [source, new IOException("")]);
        Check(fixture.Status("side").State == "failed" && fixture.Status("side").LastError == nameof(IOException), "empty failure lacks type fallback");
        fail.Invoke(group, [source, new IOException("later failure")]);
        Check(fixture.Status("side").LastError == nameof(IOException), "repeat failure replaced first cause");
    }
    private static void TimeoutAndRecovery()
    {
        using var fixture = new Fixture("side");
        fixture.Host.OpenLogWithCompanions([new("side")], maxClockWait: TimeSpan.FromMilliseconds(100));
        Append(fixture.Side("side"), "1000|never\n1000|queued\n1000|blocked\n");
        Wait(() => fixture.Status("side").State == "failed", "clock wait did not fail a stalled source");
        Check(fixture.Status("side").LastError.Contains("clock wait", StringComparison.Ordinal) && fixture.Seen.IsEmpty, "timeout diagnostic or exclusion");
        Append(fixture.Main, "1|healthy-main\n");
        Wait(() => fixture.Seen.Count == 1, "one companion failure stopped healthy main");
        fixture.Host.OpenLogWithCompanions([new("side")]);
        Append(fixture.Side("side"), "1|retry\n");
        Wait(() => fixture.Seen.Count == 2, "explicit reopen did not recover source");
        Append(fixture.Side("side"), Encoding.UTF8.GetBytes("1|").Concat(new byte[] { 0xff, 10 }).Concat(Encoding.UTF8.GetBytes("1|valid\n")).ToArray());
        Wait(() => fixture.Seen.Count == 3, "malformed record stopped recovery");
        Check(fixture.Seen.Last().Line == "1|valid" && fixture.Status("side").State != "failed", "recoverable framing error disabled source");
        Append(fixture.Side("side"), "undated tail");
        fixture.Host.Dispose();
        Check(fixture.Seen.Count == 3, "incomplete EOF tail dispatched during shutdown");
    }
    private static void EligibleTieResetsWait()
    {
        using var fixture = new Fixture("side");
        fixture.Host.OpenLogWithCompanions([new("side")], maxClockWait: TimeSpan.FromMilliseconds(400));
        using var callbackReturned = new ManualResetEventSlim();
        string? eligibleState = null;
        fixture.Root.BeforeLogLineRead += (_, info) =>
        {
            if (info.originalLogLine != "10|eligible-tie") return;
            // Capture eligibility before the test can rescue the head with any later main arrival.
            eligibleState = fixture.Status("side").State;
            // The side head is now eligible, but main wins the ready-head tie. A plugin can then rewind main time.
            fixture.Root.LastKnownTime = Epoch;
            Append(fixture.Main, "0|backward\n");
            Wait(() => fixture.Status("").Queued == 1, "backward main record not queued during callback");
            Thread.Sleep(450);
            callbackReturned.Set();
        };
        Append(fixture.Side("side"), "10|waiting\n");
        Wait(() => fixture.Status("side").State == "waiting_for_clock", "side head never began its initial wait");
        Append(fixture.Main, "10|eligible-tie\n");
        Check(callbackReturned.Wait(TimeSpan.FromSeconds(5)), "eligible-tie callback did not finish");
        Check(eligibleState == "following", "eligible but unselected head retained its prior waiting state");
        Wait(() => fixture.Seen.Any(row => row.Line == "0|backward"), "backward main did not dispatch");
        Check(fixture.Status("side").State != "failed", "new future wait reused an expired eligible interval");
        Append(fixture.Main, "11|release\n");
        Wait(() => fixture.Seen.Count == 4, "fresh interval did not admit later clock release");
        Check(fixture.Seen.Select(row => row.Line).SequenceEqual(["10|eligible-tie", "0|backward", "10|waiting", "11|release"]), "eligibility transition changed ready-head order");
    }
    private static void ReentrantAndImport()
    {
        using var fixture = new Fixture("side");
        fixture.Host.OpenLogWithCompanions([new("side")]);
        using var returned = new ManualResetEventSlim();
        LogLineEventDelegate switcher = (_, info) =>
        {
            if (info.originalLogLine != "0|switch") return;
            fixture.Root.OpenLog(false, false);
            returned.Set();
        };
        fixture.Root.BeforeLogLineRead += switcher;
        Append(fixture.Main, "0|switch\n0|stale\n");
        Check(returned.Wait(TimeSpan.FromSeconds(5)), "reentrant group replacement deadlocked");
        fixture.Root.BeforeLogLineRead -= switcher;
        Append(fixture.Main, "0|new-main\n");
        Wait(() => fixture.Seen.Count == 1, "new reader after callback reopen");
        Check(fixture.Seen.Single().Line == "0|new-main" && fixture.Host.ReadLiveLogSources() is [{ Name: "" }], "stale group dispatched after reentrant replacement");
        fixture.Host.OpenLogWithCompanions([new("side")]);
        var import = Path.Combine(fixture.Directory.FullName, "import.log");
        var before = fixture.Seen.Count;
        fixture.Root.ReadThreadLock = true;
        Append(fixture.Side("side"), "0|after-import\n");
        Wait(() => fixture.Status("side").Queued == 1, "live source did not queue before import");
        // Empty import exercises suspension/restoration without invoking fixture live-only callbacks.
        File.WriteAllText(import, "");
        Check(fixture.Host.ImportLog(import).RecordsDispatched == 0 && fixture.Root.ReadThreadLock, "group import suspension/restoration");
        Check(fixture.Seen.Count == before, "live callback escaped import ownership");
        fixture.Root.ReadThreadLock = false;
        Wait(() => fixture.Seen.Count == before + 1, "import suspension did not resume group");
    }
    private static void BomAndPartialAdmission()
    {
        using var fixture = new Fixture("auto", "literal");
        fixture.Root.ReadThreadLock = true;
        fixture.Host.OpenLogWithCompanions([new("auto", new(20127, true)), new("literal", new(20127, true, false))]);
        var bytes = Encoding.UTF8.GetPreamble().Concat(Encoding.UTF8.GetBytes("0|café\n")).ToArray();
        Append(fixture.Side("auto"), bytes); Append(fixture.Side("literal"), bytes); Append(fixture.Main, "0|main\n");
        Wait(() => fixture.Host.ReadLiveLogSources().All(source => source.Queued == 1), "BOM fixture not ready");
        fixture.Root.ReadThreadLock = false;
        Wait(() => fixture.Seen.Count == 2 && fixture.Status("literal").Dropped >= 1, "per-reader BOM policy not applied");
        Check(fixture.Seen.Any(row => row.Name == "auto" && row.Line == "0|café"), "BOM override lost Unicode text");
        try { fixture.Host.OpenLogWithCompanions([new("auto"), new("missing")]); throw new Exception("missing live source accepted"); }
        catch (FileNotFoundException) { }
        // Missing catalog entries fail before candidate readers are created; the previous group is already retired.
        using var mainExclusive = new FileStream(fixture.Main, FileMode.Open, FileAccess.ReadWrite, FileShare.None);
        using var sideExclusive = new FileStream(fixture.Side("auto"), FileMode.Open, FileAccess.ReadWrite, FileShare.None);
    }
    private static void PartialReaderOpenFailure()
    {
        using var fixture = new Fixture("first", "locked");
        fixture.Host.OpenLogWithCompanions([new("first")]);
        Append(fixture.Main, "0|before-failure\n");
        Wait(() => fixture.Seen.Count == 1, "previous group did not run");
        var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
        var oldGroup = fixture.Root.GetType().GetField("follower", flags)!.GetValue(fixture.Root)!;
        try { fixture.Host.OpenLogWithCompanions(Enumerable.Range(0, 9).Select(index => new ImportCompanion("extra" + index)).ToArray()); throw new Exception("Live source limit accepted"); }
        catch (ArgumentException) { }
        try { fixture.Host.OpenLogWithCompanions([new("first")], maxClockWait: TimeSpan.Zero); throw new Exception("Zero clock timeout accepted"); }
        catch (ArgumentOutOfRangeException) { }
        Check(ReferenceEquals(oldGroup, fixture.Root.GetType().GetField("follower", flags)!.GetValue(fixture.Root)), "invalid options retired the healthy group");
        // The selected name order is main -> first -> locked. The catalog uses metadata only,
        // so an exclusive lock rejects the last FileStream open after the first two readers started.
        using (var locked = new FileStream(fixture.Side("locked"), FileMode.Open, FileAccess.ReadWrite, FileShare.None))
        {
            try
            {
                fixture.Host.OpenLogWithCompanions([new("locked"), new("first")]);
                throw new Exception("Exclusively locked final source was accepted");
            }
            catch (IOException error)
            {
                Check(error.Message.Contains(Path.GetFileName(fixture.Side("locked")), StringComparison.Ordinal), "failure was not the final source open");
            }
            Check(!(bool)oldGroup.GetType().GetProperty("IsAlive", flags)!.GetValue(oldGroup)!, "retired group dispatcher remained alive");
            Check(fixture.Host.ReadLiveLogSources().Length == 0 && fixture.Root.LogFilePath == fixture.Main && fixture.Root.LastKnownTime == Epoch,
                "failed candidate replaced live state or remained attached");
            using var main = new FileStream(fixture.Main, FileMode.Open, FileAccess.ReadWrite, FileShare.None);
            using var first = new FileStream(fixture.Side("first"), FileMode.Open, FileAccess.ReadWrite, FileShare.None);
            Check(fixture.Seen.Count == 1, "partial candidate emitted callbacks");
        }
        // The ordinary follower remains usable after a failed group admission.
        fixture.Root.OpenLog(false, false);
        Append(fixture.Main, "0|ordinary-retry\n");
        Wait(() => fixture.Seen.Count == 2, "ordinary follower failed after partial admission");
        Check(fixture.Seen.Last().Line == "0|ordinary-retry" && fixture.Seen.Last().Name == "", "ordinary path was corrupted");
        // Retrying the same explicit set after releasing the lock must also work.
        fixture.Root.ReadThreadLock = true;
        fixture.Host.OpenLogWithCompanions([new("locked"), new("first")]);
        Append(fixture.Main, "0|group-retry\n");
        Append(fixture.Side("first"), "0|first-retry\n");
        Append(fixture.Side("locked"), "0|locked-retry\n");
        Wait(() => fixture.Host.ReadLiveLogSources().All(source => source.Queued == 1), "retry readers did not open");
        fixture.Root.ReadThreadLock = false;
        Wait(() => fixture.Seen.Count == 5, "group retry after failed open");
        Check(fixture.Seen.Skip(2).Select(row => row.Name).SequenceEqual(["", "first", "locked"]), "retry retained stale callbacks or source order");
        Console.WriteLine("Live companion final-source open failure, prior handle release and ordinary/group retry passed.");
    }

    private static void BufferBudgetFailure()
    {
        using var fixture = new Fixture("a", "b", "c");
        fixture.Root.ReadThreadLock = true;
        fixture.Host.OpenLogWithCompanions([new("a"), new("b"), new("c")]);
        var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic;
        var group = fixture.Root.GetType().GetField("follower", flags)!.GetValue(fixture.Root)!;
        var budget = group.GetType().GetField("bufferedCharacters", flags)!;
        long Reserved() => (long)budget.GetValue(group)!;
        var maximum = "0|" + new string('x', 8 * 1024 * 1024 - 2) + "\n";
        Append(fixture.Side("a"), maximum); Append(fixture.Side("a"), maximum);
        Wait(() => Reserved() == 16 * 1024 * 1024, "first source did not apply bounded queue backpressure");
        Append(fixture.Side("b"), maximum); Append(fixture.Side("b"), maximum);
        Wait(() => Reserved() == 32 * 1024 * 1024, "UTF-16 aggregate text budget fixture");
        Append(fixture.Side("c"), "0|over-budget\n");
        Wait(() => fixture.Status("c").State == "failed", "aggregate text overflow did not fail the source");
        Check(fixture.Status("c").LastError.Contains("64 MiB", StringComparison.Ordinal) && fixture.Seen.IsEmpty && Reserved() == 32 * 1024 * 1024,
            "overflow silently dispatched/dropped data or exceeded its reservation bound");
        fixture.Host.Dispose();
        Check(Reserved() == 0, "stop did not release queued and blocked text reservations");
    }

}
