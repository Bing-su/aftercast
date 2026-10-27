using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;

namespace Advanced_Combat_Tracker;

// Initial history has the default StreamReader policy, independent of the live plugin codec.
internal static class InitialZoneReader
{
    private const int MaximumRecordBytes = 8 * 1024 * 1024;

    internal static string? Find(long length, Func<byte[], int, long, bool> read, Regex regex,
        Func<bool> canceled, Action<Exception> report)
    {
        if (canceled() || length == 0) return null;
        var header = new byte[4];
        var headerLength = (int)Math.Min(length, header.Length);
        if (!read(header, headerLength, 0)) return null;
        // Longest BOM first: UTF-32 LE shares the UTF-16 LE prefix.
        var (codePage, preamble) = headerLength >= 4 && header.AsSpan(0, 4).SequenceEqual(new byte[] { 0xff, 0xfe, 0, 0 }) ? (12000, 4)
            : headerLength >= 4 && header.AsSpan(0, 4).SequenceEqual(new byte[] { 0, 0, 0xfe, 0xff }) ? (12001, 4)
            : headerLength >= 3 && header.AsSpan(0, 3).SequenceEqual(new byte[] { 0xef, 0xbb, 0xbf }) ? (65001, 3)
            : headerLength >= 2 && header[0] == 0xff && header[1] == 0xfe ? (1200, 2)
            : headerLength >= 2 && header[0] == 0xfe && header[1] == 0xff ? (1201, 2) : (65001, 0);
        var encoding = Encoding.GetEncoding(codePage);
        var width = LiveLogDecoder.Width(encoding);
        // An incomplete final code unit is part of the unclosed tail, never a completed historical record.
        var position = length - (length - preamble) % width;
        var bytes = new byte[4096];
        var reverseLine = new List<byte>();
        var skipTail = true;
        var oversized = false;
        string? MatchLine()
        {
            if (oversized || reverseLine.Count == 0 || canceled()) return null;
            reverseLine.Reverse();
            var text = encoding.GetString(CollectionsMarshal.AsSpan(reverseLine));
            // ACT excludes encounter boundary annotations that merely contain a zone pattern.
            if (text.Contains(" combat by ", StringComparison.Ordinal)) return null;
            var match = regex.Match(text);
            return match.Success && match.Groups["zone"].Success ? match.Groups["zone"].Value : null;
        }
        while (position > preamble)
        {
            if (canceled()) return null;
            var count = (int)Math.Min(position - preamble, bytes.Length);
            position -= count;
            if (!read(bytes, count, position)) return null;
            for (var index = count - width; index >= 0; index -= width)
            {
                var unit = bytes.AsSpan(index, width);
                if (LiveLogDecoder.IsControl(unit, encoding, (byte)'\r') || LiveLogDecoder.IsControl(unit, encoding, (byte)'\n'))
                {
                    if (!skipTail && MatchLine() is { } zone) return zone;
                    skipTail = false;
                    reverseLine.Clear();
                    oversized = false;
                }
                else if (!skipTail && !oversized)
                {
                    if (reverseLine.Count + width > MaximumRecordBytes)
                    {
                        oversized = true;
                        reverseLine.Clear();
                        report(new InvalidDataException("Zone snapshot record exceeds 8 MiB source bytes"));
                    }
                    else for (var offset = width - 1; offset >= 0; offset--) reverseLine.Add(unit[offset]);
                }
            }
        }
        return !skipTail && !canceled() ? MatchLine() : null;
    }
}
