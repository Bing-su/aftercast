using System.Runtime.CompilerServices;
using System.Text;
using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

// Bind the real ACT property; exercise the StreamReader(stream, LogEncoding) contract inspected in ACT's import path.
public static class LogEncodingProbe
{
    public static string Run()
    {
        Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
        var root = (FormActMain)RuntimeHelpers.GetUninitializedObject(typeof(FormActMain));
        GC.SuppressFinalize(root);
        foreach (var (codePage, text) in new[] { (65001,"한글日本語🙂"), (1200,"한글日本語🙂"), (1201,"한글日本語🙂"),
            (12000,"한글日本語🙂"), (12001,"한글日本語🙂"), (932,"日本語"), (949,"한글"), (1252,"café"), (20127,"ascii") })
        {
            var encoding = Encoding.GetEncoding(codePage);
            root.LogEncoding = encoding;
            if (!ReferenceEquals(root.LogEncoding, encoding)) throw new Exception("LogEncoding property binding changed");
            var bytes = encoding.GetBytes(text + "\r\nlast");
            using var reader = new StreamReader(new MemoryStream(bytes), root.LogEncoding);
            if (reader.ReadLine() != text || reader.ReadLine() != "last" || reader.ReadLine() is not null)
                throw new Exception("Selected encoding/CRLF contract changed");
            if (encoding.GetPreamble().Length == 0) continue;
            using var bom = new StreamReader(new MemoryStream(encoding.GetPreamble().Concat(bytes).ToArray()), Encoding.ASCII);
            if (bom.ReadLine() != text || bom.CurrentEncoding.CodePage != codePage)
                throw new Exception("StreamReader BOM override contract changed");
        }
        using (var reader = new StreamReader(new MemoryStream(new byte[] { 0xff, 10 }), Encoding.UTF8))
            if (reader.ReadLine() != "\uFFFD") throw new Exception("ACT UTF-8 replacement contract changed");
        using (var reader = new StreamReader(new MemoryStream(new byte[] { 0xef, 0xbb, 0xbf, 0xff, 10 }), new UTF8Encoding(false,true)))
            if (reader.ReadLine() != "\uFFFD") throw new Exception("BOM did not reset StreamReader fallback");
        var utf32 = Encoding.UTF32.GetPreamble().Concat(Encoding.UTF32.GetBytes("a")).ToArray();
        using (var reader = new StreamReader(new MemoryStream(utf32), Encoding.Unicode))
            if (reader.ReadToEnd() != "\0a\0" || reader.CurrentEncoding.CodePage != 1200)
                throw new Exception("Selected preamble must precede BOM override");
        using (var reader = new StreamReader(new MemoryStream(new byte[] { 0xef, 0xbb, 0xbf, 97 }), Encoding.UTF8, false))
            if (reader.ReadToEnd() != "a") throw new Exception("Selected preamble must be consumed with detection disabled");
        return "Log encodings";
    }
}
