using System.Collections.Concurrent;
using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Text;
using Advanced_Combat_Tracker;

internal static class InitialBoundaryChecks
{
    private static readonly Type Follower = typeof(FormActMain).Assembly.GetType("Advanced_Combat_Tracker.LogFollower", true)!;

    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-initial-boundary-");
        try
        {
            Continuation(Path.Combine(directory.FullName, "ascii.log"), Encoding.UTF8.GetBytes("old partial"), Encoding.UTF8.GetBytes(" suffix\nnext\n"));
            var encoded = Encoding.UTF8.GetBytes("한");
            Continuation(Path.Combine(directory.FullName, "utf8.log"), encoded[..1], encoded[1..].Concat(Encoding.UTF8.GetBytes("\nnext\n")).ToArray());
            Continuation(Path.Combine(directory.FullName, "crlf.log"), Encoding.UTF8.GetBytes("old\r"), Encoding.UTF8.GetBytes("\nnext\n"));
            Continuation(Path.Combine(directory.FullName, "bom-only.log"), Encoding.UTF8.GetPreamble(),
                Encoding.UTF8.GetBytes("next\n"));
            Continuation(Path.Combine(directory.FullName, "bom-partial.log"), Encoding.UTF8.GetPreamble().Concat(Encoding.UTF8.GetBytes("old partial")).ToArray(),
                Encoding.UTF8.GetBytes(" suffix\nnext\n"));
            Reset(Path.Combine(directory.FullName, "truncate.log"), replacement: false);
            Reset(Path.Combine(directory.FullName, "replacement.log"), replacement: true);
        }
        finally { directory.Delete(true); }
        Console.WriteLine("Initial partial-record ASCII/UTF-8 continuation, pause, truncation and replacement boundaries passed.");
    }

    private static IDisposable Create(string path, Action<string> line, Action<Exception> error, Func<bool> paused)
    {
        try
        {
            return (IDisposable)Follower.GetConstructor(BindingFlags.Instance | BindingFlags.NonPublic, null,
                [typeof(string), typeof(Action<string>), typeof(Action<Exception>), typeof(Func<bool>)], null)!
                .Invoke([path, line, error, paused]);
        }
        catch (TargetInvocationException exception) when (exception.InnerException is not null)
        {
            ExceptionDispatchInfo.Capture(exception.InnerException).Throw();
            throw;
        }
    }

    private static void Continuation(string path, byte[] initial, byte[] appended)
    {
        File.WriteAllBytes(path, initial);
        var lines = new ConcurrentQueue<string>();
        var errors = new ConcurrentQueue<Exception>();
        var pause = 1;
        using var observed = new ManualResetEventSlim();
        using var reader = Create(path, lines.Enqueue, errors.Enqueue, () =>
        {
            if (Volatile.Read(ref pause) == 0) return false;
            observed.Set();
            return true;
        });
        Check(observed.Wait(TimeSpan.FromSeconds(5)), "Reader did not pause.");
        Append(path, appended);
        Check(lines.IsEmpty && errors.IsEmpty, "Paused initial continuation was decoded or delivered.");
        Volatile.Write(ref pause, 0);
        Check(SpinWait.SpinUntil(() => lines.Count == 1, TimeSpan.FromSeconds(5)), "Next complete record was not delivered.");
        reader.Dispose();
        Check(lines.ToArray().SequenceEqual(["next"]) && errors.IsEmpty, "Historical suffix was parsed or split UTF-8 emitted a diagnostic.");
    }

    private static void Reset(string path, bool replacement)
    {
        File.WriteAllText(path, "old incomplete record", new UTF8Encoding(false));
        var lines = new ConcurrentQueue<string>();
        var errors = new ConcurrentQueue<Exception>();
        var pause = 1;
        using var observed = new ManualResetEventSlim();
        using var reader = Create(path, lines.Enqueue, errors.Enqueue, () =>
        {
            if (Volatile.Read(ref pause) == 0) return false;
            observed.Set();
            return true;
        });
        Check(observed.Wait(TimeSpan.FromSeconds(5)), "Reader did not pause before reset.");
        if (replacement) File.Move(path, path + ".old");
        // This shorter image makes same-file truncation observable even before the paused reader resumes.
        File.WriteAllText(path, "fresh\n", new UTF8Encoding(true));
        Volatile.Write(ref pause, 0);
        Check(SpinWait.SpinUntil(() => lines.Count == 1, TimeSpan.FromSeconds(5)), "Reset did not clear historical continuation discard.");
        reader.Dispose();
        Check(lines.ToArray().SequenceEqual(["fresh"]) && errors.IsEmpty, "Reset lost a new first record or retained the old partial boundary.");
    }

    private static void Append(string path, byte[] bytes)
    {
        using var stream = new FileStream(path, FileMode.Append, FileAccess.Write, FileShare.ReadWrite | FileShare.Delete);
        stream.Write(bytes);
        stream.Flush();
    }
    private static void Check(bool value, string message)
    {
        if (!value) throw new InvalidOperationException(message);
    }
}
