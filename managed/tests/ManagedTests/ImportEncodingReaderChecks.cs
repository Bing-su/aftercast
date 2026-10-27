using System.Reflection;
using System.Text;
using Aftercast.ActCompat;

internal static partial class ImportLogReaderChecks
{
    private static Encoding Selected(int codePage, bool strict = true)
    {
        Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
        var encoding = (Encoding)Encoding.GetEncoding(codePage).Clone();
        if (strict) encoding.DecoderFallback = DecoderFallback.ExceptionFallback;
        return encoding;
    }
    private static IDisposable Encoded(byte[] bytes, Encoding encoding, bool bom = true, int chunk = 1) =>
        Invoke(() => (IDisposable)ReaderType.GetConstructor(BindingFlags.Instance | BindingFlags.NonPublic,
            null, [typeof(Stream), typeof(Encoding), typeof(bool)], null)!.Invoke([new ShortReadStream(bytes, chunk), encoding, bom]));
    private static void EncodingCases()
    {
        foreach (var codePage in new[] { 65001, 1200, 1201, 12000, 12001, 932, 949, 1252, 20127 })
        {
            var encoding = Selected(codePage);
            var text = codePage switch { 932 => "日本語", 949 => "한글", 1252 => "café", 20127 => "ascii", _ => "한글日本語🙂" };
            var marker = codePage is 65001 or 1200 or 1201 or 12000 or 12001 ? "\uFEFF" : "";
            var records = new[] { text, "", marker + text, text, "last" };
            var source = encoding.GetBytes($"{text}\r\n\r\n{marker}{text}\r{text}\nlast");
            foreach (var chunk in new[] { 1, 7, 4096 })
            {
                using (var reader = Encoded(source, encoding, chunk: chunk))
                    Check(ReadAll(reader).SequenceEqual(records), $"selected {codePage} framing {chunk}");
                using (var reader = Encoded(source, encoding, chunk: chunk))
                    Check(ReadAllAsync(reader).GetAwaiter().GetResult().SequenceEqual(records), $"async selected {codePage} framing {chunk}");
            }
            if (encoding.GetPreamble().Length == 0) continue;
            var withBom = encoding.GetPreamble().Concat(source).ToArray();
            using (var reader = Encoded(withBom, Selected(65001)))
                Check(ReadAll(reader).SequenceEqual(records), $"BOM override {codePage} without guessing");
            using (var reader = Encoded(encoding.GetPreamble(), Selected(65001)))
                Check(ReadAllAsync(reader).GetAwaiter().GetResult().Length == 0, $"BOM only {codePage}");
        }
        // These characters look like delimiters at the byte level, but are not CR/LF UTF code units.
        foreach (var codePage in new[] { 1200, 1201, 12000, 12001 })
        {
            var text = "\u0a00\u0d00\u010a\u010d🙂\n끝";
            using var reader = Encoded(Selected(codePage).GetBytes(text), Selected(codePage));
            Check(ReadAll(reader).SequenceEqual(new[] { text.Split('\n')[0], "끝" }), $"aligned delimiters {codePage}");
        }
        PreamblePrecedence();
        EncodingFailures();
        WideRecordLimits();
    }
    private static void PreamblePrecedence()
    {
        foreach (var sourcePage in new[] { 65001, 1200, 1201, 12000, 12001 })
        foreach (var selectedPage in new[] { 65001, 1200, 1201, 12000, 12001, 20127, 932 })
        foreach (var detect in new[] { true, false })
        {
            var source = Selected(sourcePage);
            var bytes = source.GetPreamble().Concat(source.GetBytes("a\r\nb\nlast")).ToArray();
            // Replacement avoids making this valid-byte framing matrix a test of the intentional strict-BOM difference.
            var selected = Selected(selectedPage, false);
            using var reference = new StreamReader(new MemoryStream(bytes), selected, detect);
            var expected = new List<string>();
            while (reference.ReadLine() is { } line) expected.Add(line);
            using var reader = Encoded(bytes, selected, detect);
            Check(ReadAllAsync(reader).GetAwaiter().GetResult().SequenceEqual(expected), $"StreamReader preamble precedence {sourcePage}/{selectedPage}/{detect}");
        }
    }
    private static void EncodingFailures()
    {
        foreach (var (codePage, invalid) in new[] {
            (65001, new byte[] { 0xff }), (932, new byte[] { 0x82 }), (949, new byte[] { 0x81 }),
            (1200, new byte[] { 0, 0xd8 }), (1201, new byte[] { 0xd8, 0 }),
            (12000, new byte[] { 0, 0, 0x11, 0 }), (12001, new byte[] { 0, 0x11, 0, 0 }),
            (20127, new byte[] { 0xff }) })
        {
            var encoding = Selected(codePage);
            var source = encoding.GetBytes("valid\r\n").Concat(invalid).ToArray();
            using (var reader = Encoded(source, encoding))
            {
                Check(Read(reader, out var prefix) && prefix == "valid", "decode must not fail ahead of valid records");
                Throws<InvalidDataException>(() => ReadAll(reader));
                Throws<InvalidOperationException>(() => ReadAll(reader));
            }
            using (var reader = Encoded(source, Selected(codePage, false)))
            using (var reference = new StreamReader(new MemoryStream(source), Selected(codePage, false), false))
                Check(ReadAllAsync(reader).GetAwaiter().GetResult().SequenceEqual(new[] { reference.ReadLine()!, reference.ReadLine()! }), $"ACT replacement fallback {codePage}");
        }
        foreach (var codePage in new[] { 1200, 1201, 12000, 12001 })
            using (var reader = Encoded(Selected(codePage).GetBytes("valid\n").Concat(new byte[] { 0 }).ToArray(), Selected(codePage)))
            {
                Check(Read(reader, out var line) && line == "valid", "partial code-unit prefix");
                Throws<InvalidDataException>(() => ReadAllAsync(reader).GetAwaiter().GetResult());
            }
        var bom = Encoding.UTF8.GetPreamble().Concat(Encoding.UTF8.GetBytes("first\n")).ToArray();
        using (var reader = Encoded(bom, new UTF8Encoding(false,true), false)) Check(ReadAll(reader).Single() == "\uFEFFfirst", "disabled detection without selected preamble");
        using (var reader = Encoded(bom, Selected(65001), false)) Check(ReadAll(reader).Single() == "first", "selected preamble is consumed even with detection disabled");
        using (var reader = Encoded(Selected(932).GetBytes("日本語\n"), Selected(65001)))
            Throws<InvalidDataException>(() => ReadAll(reader));
        using (var reader = Encoded(Selected(1200).GetBytes("한글\n"), Selected(1200)))
        {
            Throws<OperationCanceledException>(() => ReadAllAsync(reader, new CancellationToken(true)).GetAwaiter().GetResult());
            Check(ReadAll(reader).Single() == "한글", "cancellation before input does not corrupt code-unit state");
        }
    }
    private static void WideRecordLimits()
    {
        foreach (var (codePage, width) in new[] { (1200, 2), (12000, 4) })
        {
            var encoding = Selected(codePage);
            var text = new string('x', 8 * 1024 * 1024 / width);
            using (var reader = Encoded(encoding.GetBytes(text + "\r\n"), encoding, chunk: 4096))
                Check(ReadAll(reader).Single() == text, $"source byte limit excludes wide delimiter {codePage}");
            using (var reader = Encoded(encoding.GetBytes(text + "x"), encoding, chunk: 4096))
                Throws<InvalidDataException>(() => ReadAll(reader));
        }
    }
}
