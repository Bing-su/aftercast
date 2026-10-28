using System.Xml;
using System.Security.Cryptography;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    private readonly SortedList<string, RenameFix> renameList = new();
    private readonly SortedList<string, RedirectFix> redirectList = new();
    private string? correctionDiskHash;
    private bool correctionFileEditable = true;

    internal sealed class RenameFix(string before, string after) : IEquatable<RenameFix>
    {
        public string Before { get; set; } = before;
        public string After { get; set; } = after;
        public bool Active { get; set; } = true;
        public bool IsMatch(CombatActionEventArgs action) => action.attacker == Before || action.victim == Before;
        public bool Fix(CombatActionEventArgs action)
        {
            if (!Active) return false;
            var changed = false;
            if (action.attacker == Before) { action.attacker = After; changed = true; }
            if (action.victim == Before) { action.victim = After; changed = true; }
            return changed;
        }
        public bool Equals(RenameFix? other) => ToString().Equals(other!.ToString());
        public override string ToString() => Before + " -> " + After;
    }

    internal sealed class RedirectFix(string ability, string destination, int type) : IEquatable<RedirectFix>
    {
        public string Ability { get; set; } = ability;
        public string DestinationCombatant { get; set; } = destination;
        public bool Active { get; set; } = true;
        public int Type { get; set; } = type;
        public bool IsMatch(CombatActionEventArgs action) => action.theAttackType == Ability && Type switch
        {
            1 => CombatantData.DamageSwingTypes.Contains(action.swingType),
            2 => CombatantData.HealingSwingTypes.Contains(action.swingType),
            3 => !CombatantData.DamageSwingTypes.Contains(action.swingType) && !CombatantData.HealingSwingTypes.Contains(action.swingType),
            _ => true
        };
        public bool Fix(CombatActionEventArgs action)
        {
            if (!IsMatch(action)) return false;
            action.attacker = DestinationCombatant;
            return true;
        }
        public bool Equals(RedirectFix? other) => ToString().Equals(other!.ToString());
        public override string ToString() => $"* ({Ability}) -> {DestinationCombatant} ({Ability})";
    }

    private void RenameCombatant(CombatActionEventArgs action)
    {
        foreach (var rule in renameList.Values) if (rule.IsMatch(action)) rule.Fix(action);
    }
    private void RedirectAbility(CombatActionEventArgs action)
    {
        foreach (var rule in redirectList.Values) if (rule.Active) rule.Fix(action);
    }

    private static byte[]? ReadCorrectionBytes(string path)
    {
        try
        {
            using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read | FileShare.Delete);
            if (stream.Length > 4 * 1024 * 1024) throw new InvalidDataException("Corrections exceed 4 MiB");
            var bytes = new byte[checked((int)stream.Length)]; stream.ReadExactly(bytes);
            if (stream.ReadByte() != -1) throw new IOException("Correction file changed while reading");
            return bytes;
        }
        catch (FileNotFoundException) { return null; }
        catch (DirectoryNotFoundException) { return null; }
    }
    private static string? CorrectionHash(byte[]? bytes) => bytes is null ? null : Convert.ToHexString(SHA256.HashData(bytes));
    internal void LoadDataCorrections(string path)
    {
        var bytes = ReadCorrectionBytes(path);
        if (bytes is null) { correctionDiskHash = null; return; }
        var (renames, redirects, editable) = ParseCorrections(bytes);
        lock (CombatGate)
        {
            renameList.Clear(); redirectList.Clear();
            foreach (var (key, rule) in renames) renameList.Add(key, rule);
            foreach (var (key, rule) in redirects) redirectList.Add(key, rule);
            correctionDiskHash = CorrectionHash(bytes); correctionFileEditable = editable;
        }
    }
    private static (SortedList<string, RenameFix>, SortedList<string, RedirectFix>, bool) ParseCorrections(byte[] bytes)
    {
        using var stream = new MemoryStream(bytes, false);
        using var reader = XmlReader.Create(stream, new XmlReaderSettings
        { DtdProcessing = DtdProcessing.Prohibit, XmlResolver = null, MaxCharactersInDocument = 4 * 1024 * 1024 });
        var document = new XmlDocument { XmlResolver = null }; document.Load(reader);
        if (document.DocumentElement?.Name != "DataCorrections") throw new XmlException("Expected DataCorrections root");
        var renames = new SortedList<string, RenameFix>(); var redirects = new SortedList<string, RedirectFix>();
        static string Required(XmlElement element, string name) => element.HasAttribute(name)
            ? element.GetAttribute(name) : throw new XmlException("Missing correction attribute: " + name);
        foreach (XmlElement element in document.SelectNodes("/DataCorrections/RenameFix/Rename")!)
        {
            var rule = new RenameFix(Required(element, "Before"), Required(element, "After"))
            { Active = !element.HasAttribute("Active") || bool.Parse(element.GetAttribute("Active")) };
            renames.TryAdd(rule.ToString(), rule);
        }
        foreach (XmlElement element in document.SelectNodes("/DataCorrections/AbilityRedirectFix/Redirect")!)
        {
            var rule = new RedirectFix(Required(element, "Ability"), Required(element, "Into"), int.Parse(Required(element, "Type")))
            { Active = !element.HasAttribute("Active") || bool.Parse(element.GetAttribute("Active")) };
            if (redirects.TryGetValue(rule.ToString(), out var previous)) previous.Type = rule.Type; else redirects.Add(rule.ToString(), rule);
        }
        // Refuse editing documents with unknown content instead of silently deleting it.
        static bool AllowedAttributes(XmlElement item, params string[] allowed) =>
            item.Attributes.Cast<XmlAttribute>().All(attribute => allowed.Contains(attribute.Name, StringComparer.Ordinal));
        static bool OnlyElements(XmlNode node, string childName) => node.ChildNodes.Cast<XmlNode>().All(child =>
            child is XmlWhitespace or XmlSignificantWhitespace || child is XmlElement item && item.Name == childName &&
            item.ChildNodes.Count == 0 && (childName == "Rename" ? AllowedAttributes(item, "Before", "After", "Active") : AllowedAttributes(item, "Ability", "Into", "Type", "Active")));
        var root = document.DocumentElement;
        var sections = root.ChildNodes.OfType<XmlElement>().ToArray();
        var editable = document.ChildNodes.Cast<XmlNode>().All(node => node is XmlDeclaration or XmlWhitespace or XmlSignificantWhitespace || ReferenceEquals(node, root)) &&
            AllowedAttributes(root) && root.ChildNodes.Cast<XmlNode>().All(node => node is XmlWhitespace or XmlSignificantWhitespace || node is XmlElement) &&
            sections.Select(section => section.Name).Distinct(StringComparer.Ordinal).Count() == sections.Length &&
            sections.All(section => AllowedAttributes(section) && (section.Name == "RenameFix" ? OnlyElements(section, "Rename") : section.Name == "AbilityRedirectFix" && OnlyElements(section, "Redirect"))) &&
            document.SelectNodes("/DataCorrections/RenameFix/Rename")!.Count == renames.Count &&
            document.SelectNodes("/DataCorrections/AbilityRedirectFix/Redirect")!.Count == redirects.Count;
        return (renames, redirects, editable);
    }
}
