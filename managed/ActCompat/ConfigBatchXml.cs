using System.Globalization;
using System.Text;
using System.Xml;

namespace Aftercast.ActCompat;

internal static class ConfigBatchXml
{
    private static XmlElement Document(string text)
    {
        ArgumentNullException.ThrowIfNull(text);
        if (Encoding.UTF8.GetByteCount(text) > 16384) throw new InvalidDataException("Config exceeds 16 KiB");
        using var input = new StringReader(text);
        using var reader = XmlReader.Create(input, new XmlReaderSettings { DtdProcessing = DtdProcessing.Prohibit, XmlResolver = null, MaxCharactersInDocument = 16384 });
        var doc = new XmlDocument { XmlResolver = null };
        doc.Load(reader);
        var root = doc.DocumentElement;
        if (root is null || root.Name != "Config" || root.NamespaceURI.Length != 0) throw new InvalidDataException("Expected ACT Config");
        return root;
    }
    internal static XmlElement[] Read(string text, string section, string row, params string[] allowedAttributes)
    {
        var root = Document(text);
        if (root.HasAttribute("Xml"))
        {
            if (root.Attributes.Count != 1 || root.ChildNodes.Count != 0) throw new InvalidDataException("Invalid Config wrapper");
            root = Document(root.GetAttribute("Xml"));
        }
        if (root.Attributes.Count != 0) throw new InvalidDataException("Unsupported Config attributes");
        var parts = root.ChildNodes.Cast<XmlNode>().ToArray();
        if (parts.Length != 1 || parts[0] is not XmlElement group || group.Name != section || group.NamespaceURI.Length != 0 || group.Attributes.Count != 0)
            throw new InvalidDataException("Config must contain only one " + section + " section; mixed or unsupported settings are rejected");
        var rows = group.ChildNodes.Cast<XmlNode>().ToArray();
        if (rows.Length is < 1 or > 32) throw new InvalidDataException("Config must contain 1–32 items");
        var allowed = new HashSet<string>(allowedAttributes, StringComparer.Ordinal);
        return rows.Select(node =>
        {
            if (node is not XmlElement element || element.Name != row || element.NamespaceURI.Length != 0 || element.ChildNodes.Count != 0 ||
                element.Attributes.Cast<XmlAttribute>().Any(attribute => attribute.NamespaceURI.Length != 0 || !allowed.Contains(attribute.Name)))
                throw new InvalidDataException("Unsupported Config item or attribute");
            return element;
        }).ToArray();
    }
    internal static string Required(XmlElement element, string attribute) => element.HasAttribute(attribute)
        ? element.GetAttribute(attribute) : throw new InvalidDataException("Missing " + attribute);
    internal static string Text(XmlElement element, string attribute, string fallback = "") => element.HasAttribute(attribute)
        ? element.GetAttribute(attribute) : fallback;
    internal static bool Bool(XmlElement element, string attribute, bool fallback)
    {
        var value = Text(element, attribute);
        return value.Length == 0 ? fallback : bool.TryParse(value, out var result) ? result : throw new InvalidDataException("Invalid " + attribute);
    }
    internal static int Int(XmlElement element, string attribute, int fallback)
    {
        var value = Text(element, attribute);
        return value.Length == 0 ? fallback : int.TryParse(value, NumberStyles.Integer, CultureInfo.CurrentCulture, out var result) ? result : throw new InvalidDataException("Invalid " + attribute);
    }
}
