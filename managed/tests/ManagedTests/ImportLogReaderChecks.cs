using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Text;
using Advanced_Combat_Tracker;

internal static partial class ImportLogReaderChecks
{
    private static readonly Type ReaderType = typeof(ActGlobals).Assembly.GetType("Advanced_Combat_Tracker.ImportLogReader", true)!;

    private sealed class ShortReadStream(byte[] bytes, int maximumRead) : MemoryStream(bytes)
    {
        public override int Read(byte[] buffer, int offset, int count) => base.Read(buffer, offset, Math.Min(count, maximumRead));
        public override ValueTask<int> ReadAsync(Memory<byte> buffer, CancellationToken cancellationToken = default) =>
            base.ReadAsync(buffer[..Math.Min(buffer.Length, maximumRead)], cancellationToken);
    }

    private sealed class PrematureEndStream(byte[] bytes) : MemoryStream(bytes)
    {
        public override long Length => base.Length + 1;
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception("Import record reader check failed: " + message);
    }

    private static T Invoke<T>(Func<T> action)
    {
        try { return action(); }
        catch (TargetInvocationException error) when (error.InnerException is not null)
        {
            ExceptionDispatchInfo.Capture(error.InnerException).Throw();
            throw;
        }
    }

    private static IDisposable Create(object source)
    {
        var type = source is string ? typeof(string) : typeof(Stream);
        return Invoke(() => (IDisposable)ReaderType.GetConstructor(BindingFlags.Instance | BindingFlags.NonPublic,
            null, [type], null)!.Invoke([source]));
    }

    private static bool Read(IDisposable reader, out string line)
    {
        object?[] args = [null];
        var result = Invoke(() => (bool)ReaderType.GetMethod("TryRead", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(reader, args)!);
        line = (string)args[0]!;
        return result;
    }

    private static string[] ReadAll(IDisposable reader)
    {
        var result = new List<string>();
        while (Read(reader, out var line)) result.Add(line);
        return result.ToArray();
    }

    private static async Task<string[]> ReadAllAsync(IDisposable reader, CancellationToken cancellation = default)
    {
        var result = new List<string>();
        while (await Invoke(() => (ValueTask<string?>)ReaderType.GetMethod("ReadAsync", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(reader, [cancellation])!) is { } line)
            result.Add(line);
        return result.ToArray();
    }

    private static void Throws<T>(Action action) where T : Exception
    {
        try { action(); }
        catch (T) { return; }
        throw new Exception("Import record reader did not throw " + typeof(T).Name);
    }

    internal static void Run()
    {
        var text = "\uFEFFa\r\n\r\nb\rc\nlast";
        using (var reader = Create(new ShortReadStream(Encoding.UTF8.GetBytes(text), 1)))
            Check(ReadAll(reader).SequenceEqual(new[] { "a", "", "b", "c", "last" }), "mixed framing and CRLF across partial reads");
        using (var reader = Create(new ShortReadStream(Encoding.UTF8.GetBytes("\uFEFF한글\n\uFEFF끝"), 1)))
            Check(ReadAll(reader).SequenceEqual(new[] { "한글", "\uFEFF끝" }), "split UTF-8 and byte-zero-only BOM");
        using (var reader = Create(new MemoryStream(Array.Empty<byte>())))
            Check(ReadAll(reader).Length == 0, "empty file");
        using (var reader = Create(new MemoryStream(Encoding.UTF8.GetBytes("\n\r\n\r"))))
            Check(ReadAll(reader).SequenceEqual(new[] { "", "", "" }), "empty records without invented trailing record");
        using (var reader = Create(new ShortReadStream(new byte[] { 0xef, 0xbb, 0xbf }, 1)))
            Check(ReadAll(reader).Length == 0, "BOM-only file");
        var records = new[] { "first", "한글", "", new string('x', 4095), "🙂", "last" };
        var framed = Encoding.UTF8.GetBytes(string.Join('\n', records));
        foreach (var chunk in new[] { 1, 7, 4096 })
        {
            using (var reader = Create(new ShortReadStream(framed, chunk)))
                Check(ReadAll(reader).SequenceEqual(records), "buffered and direct decoding agree across record boundaries");
            using (var reader = Create(new ShortReadStream(framed, chunk)))
                Check(ReadAllAsync(reader).GetAwaiter().GetResult().SequenceEqual(records), "async framing agrees across short reads");
            using (var reader = Create(new ShortReadStream(Encoding.UTF8.GetBytes(text), chunk)))
                Check(ReadAllAsync(reader).GetAwaiter().GetResult().SequenceEqual(new[] { "a", "", "b", "c", "last" }), "async BOM and CRLF framing");
        }
        using (var reader = Create(new MemoryStream(framed)))
            Throws<OperationCanceledException>(() => ReadAllAsync(reader, new CancellationToken(true)).GetAwaiter().GetResult());
        foreach (var bytes in new[] { new byte[] { 0xc3, 0x28, 10 }, new byte[] { 0xe2, 0x82 } })
            using (var reader = Create(new ShortReadStream(bytes, 1)))
                Throws<InvalidDataException>(() => ReadAll(reader));
        using (var reader = Create(new PrematureEndStream(Encoding.UTF8.GetBytes("valid\n"))))
            Throws<EndOfStreamException>(() => ReadAll(reader));

        const int limit = 8 * 1024 * 1024;
        var exact = new byte[limit + 3];
        exact[0] = 0xef; exact[1] = 0xbb; exact[2] = 0xbf;
        Array.Fill(exact, (byte)'x', 3, limit);
        using (var reader = Create(new MemoryStream(exact)))
            Check(Read(reader, out var line) && line.Length == limit && !Read(reader, out _), "maximum record excludes the initial BOM");
        var oversized = new byte[limit + 1];
        Array.Fill(oversized, (byte)'x');
        using (var reader = Create(new MemoryStream(oversized)))
            Throws<InvalidDataException>(() => ReadAll(reader));

        var directory = Directory.CreateTempSubdirectory("aftercast-import-reader-");
        try
        {
            var path = Path.Combine(directory.FullName, "input.log");
            File.WriteAllText(path, "captured\n", new UTF8Encoding(false));
            using (var reader = Create(path))
            {
                File.AppendAllText(path, "later\n", new UTF8Encoding(false));
                Check(ReadAll(reader).SequenceEqual(new[] { "captured" }), "appends beyond initial length are ignored");
            }
            File.WriteAllText(path, "first\nsecond\n", new UTF8Encoding(false));
            using (var reader = Create(path))
            {
                Check(Read(reader, out var first) && first == "first", "first captured record");
                using (var writer = new FileStream(path, FileMode.Open, FileAccess.Write, FileShare.ReadWrite | FileShare.Delete)) writer.SetLength(0);
                Throws<EndOfStreamException>(() => ReadAll(reader));
            }
        }
        finally { directory.Delete(recursive: true); }
        EncodingCases();
        Console.WriteLine("Import strict UTF-8, selected encodings/BOM, partial reads, fixed-length boundaries and record limits passed.");
    }
}
