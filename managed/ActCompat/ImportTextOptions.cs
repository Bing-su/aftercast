using System.Text;

namespace Aftercast.ActCompat;

// A finite stateless set keeps CR/LF framing byte-exact, including DBCS trail-byte and UTF code-unit boundaries.
public sealed record ImportTextOptions(int CodePage = 65001, bool ReplaceInvalid = false, bool DetectBom = true)
{
    static ImportTextOptions() => Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
    internal static Encoding Validate(Encoding encoding)
    {
        ArgumentNullException.ThrowIfNull(encoding);
        ValidateCodePage(encoding.CodePage);
        if (encoding.Preamble.Length > 4) throw new ArgumentException("Unsupported import encoding preamble");
        var fallback = encoding.DecoderFallback;
        if (fallback.MaxCharCount > 2 ||
            fallback is DecoderReplacementFallback replacement && replacement.DefaultString.IndexOfAny(['\r','\n']) >= 0 ||
            fallback is not (DecoderExceptionFallback or DecoderReplacementFallback) &&
                fallback.GetType() != Encoding.GetEncoding(encoding.CodePage).DecoderFallback.GetType())
            throw new ArgumentException("Unsupported import decoder fallback: use strict or the encoding's standard replacement");
        return (Encoding)encoding.Clone();
    }
    private static void ValidateCodePage(int codePage)
    {
        if (codePage is not (65001 or 1200 or 1201 or 12000 or 12001 or 932 or 949 or 1252 or 20127))
            throw new ArgumentException($"Unsupported import code page: {codePage}");
    }
    internal Encoding Resolve()
    {
        var encoding = DefaultEncoding(CodePage);
        if (!ReplaceInvalid) encoding.DecoderFallback = DecoderFallback.ExceptionFallback;
        return Validate(encoding);
    }
    private static Encoding DefaultEncoding(int codePage)
    {
        // The provider is part of the .NET shared framework; no locale inference or external conversion is used.
        ValidateCodePage(codePage);
        return (Encoding)Encoding.GetEncoding(codePage).Clone();
    }
    internal static Encoding BomEncoding(int codePage, Encoding selected)
    {
        var encoding = DefaultEncoding(codePage);
        // ACT's BOM detection resets to the detected encoding's fallback. Preserve explicit strictness instead.
        if (selected.DecoderFallback is DecoderExceptionFallback) encoding.DecoderFallback = DecoderFallback.ExceptionFallback;
        return encoding;
    }
}
