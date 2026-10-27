using System.Text;

namespace Advanced_Combat_Tracker;

// File and Unicode text inputs share scheduling/dispatch, not byte decoding semantics.
internal interface IImportRecordReader : IDisposable
{
    long TotalBytes { get; }
    long BytesRead { get; }
    ValueTask<string?> ReadAsync(CancellationToken cancellation);
}

// Clipboard is already Unicode: preserve BOM characters and StringReader's CR/LF/CRLF framing.
internal sealed class ImportTextReader(string text) : IImportRecordReader
{
    internal const int MaximumUtf8Bytes = 8 * 1024 * 1024;
    private static readonly Encoding Utf8 = new UTF8Encoding(false, true);
    private readonly StringReader reader = new(text);
    private int position, emptyLines;
    private bool ended;
    public long TotalBytes { get; } = Validate(text);
    public long BytesRead { get; private set; }
    internal static int Validate(string text)
    {
        ArgumentNullException.ThrowIfNull(text);
        if (text.Length == 0 || text.Length > MaximumUtf8Bytes) throw new ArgumentException("Clipboard import requires nonempty text up to 8 MiB UTF-8", nameof(text));
        var count = Utf8.GetByteCount(text);
        if (count > MaximumUtf8Bytes) throw new ArgumentException("Clipboard import exceeds 8 MiB UTF-8", nameof(text));
        return count;
    }
    public ValueTask<string?> ReadAsync(CancellationToken cancellation)
    {
        cancellation.ThrowIfCancellationRequested();
        if (ended) return ValueTask.FromResult<string?>(null);
        var line = reader.ReadLine();
        if (line is null) { ended = true; return ValueTask.FromResult<string?>(null); }
        var before = position;
        position += line.Length;
        if (position < text.Length && text[position] == '\r') position++;
        if (position < text.Length && text[position] == '\n') position++;
        // Progress uses exact UTF-8 equivalent bytes, not ACT's assumed line.Length + 2 count.
        BytesRead += Utf8.GetByteCount(text.AsSpan(before, position - before));
        emptyLines = line.Length == 0 ? emptyLines + 1 : 0;
        if (emptyLines > 1000) ended = true; // ACT ImportClip terminates after this consecutive-empty threshold.
        return ValueTask.FromResult<string?>(line);
    }
    public void Dispose() => reader.Dispose();
}
