using System.Globalization;
using System.Text;
using System.Xml;
using Aftercast.ActCompat;

namespace Aftercast.ActCompat
{
    public sealed record TriggerSnippetPreview(ulong Revision, string Key, bool Exists, TriggerDefinition Definition);
}

namespace Advanced_Combat_Tracker
{
    public partial class FormActMain
    {
        // ACT shares one Trigger element; this preview never changes definitions or runs effects.
        public TriggerSnippetPreview PreviewTriggerSnippet(string snippet)
        {
            ArgumentNullException.ThrowIfNull(snippet);
            if (Encoding.UTF8.GetByteCount(snippet) > 16384) throw new InvalidDataException("Trigger snippet exceeds 16 KiB");
            using var input = new StringReader(snippet);
            using var reader = XmlReader.Create(input, new XmlReaderSettings { DtdProcessing = DtdProcessing.Prohibit, XmlResolver = null, MaxCharactersInDocument = 16384 });
            var document = new XmlDocument { XmlResolver = null };
            document.Load(reader);
            var element = document.DocumentElement;
            if (element is null || element.Name != "Trigger" || element.NamespaceURI.Length != 0)
                throw new InvalidDataException("Only a single ACT Trigger snippet is supported");
            if (element.ChildNodes.Cast<XmlNode>().Any(node => node.NodeType != XmlNodeType.Whitespace))
                throw new InvalidDataException("Trigger snippet cannot contain child nodes");
            var allowed = new HashSet<string>(StringComparer.Ordinal) { "R", "C", "SD", "ST", "CR", "T", "TN", "Ta" };
            if (element.Attributes.Cast<XmlAttribute>().Any(attribute => attribute.NamespaceURI.Length != 0 || !allowed.Contains(attribute.Name)))
                throw new InvalidDataException("Unsupported trigger snippet attribute");
            if (!element.HasAttribute("R") || !element.HasAttribute("C")) throw new InvalidDataException("Trigger snippet requires R and C attributes");
            var soundType = int.TryParse(element.GetAttribute("ST"), NumberStyles.Integer, CultureInfo.CurrentCulture, out var parsed) ? parsed : 0;
            var definition = new TriggerDefinition(element.GetAttribute("C"), element.GetAttribute("R"), soundType,
                element.GetAttribute("SD"), true, element.GetAttribute("CR") == "T", element.GetAttribute("T") == "T",
                element.GetAttribute("TN"), element.GetAttribute("Ta") == "T");
            CustomTrigger trigger;
            try { trigger = CompileDefinition(definition); }
            catch (System.Text.RegularExpressions.RegexParseException)
            { throw new InvalidDataException("Trigger snippet contains an invalid regular expression"); }
            lock (triggerGate)
            {
                _ = TriggerSnapshot();
                return new(triggerRevision, trigger.Key, CustomTriggers.ContainsKey(trigger.Key), Definition(trigger));
            }
        }
    }
}
