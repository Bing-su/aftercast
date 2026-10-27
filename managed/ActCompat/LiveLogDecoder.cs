using System.Runtime.InteropServices;
using System.Text;
using Aftercast.ActCompat;

namespace Advanced_Combat_Tracker;

// A reader/file generation owns framing state; incomplete appended characters are never flushed at temporary EOF.
internal sealed class LiveLogDecoder : IDisposable
{
    private const int MaximumRecordBytes = 8 * 1024 * 1024;
    private static readonly (byte[] Bytes, int CodePage)[] Boms = [
        ([0,0,0xfe,0xff],12001), ([0xff,0xfe,0,0],12000), ([0xef,0xbb,0xbf],65001),
        ([0xff,0xfe],1200), ([0xfe,0xff],1201),
    ];
    private readonly Encoding configured;
    private Encoding encoding;
    private readonly MemoryStream pending = new();
    private readonly List<byte> prefix = new(4);
    private readonly byte[] unit = new byte[4];
    private int width, unitCount, alignmentSkip;
    private bool bigEndian, checkingPrefix, discarding, skipLf;
    internal bool HasPending => pending.Length != 0 || prefix.Count != 0 || unitCount != 0;
    internal static int Width(Encoding encoding) => encoding.CodePage is 1200 or 1201 ? 2 : encoding.CodePage is 12000 or 12001 ? 4 : 1;
    internal static bool IsControl(ReadOnlySpan<byte> unit, Encoding encoding, byte character)
    {
        var characterIndex = encoding.CodePage is 1201 or 12001 ? unit.Length - 1 : 0;
        for (var index = 0; index < unit.Length; index++)
            if (unit[index] != (index == characterIndex ? character : 0)) return false;
        return true;
    }
    internal LiveLogDecoder(Encoding encoding, long position, bool discard, bool previousCr)
    {
        configured = ImportTextOptions.Validate(encoding);
        this.encoding = configured;
        Reset(position, discard, previousCr);
    }
    internal void Reset(long position = 0, bool discard = false, bool previousCr = false)
    {
        pending.SetLength(0); prefix.Clear(); unitCount = 0;
        encoding = configured;
        SetWidth();
        alignmentSkip = (int)((width - position % width) % width);
        checkingPrefix = position == 0;
        discarding = discard;
        skipLf = previousCr;
    }
    private void SetWidth()
    {
        width = Width(encoding);
        bigEndian = encoding.CodePage is 1201 or 12001;
    }
    internal bool Feed(ReadOnlySpan<byte> bytes, Func<string, bool> deliver, Action<Exception> report)
    {
        foreach (var value in bytes)
        {
            if (checkingPrefix)
            {
                prefix.Add(value);
                if (!PrefixReady()) continue;
                if (!Begin(deliver, report)) return false;
            }
            else if (!Frame(value, deliver, report)) return false;
        }
        return true;
    }
    private bool PrefixReady()
    {
        var bytes = CollectionsMarshal.AsSpan(prefix);
        var own = encoding.Preamble;
        if (!own.IsEmpty && bytes.Length >= own.Length && bytes[..own.Length].SequenceEqual(own)) return true;
        if (bytes.Length < own.Length && own[..bytes.Length].SequenceEqual(bytes)) return false;
        foreach (var (bom, _) in Boms)
            if (bytes.Length < bom.Length && bom.AsSpan(0, bytes.Length).SequenceEqual(bytes)) return false;
        return true;
    }
    private bool Begin(Func<string, bool> deliver, Action<Exception> report)
    {
        Span<byte> bytes = stackalloc byte[4];
        prefix.CopyTo(bytes); var count = prefix.Count; prefix.Clear();
        var own = encoding.Preamble;
        var skip = !own.IsEmpty && count >= own.Length && bytes[..own.Length].SequenceEqual(own) ? own.Length : 0;
        if (skip == 0)
            foreach (var (bom, codePage) in Boms)
                if (count >= bom.Length && bytes[..bom.Length].SequenceEqual(bom))
                { encoding = ImportTextOptions.BomEncoding(codePage, configured); skip = bom.Length; break; }
        SetWidth(); checkingPrefix = false;
        for (var index = skip; index < count; index++) if (!Frame(bytes[index], deliver, report)) return false;
        return true;
    }
    private bool Frame(byte value, Func<string, bool> deliver, Action<Exception> report)
    {
        if (alignmentSkip > 0) { alignmentSkip--; return true; }
        unit[unitCount++] = value;
        if (unitCount < width) return true;
        unitCount = 0;
        var character = bigEndian ? unit[width - 1] : unit[0];
        var delimiter = (character is (byte)'\r' or (byte)'\n') && IsControl(unit.AsSpan(0, width), encoding, character);
        if (skipLf)
        {
            skipLf = false;
            if (delimiter && character == (byte)'\n') return true;
        }
        if (delimiter)
        {
            skipLf = character == (byte)'\r';
            if (discarding) { discarding = false; return true; }
            string line;
            try { line = encoding.GetString(pending.GetBuffer().AsSpan(0, (int)pending.Length)); }
            catch (DecoderFallbackException error) { pending.SetLength(0); report(error); return true; }
            pending.SetLength(0);
            return deliver(line);
        }
        if (discarding) return true;
        if (pending.Length + width > MaximumRecordBytes)
        {
            pending.SetLength(0); discarding = true;
            report(new InvalidDataException($"Pending log line exceeds {MaximumRecordBytes} source bytes"));
        }
        else pending.Write(unit, 0, width);
        return true;
    }
    public void Dispose() => pending.Dispose();
}
