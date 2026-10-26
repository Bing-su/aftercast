using System.Text;

namespace Advanced_Combat_Tracker;

// Read a bounded file prefix so a concurrently growing live log cannot make an import endless.
internal sealed class ImportLogReader : IDisposable
{
    private const int MaximumRecordLength = 8 * 1024 * 1024;
    private readonly Stream stream;
    private readonly long initialLength;
    private readonly byte[] buffer = new byte[4096];
    private readonly MemoryStream pending = new();
    private readonly UTF8Encoding encoding = new(false, true);
    private long bytesRead;
    private int offset, count;
    private bool prefixChecked, skipLf, disposed;
    internal long TotalBytes => initialLength;
    internal long BytesRead => bytesRead;

    internal ImportLogReader(string path) : this(new FileStream(path, FileMode.Open, FileAccess.Read,
        FileShare.ReadWrite | FileShare.Delete, 4096, FileOptions.Asynchronous | FileOptions.SequentialScan)) { }

    // The deterministic stream seam has the same ownership and byte-zero contract as a newly opened file.
    internal ImportLogReader(Stream stream)
    {
        ArgumentNullException.ThrowIfNull(stream);
        try
        {
            if (!stream.CanRead || !stream.CanSeek || stream.Position != 0)
                throw new ArgumentException("An import stream must be readable, seekable and positioned at byte zero.", nameof(stream));
            initialLength = stream.Length;
            this.stream = stream;
        }
        catch { stream.Dispose(); pending.Dispose(); throw; }
    }

    internal bool TryRead(out string line)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        CheckPrefix();
        while (true)
        {
            if (offset == count && !ReadChunk())
            {
                if (pending.Length == 0) { line = ""; return false; }
                line = DecodeRecord(pending.GetBuffer().AsSpan(0, (int)pending.Length));
                return true;
            }
            if (TryDecodeBuffered(out line)) return true;
        }
    }

    // Both entry points share framing, e.g. CRLF split across reads must have identical results.
    internal async ValueTask<string?> ReadAsync(CancellationToken cancellation)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        if (!prefixChecked)
        {
            var prefixLength = (int)Math.Min(3, initialLength);
            while (count < prefixLength)
            {
                ValidateLength();
                var read = await stream.ReadAsync(buffer.AsMemory(count, prefixLength - count), cancellation).ConfigureAwait(false);
                if (read == 0) throw new EndOfStreamException("The import file ended before its captured length.");
                count += read; bytesRead += read;
            }
            SkipBom();
        }
        while (true)
        {
            cancellation.ThrowIfCancellationRequested();
            if (offset == count)
            {
                ValidateLength();
                if (bytesRead == initialLength)
                    return pending.Length == 0 ? null : DecodeRecord(pending.GetBuffer().AsSpan(0, (int)pending.Length));
                count = await stream.ReadAsync(buffer.AsMemory(0, (int)Math.Min(buffer.Length, initialLength - bytesRead)), cancellation).ConfigureAwait(false);
                offset = 0;
                if (count == 0) throw new EndOfStreamException("The import file ended before its captured length.");
                bytesRead += count;
            }
            if (TryDecodeBuffered(out var line)) return line;
        }
    }

    private bool TryDecodeBuffered(out string line)
    {
        line = "";
        while (offset < count)
        {
            var remaining = buffer.AsSpan(offset, count - offset);
            if (skipLf)
            {
                skipLf = false;
                if (remaining[0] == (byte)'\n') { offset++; continue; }
            }
            var delimiter = remaining.IndexOfAny((byte)'\r', (byte)'\n');
            var fragment = delimiter < 0 ? remaining : remaining[..delimiter];
            if (pending.Length + fragment.Length > MaximumRecordLength)
                throw new InvalidDataException($"Import record exceeds {MaximumRecordLength} UTF-8 bytes.");
            offset += fragment.Length;
            if (delimiter < 0)
            {
                pending.Write(fragment);
                continue;
            }
            skipLf = buffer[offset++] == (byte)'\r';
            // Decode complete in-buffer records directly; only records spanning reads need a copy.
            if (pending.Length == 0) line = DecodeRecord(fragment);
            else
            {
                pending.Write(fragment);
                line = DecodeRecord(pending.GetBuffer().AsSpan(0, (int)pending.Length));
            }
            return true;
        }
        return false;
    }

    private void CheckPrefix()
    {
        if (prefixChecked) return;
        prefixChecked = true;
        var prefixLength = (int)Math.Min(3, initialLength);
        while (count < prefixLength)
        {
            ValidateLength();
            var read = stream.Read(buffer, count, prefixLength - count);
            if (read == 0) throw new EndOfStreamException("The import file ended before its captured length.");
            count += read;
            bytesRead += read;
        }
        SkipBom();
    }

    private void SkipBom()
    {
        prefixChecked = true;
        // A BOM is framing only at byte zero; later U+FEFF characters belong to the original record.
        if (count == 3 && buffer[0] == 0xef && buffer[1] == 0xbb && buffer[2] == 0xbf) offset = count;
    }

    private bool ReadChunk()
    {
        // Validate at chunk boundaries and EOF, avoiding a filesystem length query for every short record.
        ValidateLength();
        if (bytesRead == initialLength) return false;
        count = stream.Read(buffer, 0, (int)Math.Min(buffer.Length, initialLength - bytesRead));
        offset = 0;
        if (count == 0) throw new EndOfStreamException("The import file ended before its captured length.");
        bytesRead += count;
        return true;
    }

    private void ValidateLength()
    {
        if (stream.Length < initialLength) throw new EndOfStreamException("The import file was truncated during reading.");
    }

    private string DecodeRecord(ReadOnlySpan<byte> record)
    {
        try { return encoding.GetString(record); }
        catch (DecoderFallbackException error) { throw new InvalidDataException("The import record is not valid UTF-8.", error); }
        finally { pending.SetLength(0); }
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;
        try { stream.Dispose(); }
        finally { pending.Dispose(); }
    }
}
