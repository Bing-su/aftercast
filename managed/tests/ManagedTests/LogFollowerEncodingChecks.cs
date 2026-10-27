using System.Collections.Concurrent;
using System.Reflection;
using System.Text;

internal static partial class LogFollowerChecks
{
    private static IDisposable EncodedFollower(string path, Encoding encoding, ConcurrentQueue<string> lines,
        ConcurrentQueue<Exception> errors, Func<bool> paused) =>
        (IDisposable)FollowerType.GetConstructor(BindingFlags.NonPublic | BindingFlags.Instance, null,
            [typeof(string), typeof(Action<string>), typeof(Action<Exception>), typeof(Func<bool>), typeof(bool), typeof(Encoding)], null)!
            .Invoke([path, (Action<string>)lines.Enqueue, (Action<Exception>)errors.Enqueue, paused, false, encoding]);

    private static void SelectedEncodings(string directory)
    {
        Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
        foreach (var codePage in new[] { 65001, 1200, 1201, 12000, 12001, 932, 949, 1252, 20127 })
        {
            var encoding = Encoding.GetEncoding(codePage, EncoderFallback.ExceptionFallback, DecoderFallback.ExceptionFallback);
            var text = codePage == 932 ? "日本語" : codePage == 949 ? "한글" : codePage == 1252 ? "café" : codePage == 20127 ? "ascii" : "한글日本語🙂";
            var path = Path.Combine(directory, $"encoding-{codePage}.log");
            File.WriteAllBytes(path, []);
            var lines = new ConcurrentQueue<string>();
            var errors = new ConcurrentQueue<Exception>();
            var polls = 0;
            using var follower = EncodedFollower(path, encoding, lines, errors, () => { Interlocked.Increment(ref polls); return false; });
            void Drain()
            {
                var target = Volatile.Read(ref polls) + 2;
                Wait(() => Volatile.Read(ref polls) >= target, $"{codePage} read cycle");
            }
            // Force short reads across every BOM/code-unit boundary, with no flush at temporary EOF.
            foreach (var value in encoding.GetPreamble().Concat(encoding.GetBytes(text)))
            {
                Append(path, [value]);
                Drain();
                Check(lines.IsEmpty && errors.IsEmpty, $"{codePage} premature decoding");
            }
            Append(path, encoding.GetBytes("\r"));
            Wait(() => lines.Count == 1, $"{codePage} CR completion");
            Append(path, encoding.GetBytes("\nsecond\n"));
            Wait(() => lines.Count == 2, $"{codePage} CRLF framing");
            Check(lines.ToArray().SequenceEqual([text, "second"]) && errors.IsEmpty, $"{codePage} originals");

            // Truncation drops an incomplete code unit and restores the captured codec.
            Append(path, encoding.GetBytes("pending")[..1]);
            Drain();
            using (var output = new FileStream(path, FileMode.Open, FileAccess.Write, FileShare.ReadWrite | FileShare.Delete))
                output.SetLength(0);
            Drain();
            Append(path, encoding.GetPreamble().Concat(encoding.GetBytes("fresh\n")).ToArray());
            Wait(() => lines.Count == 3, $"{codePage} truncation reset");
            Check(lines.ToArray()[2] == "fresh" && errors.Count == 1 && errors.TryPeek(out var truncated) && truncated is InvalidDataException && truncated.Message.Contains("truncation", StringComparison.Ordinal), $"{codePage} stale pending state");
            follower.Dispose();
            Append(path, encoding.GetBytes("after stop\n"));
            Check(lines.Count == 3, $"{codePage} callback after stop");
        }
        HistoricalPartial(directory);
        ReplacementCodec(directory);
        InvalidSelectedBytes(directory);
        Console.WriteLine("Live selected encodings, split BOM/code units, CR framing, truncation, historical tails and replacement codec checks passed.");
    }

    private static void HistoricalPartial(string directory)
    {
        foreach (var codePage in new[] { 65001, 1200, 1201, 12000, 12001, 932, 949 })
        {
            var encoding = Encoding.GetEncoding(codePage);
            var path = Path.Combine(directory, $"partial-{codePage}.log");
            var historical = encoding.GetBytes("historical");
            // Start in the middle of the last historical code unit where possible.
            var cutoff = historical.Length - 1;
            File.WriteAllBytes(path, historical[..cutoff]);
            var lines = new ConcurrentQueue<string>();
            var errors = new ConcurrentQueue<Exception>();
            using var follower = EncodedFollower(path, encoding, lines, errors, () => false);
            Append(path, historical[cutoff..].Concat(encoding.GetBytes("\nnew\n")).ToArray());
            Wait(() => lines.Count == 1, $"{codePage} historical partial tail");
            follower.Dispose();
            Check(lines.ToArray().SequenceEqual(["new"]) && errors.IsEmpty, $"{codePage} replayed history");
        }
    }

    private static void ReplacementCodec(string directory)
    {
        var path = Path.Combine(directory, "codec-rotation.log");
        File.WriteAllBytes(path, []);
        var lines = new ConcurrentQueue<string>();
        var errors = new ConcurrentQueue<Exception>();
        using var follower = EncodedFollower(path, new UTF8Encoding(false, true), lines, errors, () => false);
        Append(path, Encoding.Unicode.GetPreamble().Concat(Encoding.Unicode.GetBytes("wide\n")).ToArray());
        Wait(() => lines.Count == 1, "BOM selected wide codec");
        File.Move(path, path + ".old");
        File.WriteAllBytes(path, Encoding.UTF8.GetBytes("narrow\n"));
        Wait(() => lines.Count == 2, "rotation restored configured codec");
        follower.Dispose();
        Check(lines.ToArray().SequenceEqual(["wide", "narrow"]) && errors.IsEmpty, "rotation retained BOM codec");
    }

    private static void InvalidSelectedBytes(string directory)
    {
        foreach (var replace in new[] { false, true })
        {
            var path = Path.Combine(directory, $"invalid-{replace}.log");
            File.WriteAllBytes(path, []);
            var lines = new ConcurrentQueue<string>();
            var errors = new ConcurrentQueue<Exception>();
            using var follower = EncodedFollower(path, new UTF8Encoding(false, !replace), lines, errors, () => false);
            Append(path, [0xff, 10, (byte)'o', (byte)'k', 10]);
            Wait(() => lines.Count == (replace ? 2 : 1), "invalid byte recovery");
            follower.Dispose();
            Check(lines.ToArray().SequenceEqual(replace ? ["\uFFFD", "ok"] : ["ok"]) && errors.Count == (replace ? 0 : 1), "fallback policy changed");
        }
    }
}
