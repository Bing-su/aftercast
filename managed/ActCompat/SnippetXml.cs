using System.Text;
using System.Xml;

namespace Aftercast.ActCompat;

internal static class SnippetXml
{
    internal static XmlElement Read(string snippet, string name, params string[] attributes)
    {
        ArgumentNullException.ThrowIfNull(snippet);
        if (Encoding.UTF8.GetByteCount(snippet) > 16384) throw new InvalidDataException("ACT snippet exceeds 16 KiB");
        using var input = new StringReader(snippet);
        using var reader = XmlReader.Create(input, new XmlReaderSettings { DtdProcessing = DtdProcessing.Prohibit, XmlResolver = null, MaxCharactersInDocument = 16384 });
        var document = new XmlDocument { XmlResolver = null };
        document.Load(reader);
        var element = document.DocumentElement;
        if (element is null || element.Name != name || element.NamespaceURI.Length != 0)
            throw new InvalidDataException("Only a single ACT " + name + " snippet is supported");
        if (element.ChildNodes.Cast<XmlNode>().Any(node => node.NodeType != XmlNodeType.Whitespace))
            throw new InvalidDataException(name + " snippet cannot contain child nodes");
        var allowed = new HashSet<string>(attributes, StringComparer.Ordinal);
        if (element.Attributes.Cast<XmlAttribute>().Any(attribute => attribute.NamespaceURI.Length != 0 || !allowed.Contains(attribute.Name)))
            throw new InvalidDataException("Unsupported " + name + " snippet attribute");
        return element;
    }
}
