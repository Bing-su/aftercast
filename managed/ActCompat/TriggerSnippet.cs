using System.Globalization;
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
            var element = SnippetXml.Read(snippet, "Trigger", "R", "C", "SD", "ST", "CR", "T", "TN", "Ta");
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
