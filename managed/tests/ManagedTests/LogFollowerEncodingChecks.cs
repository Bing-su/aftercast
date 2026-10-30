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
        InitialTailBom(directory);
        HistoricalPartial(directory);
        ReplacementCodec(directory);
        InvalidSelectedBytes(directory);
        Console.WriteLine("Live selected encodings, split BOM/code units, CR framing, truncation, historical tails and replacement codec checks passed.");
    }

    private static void InitialTailBom(string directory)
    {
        foreach (var codePage in new[] { 65001, 1200, 1201, 12000, 12001 })
        {
            var selected = new UTF8Encoding(false, true);
            var incoming = Encoding.GetEncoding(codePage);
            var path = Path.Combine(directory, $"tail-bom-{codePage}.log");
            File.WriteAllBytes(path, selected.GetBytes("history\n"));
            var lines = new ConcurrentQueue<string>();
            var errors = new ConcurrentQueue<Exception>();
            var polls = 0;
            using var follower = EncodedFollower(path, selected, lines, errors, () => { Interlocked.Increment(ref polls); return false; });
            foreach (var value in incoming.GetPreamble())
            {
                Append(path, [value]);
                var target = Volatile.Read(ref polls) + 2;
                Wait(() => Volatile.Read(ref polls) >= target, "tail BOM split read");
                Check(lines.IsEmpty && errors.IsEmpty, "tail BOM emitted before a complete record");
            }
            Append(path, incoming.GetBytes("한글日本\n"));
            Wait(() => lines.Count == 1, $"tail BOM {codePage} line");
            Check(lines.TryPeek(out var line) && line == "한글日本" && errors.IsEmpty, $"tail BOM {codePage} decoding");
            Append(path, incoming.GetPreamble().Concat(incoming.GetBytes("later\n")).ToArray());
            Wait(() => lines.Count == 2, $"later BOM {codePage} line");
            Check(lines.ToArray()[1] == "\uFEFFlater" && errors.IsEmpty, $"later BOM {codePage} must remain content");
        }
        // A fresh wide reader begins at an odd byte offset; the BOM defines a new alignment origin.
        var oddPath = Path.Combine(directory, "tail-bom-odd.log");
        File.WriteAllBytes(oddPath, Encoding.UTF8.GetBytes("xx\n"));
        var oddLines = new ConcurrentQueue<string>(); var oddErrors = new ConcurrentQueue<Exception>();
        using (var odd = EncodedFollower(oddPath, new UTF8Encoding(false, true), oddLines, oddErrors, () => false))
        {
            Append(oddPath, Encoding.Unicode.GetPreamble().Concat(Encoding.Unicode.GetBytes("wide\n")).ToArray());
            Wait(() => oddLines.Count == 1, "odd tail BOM line");
            Check(oddLines.TryPeek(out var line) && line == "wide" && oddErrors.IsEmpty, "tail BOM alignment");
        }
        var crPath = Path.Combine(directory, "tail-bom-cr.log");
        File.WriteAllBytes(crPath, Encoding.UTF8.GetBytes("history\r"));
        var crLines = new ConcurrentQueue<string>(); var crErrors = new ConcurrentQueue<Exception>();
        using (var cr = EncodedFollower(crPath, new UTF8Encoding(false, true), crLines, crErrors, () => false))
        {
            Append(crPath, Encoding.Unicode.GetPreamble().Concat(Encoding.Unicode.GetBytes("\nwide\n")).ToArray());
            Wait(() => crLines.Count == 2, "tail BOM after CR");
            Check(crLines.ToArray().SequenceEqual(["", "wide"]) && crErrors.IsEmpty, "tail BOM CR boundary");
        }
        var plainCrPath = Path.Combine(directory, "tail-no-bom-cr.log");
        File.WriteAllBytes(plainCrPath, Encoding.UTF8.GetBytes("history\r"));
        var plainLines = new ConcurrentQueue<string>(); var plainErrors = new ConcurrentQueue<Exception>();
        using (var plain = EncodedFollower(plainCrPath, new UTF8Encoding(false, true), plainLines, plainErrors, () => false))
        {
            Append(plainCrPath, "\nfresh\n");
            Wait(() => plainLines.Count == 1, "plain tail after CR");
            Check(plainLines.TryPeek(out var line) && line == "fresh" && plainErrors.IsEmpty, "plain historical CRLF was not coalesced");
        }
        // Disabling foreign BOM detection still consumes the configured codec's own preamble.
        var explicitPath = Path.Combine(directory, "tail-bom-explicit.log");
        File.WriteAllBytes(explicitPath, Encoding.UTF8.GetBytes("history\n"));
        var explicitLines = new ConcurrentQueue<string>(); var explicitErrors = new ConcurrentQueue<Exception>();
        var constructor = FollowerType.GetConstructor(BindingFlags.NonPublic | BindingFlags.Instance, null,
            [typeof(string), typeof(Action<string>), typeof(Action<Exception>), typeof(Func<bool>), typeof(bool), typeof(Encoding), typeof(Action), typeof(bool)], null)!;
        using (var follower = (IDisposable)constructor.Invoke([explicitPath, (Action<string>)explicitLines.Enqueue, (Action<Exception>)explicitErrors.Enqueue, (Func<bool>)(() => false), false, new UTF8Encoding(true, true), null, false]))
        {
            Append(explicitPath, Encoding.UTF8.GetPreamble().Concat(Encoding.UTF8.GetBytes("own\n")).ToArray());
            Wait(() => explicitLines.Count == 1, "explicit own preamble");
            Check(explicitLines.TryPeek(out var first) && first == "own", "configured preamble was not consumed");
            Check(explicitErrors.IsEmpty, "configured preamble produced an error");
        }
        var foreignPath = Path.Combine(directory, "tail-bom-explicit-foreign.log");
        File.WriteAllBytes(foreignPath, Encoding.UTF8.GetBytes("history\n"));
        var foreignLines = new ConcurrentQueue<string>(); var foreignErrors = new ConcurrentQueue<Exception>();
        using (var follower = (IDisposable)constructor.Invoke([foreignPath, (Action<string>)foreignLines.Enqueue, (Action<Exception>)foreignErrors.Enqueue, (Func<bool>)(() => false), false, new UTF8Encoding(false, true), null, false]))
        {
            Append(foreignPath, [0xff, 0xfe, 0x0a]);
            Wait(() => foreignErrors.Count == 1, "foreign initial BOM without detection");
            Append(foreignPath, "fresh\n");
            Wait(() => foreignLines.Count == 1, "strict decoding recovers after foreign bytes");
            Check(foreignLines.TryPeek(out var line) && line == "fresh" && foreignErrors.TryPeek(out var error) && error is DecoderFallbackException, "foreign BOM changed an explicitly selected codec");
        }
        // The selected encoding's own preamble wins even when it prefixes a longer BOM.
        var priorityPath = Path.Combine(directory, "tail-bom-priority.log");
        var historicalBytes = Encoding.Unicode.GetBytes("history\n");
        var ambiguous = Encoding.UTF32.GetPreamble().Concat(Encoding.UTF32.GetBytes("A\n")).ToArray();
        File.WriteAllBytes(priorityPath, historicalBytes);
        using var expectedStream = new MemoryStream();
        expectedStream.Write(historicalBytes); var offset = expectedStream.Position;
        expectedStream.Write(ambiguous); expectedStream.Position = offset;
        using var expectedReader = new StreamReader(expectedStream, Encoding.Unicode, true, 128, leaveOpen: true);
        var expectedLine = expectedReader.ReadLine();
        var priorityLines = new ConcurrentQueue<string>(); var priorityErrors = new ConcurrentQueue<Exception>();
        using (var follower = EncodedFollower(priorityPath, Encoding.Unicode, priorityLines, priorityErrors, () => false))
        {
            Append(priorityPath, ambiguous);
            Wait(() => priorityLines.Count == 1, "selected preamble priority");
            Check(priorityLines.TryPeek(out var line) && line == expectedLine && priorityErrors.IsEmpty, "selected preamble priority differs from StreamReader");
        }
        // A partial historical record as a fresh source, even if its continuation starts with BOM bytes.
        var partialPath = Path.Combine(directory, "tail-bom-partial.log");
        File.WriteAllBytes(partialPath, Encoding.UTF8.GetBytes("partial"));
        var partialLines = new ConcurrentQueue<string>(); var partialErrors = new ConcurrentQueue<Exception>();
        using (var partial = EncodedFollower(partialPath, new UTF8Encoding(false, true), partialLines, partialErrors, () => false))
        {
            Append(partialPath, Encoding.UTF8.GetPreamble().Concat(Encoding.UTF8.GetBytes("continuation\nfresh\n")).ToArray());
            Wait(() => partialLines.Count == 1, "historical continuation discarded");
            Check(partialLines.TryPeek(out var line) && line == "fresh" && partialErrors.IsEmpty, "partial history was replayed");
        }
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
