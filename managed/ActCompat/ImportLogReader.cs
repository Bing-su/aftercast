using System.Text;
using Aftercast.ActCompat;

namespace Advanced_Combat_Tracker;

// Read a bounded file prefix so a concurrently growing live log cannot make an import endless.
internal sealed class ImportLogReader : IDisposable
{
    private const int MaximumRecordLength = 8 * 1024 * 1024;
    private readonly Stream stream;
    private readonly long initialLength;
    private readonly byte[] buffer = new byte[4096];
    private readonly MemoryStream pending = new();
    private Encoding encoding;
    private readonly bool detectBom;
    private int unitWidth = 1, unitCount;
    private readonly byte[] unit = new byte[4];
    private bool bigEndian, decodingFailed;
    private long bytesRead;
    private int offset, count;
    private bool prefixChecked, skipLf, disposed;
    internal bool RequireCompleteRecords { get; init; }
    internal bool DiscardedIncompleteTail { get; private set; }
    internal long TotalBytes => initialLength;
    internal long BytesRead => bytesRead;

    internal ImportLogReader(string path) : this(path, new ImportTextOptions().Resolve(), true) { }
    internal ImportLogReader(string path, Encoding encoding, bool detectBom = true) : this(new FileStream(path, FileMode.Open, FileAccess.Read,
        FileShare.ReadWrite | FileShare.Delete, 4096, FileOptions.Asynchronous | FileOptions.SequentialScan), encoding, detectBom)
    { }

    // The deterministic stream seam has the same ownership and byte-zero contract as a newly opened file.
    internal ImportLogReader(Stream stream) : this(stream, new ImportTextOptions().Resolve(), true) { }
    internal ImportLogReader(Stream stream, Encoding encoding, bool detectBom = true)
    {
        ArgumentNullException.ThrowIfNull(stream);
        try
        {
            if (!stream.CanRead || !stream.CanSeek || stream.Position != 0)
                throw new ArgumentException("An import stream must be readable, seekable and positioned at byte zero.", nameof(stream));
            this.encoding = ImportTextOptions.Validate(encoding);
            this.detectBom = detectBom;
            initialLength = stream.Length;
            this.stream = stream;
        }
        catch { stream.Dispose(); pending.Dispose(); throw; }
    }

    internal bool TryRead(out string line)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        if (decodingFailed) throw new InvalidOperationException("An import reader cannot resume after a decoding failure.");
        CheckPrefix();
        while (true)
        {
            if (offset == count && !ReadChunk())
            {
                if (pending.Length == 0) { line = ""; return false; }
                if (RequireCompleteRecords) { DiscardedIncompleteTail = true; pending.SetLength(0); line = ""; return false; }
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
        if (decodingFailed) throw new InvalidOperationException("An import reader cannot resume after a decoding failure.");
        if (!prefixChecked)
        {
            var prefixLength = (int)Math.Min(4, initialLength);
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
                if (bytesRead == initialLength && RequireCompleteRecords && pending.Length != 0)
                {
                    DiscardedIncompleteTail = true;
                    pending.SetLength(0);
                    return null;
                }
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
        if (unitWidth != 1) return TryDecodeWide(out line);
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
                throw new InvalidDataException($"Import record exceeds {MaximumRecordLength} source bytes.");
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

    private bool TryDecodeWide(out string line)
    {
        line = "";
        while (offset < count)
        {
            var value = buffer[offset++];
            unit[unitCount++] = value;
            pending.WriteByte(value);
            if (unitCount != unitWidth)
            {
                if (pending.Length > MaximumRecordLength + unitWidth - 1) throw new InvalidDataException($"Import record exceeds {MaximumRecordLength} source bytes.");
                continue;
            }
            unitCount = 0;
            var character = bigEndian ? unit[unitWidth - 1] : unit[0];
            var zero = true;
            for (var index = 0; index < unitWidth; index++)
                if (index != (bigEndian ? unitWidth - 1 : 0) && unit[index] != 0) zero = false;
            if (skipLf)
            {
                skipLf = false;
                if (zero && character == (byte)'\n') { pending.SetLength(0); continue; }
            }
            if (zero && character is (byte)'\r' or (byte)'\n')
            {
                skipLf = character == (byte)'\r';
                pending.SetLength(pending.Length - unitWidth);
                line = DecodeRecord(pending.GetBuffer().AsSpan(0, (int)pending.Length));
                return true;
            }
            if (pending.Length > MaximumRecordLength) throw new InvalidDataException($"Import record exceeds {MaximumRecordLength} source bytes.");
        }
        return false;
    }

    private void CheckPrefix()
    {
        if (prefixChecked) return;
        prefixChecked = true;
        var prefixLength = (int)Math.Min(4, initialLength);
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
        // Only a recognized byte-zero BOM selects an encoding; an interior U+FEFF remains record data.
        // StreamReader consumes the selected encoding's own preamble before optional BOM detection.
        var preamble = encoding.Preamble;
        var ownPreamble = preamble.Length != 0 && count >= preamble.Length && buffer.AsSpan(0, preamble.Length).SequenceEqual(preamble);
        if (ownPreamble) offset = preamble.Length;
        var codePage = !ownPreamble && detectBom ? count switch
        {
            >= 4 when buffer[0] == 0 && buffer[1] == 0 && buffer[2] == 0xfe && buffer[3] == 0xff => 12001,
            >= 4 when buffer[0] == 0xff && buffer[1] == 0xfe && buffer[2] == 0 && buffer[3] == 0 => 12000,
            >= 3 when buffer[0] == 0xef && buffer[1] == 0xbb && buffer[2] == 0xbf => 65001,
            >= 2 when buffer[0] == 0xff && buffer[1] == 0xfe => 1200,
            >= 2 when buffer[0] == 0xfe && buffer[1] == 0xff => 1201,
            _ => 0,
        } : 0;
        if (codePage != 0)
        {
            offset = codePage == 65001 ? 3 : codePage is 1200 or 1201 ? 2 : 4;
            encoding = ImportTextOptions.BomEncoding(codePage, encoding);
        }
        unitWidth = encoding.CodePage is 1200 or 1201 ? 2 : encoding.CodePage is 12000 or 12001 ? 4 : 1;
        bigEndian = encoding.CodePage is 1201 or 12001;
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
        if (record.Length > MaximumRecordLength) throw new InvalidDataException($"Import record exceeds {MaximumRecordLength} source bytes.");
        try { return encoding.GetString(record); }
        catch (DecoderFallbackException error)
        {
            decodingFailed = true;
            throw new InvalidDataException($"The import record is not valid {encoding.WebName} (code page {encoding.CodePage}).", error);
        }
        finally
        {
            pending.SetLength(0);
            // Multi-source import must not retain one maximum-size allocation for every admitted file.
            if (pending.Capacity > 128 * 1024) pending.Capacity = 0;
        }
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;
        try { stream.Dispose(); }
        finally { pending.Dispose(); }
    }
}
