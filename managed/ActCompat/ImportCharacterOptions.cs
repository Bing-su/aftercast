using System.Text;
using System.Xml;

namespace Aftercast.ActCompat;

// ACT's import point of view is its character identity, not a display-only actor filter.
public sealed record ImportCharacterOptions(string Name)
{
    public const int MaximumUtf8Bytes = 1024;
    public void Validate()
    {
        ArgumentNullException.ThrowIfNull(Name);
        if (Name.Length > MaximumUtf8Bytes || new UTF8Encoding(false, true).GetByteCount(Name) > MaximumUtf8Bytes)
            throw new ArgumentException("Import character exceeds 1024 UTF-8 bytes", nameof(Name));
        if (Name.Any(char.IsControl)) throw new ArgumentException("Import character must be a single control-free name", nameof(Name));
        XmlConvert.VerifyXmlChars(Name);
    }
    internal string Resolve(string? current)
    {
        Validate();
        // ImportFile/ImportClip use the current name for empty selection; SetCharName does not trim.
        return Normalize(Name.Length == 0 ? current : Name);
    }
    internal static string Normalize(string? name) => name is { Length: > 2 } ? name : "YOU";
}
