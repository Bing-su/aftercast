using System.Collections.Concurrent;
using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Text;
using System.Text.RegularExpressions;
using Advanced_Combat_Tracker;

internal static class LogInitialZoneChecks
{
    private static readonly Type Follower = typeof(ActGlobals).Assembly.GetType("Advanced_Combat_Tracker.LogFollower", true)!;
    private static readonly Regex Pattern = new("^zone:(?<zone>.+)$");
    private static void Check(bool value, string message)
    {
        if (!value) throw new Exception("Initial zone: " + message);
    }
    private static IDisposable Open(string path, ConcurrentQueue<Exception> errors) => (IDisposable)Follower
        .GetConstructor(BindingFlags.NonPublic | BindingFlags.Instance, null,
            [typeof(string), typeof(Action<string>), typeof(Action<Exception>), typeof(Func<bool>), typeof(bool), typeof(Encoding)], null)!
        .Invoke([path, (Action<string>)(_ => throw new Exception("Snapshot replayed history")),
            (Action<Exception>)errors.Enqueue, (Func<bool>)(() => false), true, Encoding.GetEncoding(949)]);
    private static string? Find(IDisposable follower, Func<bool>? canceled = null)
    {
        try { return (string?)Follower.GetMethod("ReadInitialZone", BindingFlags.Instance | BindingFlags.NonPublic)!
            .Invoke(follower, [Pattern, canceled ?? (() => false)]); }
        catch (TargetInvocationException error) when (error.InnerException is not null)
        {
            ExceptionDispatchInfo.Capture(error.InnerException).Throw();
            throw;
        }
    }
    internal static void Run()
    {
        Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
        var directory = Directory.CreateTempSubdirectory("aftercast-initial-zone-");
        try
        {
            foreach (var codePage in new[] { 65001, 1200, 1201, 12000, 12001 })
            foreach (var separator in new[] { "\r", "\n", "\r\n" })
            {
                var encoding = Encoding.GetEncoding(codePage);
                var path = Path.Combine(directory.FullName, $"{codePage}-{separator.Length}-{(int)separator[0]}.log");
                File.WriteAllBytes(path, encoding.GetPreamble().Concat(encoding.GetBytes(
                    "zone:old" + separator + new string('x', 5000) + separator + "zone:한글日本🙂" + separator +
                    "zone:ignored combat by fixture" + separator + "zone:incomplete")).ToArray());
                var errors = new ConcurrentQueue<Exception>();
                using var follower = Open(path, errors);
                Check(Find(follower) == "한글日本🙂" && errors.IsEmpty, $"{codePage} {separator.Length} latest complete zone");
            }
            foreach (var codePage in new[] { 65001, 1200, 1201, 12000, 12001 })
            {
                var encoding = Encoding.GetEncoding(codePage);
                var path = Path.Combine(directory.FullName, $"tail-{codePage}.log");
                File.WriteAllBytes(path, encoding.GetPreamble());
                var errors = new ConcurrentQueue<Exception>();
                using (var empty = Open(path, errors)) Check(Find(empty) is null, "BOM-only history selected a zone");
                var partial = encoding.GetBytes("zone:partial");
                File.WriteAllBytes(path, encoding.GetPreamble().Concat(encoding.GetBytes("zone:complete\r\n")).Concat(partial[..^1]).ToArray());
                using var follower = Open(path, errors);
                Check(Find(follower) == "complete" && errors.IsEmpty, "incomplete code unit escaped historical tail exclusion");
            }
            ReplacementAndBounds(directory.FullName);
            MalformedAndEmpty(directory.FullName);
            OversizedAndCanceled(directory.FullName);
        }
        finally { directory.Delete(true); }
        Console.WriteLine("Initial zone BOM/default replacement, encoded CR/LF, bounded reverse scan, snapshot identity, cancellation and truncation checks passed.");
    }
    private static void ReplacementAndBounds(string directory)
    {
        var path = Path.Combine(directory, "snapshot.log");
        File.WriteAllText(path, "zone:captured\r", new UnicodeEncoding(true, true));
        var errors = new ConcurrentQueue<Exception>();
        using (var follower = Open(path, errors))
        {
            File.AppendAllText(path, "zone:later\r", new UnicodeEncoding(true, false));
            File.Move(path, path + ".old");
            File.WriteAllText(path, "zone:replacement\n", new UTF8Encoding(true));
            Check(Find(follower) == "captured" && errors.IsEmpty, "snapshot followed replacement or appended past captured end");
        }
        using (var next = Open(path, errors))
            Check(Find(next) == "replacement", "new reader retained old BOM or identity");
        using (var truncated = Open(path, errors))
        {
            using (var output = new FileStream(path, FileMode.Open, FileAccess.Write, FileShare.ReadWrite | FileShare.Delete)) output.SetLength(0);
            try { Find(truncated); throw new Exception("Truncation was accepted"); }
            catch (EndOfStreamException) { }
        }
        File.WriteAllText(path, "zone:owned\n", new UTF8Encoding(false));
        using (var released = Open(path, errors))
        {
            Follower.GetMethod("CompleteInitialSnapshot", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(released, null);
            try { Find(released); throw new Exception("Released snapshot accepted a read"); }
            catch (InvalidOperationException) { }
        }
        using var stopped = Open(path, errors);
        stopped.Dispose();
        Check(Find(stopped) is null, "disposed snapshot remained accessible");
    }
    private static void MalformedAndEmpty(string directory)
    {
        var path = Path.Combine(directory, "malformed.log");
        foreach (var (encoding, invalid) in new (Encoding, byte[])[] {
            (new UTF8Encoding(false), [0xff]), (Encoding.Unicode, [0, 0xd8]),
            (Encoding.BigEndianUnicode, [0xd8, 0]), (Encoding.UTF32, [0, 0, 0x11, 0]),
            (new UTF32Encoding(true, true), [0, 0x11, 0, 0]) })
        {
            File.WriteAllBytes(path, encoding.GetPreamble().Concat(encoding.GetBytes("zone:")).Concat(invalid).Concat(encoding.GetBytes("\n")).ToArray());
            var errors = new ConcurrentQueue<Exception>();
            using var follower = Open(path, errors);
            Check(Find(follower) == "\uFFFD" && errors.IsEmpty, $"{encoding.CodePage} default replacement policy");
        }
        foreach (var content in new[] { "", "zone:unterminated", "nothing\r\n", "\r\n\r\n" })
        {
            File.WriteAllText(path, content, new UTF8Encoding(false));
            var errors = new ConcurrentQueue<Exception>();
            using var follower = Open(path, errors);
            Check(Find(follower) is null && errors.IsEmpty, "empty/nonmatching/partial record selected");
        }
    }
    private static void OversizedAndCanceled(string directory)
    {
        var path = Path.Combine(directory, "bounded.log");
        File.WriteAllText(path, "zone:older\n" + new string('x', 8 * 1024 * 1024 + 1) + "\n", new UTF8Encoding(false));
        var errors = new ConcurrentQueue<Exception>();
        using var follower = Open(path, errors);
        var polls = 0;
        Check(Find(follower, () => ++polls >= 3) is null && polls == 3 && errors.IsEmpty, "search cancellation did not stop before large record");
        Check(Find(follower) == "older" && errors.Count == 1 && errors.TryPeek(out var error) && error is InvalidDataException,
            "oversized record was not bounded/reported or prevented older selection");
    }
}
