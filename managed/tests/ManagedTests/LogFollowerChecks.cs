using System.Collections.Concurrent;
using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Text;
using Advanced_Combat_Tracker;

internal static class LogFollowerChecks
{
    private static readonly Type FollowerType = typeof(ActGlobals).Assembly.GetType("Advanced_Combat_Tracker.LogFollower", throwOnError: true)!;

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception("Log follower check failed: " + message);
    }

    private static void Wait(Func<bool> condition, string message) => Check(SpinWait.SpinUntil(condition, TimeSpan.FromSeconds(10)), message);

    private static IDisposable Create(string path, Action<string> onLine, Action<Exception> onError, Func<bool> paused)
    {
        try
        {
            return (IDisposable)FollowerType.GetConstructor(BindingFlags.NonPublic | BindingFlags.Instance, null,
                [typeof(string), typeof(Action<string>), typeof(Action<Exception>), typeof(Func<bool>)], null)!
                .Invoke([path, onLine, onError, paused]);
        }
        catch (TargetInvocationException error) when (error.InnerException is not null)
        {
            ExceptionDispatchInfo.Capture(error.InnerException).Throw();
            throw;
        }
    }

    private static void Append(string path, byte[] bytes)
    {
        using var output = new FileStream(path, FileMode.Append, FileAccess.Write, FileShare.ReadWrite | FileShare.Delete);
        output.Write(bytes);
        output.Flush(flushToDisk: true);
    }

    private static void Append(string path, string text) => Append(path, Encoding.UTF8.GetBytes(text));

    public static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("bxivact-follower-");
        try
        {
            AppendDecodePauseAndTruncate(Path.Combine(directory.FullName, "follow.log"));
            CallbackFailure(Path.Combine(directory.FullName, "callback.log"));
            MalformedUtf8(Path.Combine(directory.FullName, "malformed.log"));
            SelfDispose(Path.Combine(directory.FullName, "self.log"));
            OversizedLine(Path.Combine(directory.FullName, "oversized.log"));
            Replacement(Path.Combine(directory.FullName, "replacement.log"));
        }
        finally { directory.Delete(recursive: true); }
        Console.WriteLine("Log follower end-tail, split UTF-8, framing, pause, truncation, replacement, callback isolation and disposal checks passed.");
    }

    private static void AppendDecodePauseAndTruncate(string path)
    {
        File.WriteAllText(path, "existing content\n", new UTF8Encoding(false));
        var lines = new ConcurrentQueue<string>();
        var errors = new ConcurrentQueue<Exception>();
        var polls = 0;
        var pause = 0;
        using var pauseSeen = new ManualResetEventSlim();
        using var follower = Create(path, lines.Enqueue, errors.Enqueue, () =>
        {
            Interlocked.Increment(ref polls);
            if (Volatile.Read(ref pause) == 0) return false;
            pauseSeen.Set();
            return true;
        });
        void DrainReadCycles()
        {
            // Two later polls prove a read cycle completed after the write, without timing a tiny sleep.
            var target = Volatile.Read(ref polls) + 2;
            Wait(() => Volatile.Read(ref polls) >= target, "reader did not complete read cycles");
        }

        Append(path, "first\r\n\nfragment");
        Wait(() => lines.Count == 2, "initial appended lines were not delivered");
        DrainReadCycles();
        Check(lines.ToArray().SequenceEqual(["first", ""]), "existing content was replayed or line framing changed");
        var encoded = Encoding.UTF8.GetBytes("한글");
        Append(path, encoded[..1]);
        DrainReadCycles();
        Check(errors.IsEmpty && lines.Count == 2, "split UTF-8 or incomplete line emitted early");
        Append(path, encoded[1..]);
        Append(path, "\r\n");
        Wait(() => lines.Count == 3, "split UTF-8 line was not completed");
        Check(lines.ToArray()[2] == "fragment한글", "split UTF-8 decoder state was lost");

        Volatile.Write(ref pause, 1);
        Check(pauseSeen.Wait(TimeSpan.FromSeconds(10)), "reader did not acknowledge pause");
        Append(path, "paused\n");
        DrainReadCycles();
        Check(lines.Count == 3, "paused reader delivered a line");
        Volatile.Write(ref pause, 0);
        Wait(() => lines.Count == 4, "resume did not deliver retained file content");
        Check(lines.ToArray()[3] == "paused", "resume changed the line");

        Append(path, "discarded fragment");
        Append(path, encoded[..1]);
        DrainReadCycles();
        using (var output = new FileStream(path, FileMode.Open, FileAccess.Write, FileShare.ReadWrite | FileShare.Delete))
        {
            output.SetLength(0);
            output.Flush(flushToDisk: true);
        }
        DrainReadCycles();
        Append(path, "fresh\n");
        Wait(() => lines.Count == 5, "reader did not restart after truncation");
        Check(lines.ToArray()[4] == "fresh" && errors.IsEmpty, "truncation retained stale text or UTF-8 state");

        follower.Dispose();
        Append(path, "after disposal\n");
        Check(lines.Count == 5 && errors.IsEmpty, "disposed reader invoked callbacks");
        // Joining the worker is the synchronization proof; a delay cannot prove callback exclusion.
        Check(!((Thread)FollowerType.GetField("worker", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(follower)!).IsAlive,
            "Dispose returned before the worker exited");
    }

    private static void CallbackFailure(string path)
    {
        File.WriteAllText(path, "");
        var lines = new ConcurrentQueue<string>();
        var errors = new ConcurrentQueue<Exception>();
        using var follower = Create(path, line =>
        {
            lines.Enqueue(line);
            if (line == "throw") throw new InvalidOperationException("line callback failed");
        }, errors.Enqueue, () => false);
        Append(path, "throw\nafter\n");
        Wait(() => lines.Count == 2 && errors.Count == 1, "callback failure stopped later lines or was not reported");
        Check(lines.ToArray().SequenceEqual(["throw", "after"]), "callback delivery order changed");
        Check(errors.TryPeek(out var error) && error is InvalidOperationException && error.Message == "line callback failed",
            "callback error identity was lost");
    }

    private static void SelfDispose(string path)
    {
        File.WriteAllText(path, "");
        var lines = new ConcurrentQueue<string>();
        var errors = new ConcurrentQueue<Exception>();
        using var returned = new ManualResetEventSlim();
        IDisposable? follower = null;
        follower = Create(path, line =>
        {
            lines.Enqueue(line);
            follower!.Dispose();
            returned.Set();
        }, errors.Enqueue, () => false);
        Append(path, "first\nsecond\n");
        // Do not synchronously dispose again after a timeout: a broken self-join must fail rather than hang this test.
        Check(returned.Wait(TimeSpan.FromSeconds(10)), "self-disposal deadlocked");
        follower.Dispose();
        Check(lines.ToArray().SequenceEqual(["first"]) && errors.IsEmpty, "self-disposal allowed another callback");
    }

    private static void MalformedUtf8(string path)
    {
        File.WriteAllText(path, "");
        var errors = new ConcurrentQueue<Exception>();
        var lines = new ConcurrentQueue<string>();
        using var follower = Create(path, lines.Enqueue, errors.Enqueue, () => false);
        Append(path, [0xc3, (byte)'\n']);
        Append(path, "valid after malformed\n");
        Wait(() => errors.Count == 1 && lines.Count == 1, "malformed UTF-8 stopped later valid records or was not reported");
        Check(lines.TryPeek(out var line) && line == "valid after malformed", "malformed record was replaced or combined with the next line");
        Check(errors.TryPeek(out var error) && error is DecoderFallbackException, "malformed UTF-8 diagnostic type changed");
    }

    private static void OversizedLine(string path)
    {
        File.WriteAllText(path, "");
        var errors = new ConcurrentQueue<Exception>();
        var lines = new ConcurrentQueue<string>();
        using var follower = Create(path, lines.Enqueue, errors.Enqueue, () => false);
        Append(path, new string('x', 8 * 1024 * 1024 + 1));
        Wait(() => !errors.IsEmpty, "oversized pending line was not rejected");
        Append(path, "\nvalid after oversized\n");
        Wait(() => lines.Count == 1, "oversized record stopped later valid records");
        follower.Dispose();
        Check(lines.TryPeek(out var line) && line == "valid after oversized" && errors.Count == 1 &&
            errors.TryPeek(out var error) && error is InvalidDataException,
            "oversized record was emitted, was dropped silently or caused repeated warnings");
    }

    private static void Replacement(string path)
    {
        File.WriteAllText(path, "history\n", new UTF8Encoding(false));
        var lines = new ConcurrentQueue<string>();
        var errors = new ConcurrentQueue<Exception>();
        var pause = 1;
        var polls = 0;
        using var paused = new ManualResetEventSlim();
        using var follower = Create(path, lines.Enqueue, errors.Enqueue, () =>
        {
            Interlocked.Increment(ref polls);
            if (Volatile.Read(ref pause) == 0) return false;
            paused.Set();
            return true;
        });
        Check(paused.Wait(TimeSpan.FromSeconds(10)), "replacement reader did not pause");
        Append(path, "old\nfragment");
        Append(path, [0xed]);
        var length = new FileInfo(path).Length;
        var modified = File.GetLastWriteTimeUtc(path);
        File.Move(path, path + ".1");
        File.WriteAllText(path, "new-one\nnew-two-x\n", new UTF8Encoding(true));
        File.SetLastWriteTimeUtc(path, modified);
        Check(new FileInfo(path).Length == length && File.GetLastWriteTimeUtc(path) == modified,
            "replacement fixture must defeat size/timestamp heuristics");
        Volatile.Write(ref pause, 0);
        Wait(() => lines.Count == 3, "same-path replacement was not read from its beginning");
        Check(lines.ToArray().SequenceEqual(["old", "new-one", "new-two-x"]),
            "replacement lost the old complete tail, retained partial UTF-8, or emitted a BOM");
        Check(errors.Count == 1 && errors.TryPeek(out var error) && error is InvalidDataException &&
            error.Message.Contains("replacement", StringComparison.Ordinal), "discarded old fragment was not reported");

        File.Move(path, path + ".2");
        var nextPoll = Volatile.Read(ref polls) + 4;
        Wait(() => Volatile.Read(ref polls) >= nextPoll, "missing replacement path stopped polling");
        Append(path + ".2", "old-during-gap\n");
        Wait(() => lines.Count == 4, "missing path stopped the still-open old file");
        Check(lines.ToArray()[3] == "old-during-gap" && errors.Count == 1, "temporary path absence became a read failure");
        File.WriteAllText(path, "third\n", new UTF8Encoding(true));
        Wait(() => lines.Count == 5, "reader did not recover after replacement path reappeared");
        follower.Dispose();
        Check(lines.ToArray()[4] == "third" && errors.Count == 1, "replacement recovery changed records or diagnostics");
    }
}
