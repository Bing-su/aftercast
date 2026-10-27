using System.Globalization;
using System.Buffers.Binary;
using System.IO.Compression;
using System.Runtime.Serialization;
using System.Text;
using System.Text.RegularExpressions;
using System.Xml;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    internal const int ArchiveLimit = 64 * 1024 * 1024;
    private static DataContractSerializer SwingTags() => new(typeof(Dictionary<string, object>), new DataContractSerializerSettings { MaxItemsInObjectGraph = 100000 });
    public void EncountersToXml(List<EncounterData> encounters, Stream stream, bool includeLogs) => WriteEncounterXml(encounters, stream, includeLogs, false);
    private void WriteEncounterXml(List<EncounterData> encounters, Stream stream, bool includeLogs, bool history)
    {
        var tags = SwingTags();
        var format = history ? "O" : "s";
        using var writer = XmlWriter.Create(stream, new XmlWriterSettings { Encoding = new UTF8Encoding(false), Indent = true, CloseOutput = false });
        writer.WriteStartDocument(); writer.WriteStartElement("Encounters");
        if (history) writer.WriteAttributeString("AftercastHistoryVersion", "1");
        foreach (var encounter in encounters)
        {
            writer.WriteStartElement("Encounter");
            void Attribute(string name, object value) => writer.WriteAttributeString(name, Convert.ToString(value, CultureInfo.InvariantCulture));
            Attribute("Name", encounter.Title); Attribute("CharName", encounter.CharName);
            if (history)
            {
                Attribute("AftercastDroppedLogLines", encounter.DroppedLogLines);
                Attribute("AftercastLogIdentityKnown", encounter.LogSourceIdentityKnown);
                Attribute("AftercastClipboardImport", encounter.ClipboardImport);
            }
            if (encounter.ZoneName != "Import" && encounter.Parent is { } zone)
            {
                var zoneName = zone.ZoneName; var zoneStart = zone.StartTime;
                if (archivedEncounters is not null && archivedEncounters.TryGetValue(encounter, out var identity))
                { zoneName = encounter.ZoneName; zoneStart = identity.ZoneStart; }
                Attribute("ZoneName", zoneName); Attribute("ZoneDateTime", zoneStart.ToString(format, CultureInfo.InvariantCulture));
            }
            foreach (var time in encounter.StartTimes) writer.WriteElementString("StartTime", time.ToString(format, CultureInfo.InvariantCulture));
            foreach (var time in encounter.EndTimes) writer.WriteElementString("EndTime", time.ToString(format, CultureInfo.InvariantCulture));
            // Export each original outgoing swing once, e.g. overlapping categories must not multiply damage on reimport.
            foreach (var actor in encounter.Items.Values)
                if (actor.AllOut.TryGetValue("All", out var all))
                    foreach (var swing in all.Items)
                    {
                        writer.WriteStartElement("Swing");
                        Attribute("SwingType", swing.SwingType); Attribute("Critical", swing.Critical);
                        Attribute("Time", swing.Time.ToString(format, CultureInfo.InvariantCulture)); Attribute("TimeSorter", swing.TimeSorter);
                        Attribute("Combatant", swing.Attacker); Attribute("Victim", swing.Victim); Attribute("AttackType", swing.AttackType);
                        Attribute("DamageType", swing.DamageType); Attribute("Damage", swing.Damage.ToString()); Attribute("Special", swing.Special);
                        if (history)
                        {
                            // ACT's display-only Damage attribute loses labelled numeric values; the private DB retains them explicitly.
                            Attribute("AftercastNumber", swing.Damage.Number);
                            if (swing.Damage.RawDamageString is { } label) Attribute("AftercastLabel", label);
                            if (swing.Damage.DamageString2 is { } suffix) Attribute("AftercastSuffix", suffix);
                        }
                        if (swing.Tags.Count > 0) tags.WriteObject(writer, swing.Tags);
                        writer.WriteEndElement();
                    }
            if (includeLogs)
                foreach (var line in encounter.LogLines)
                {
                    writer.WriteStartElement("LogLine");
                    if (history) Attribute("AftercastTimeSorter", line.GlobalTimeSorter);
                    Attribute("Type", line.Type); Attribute("Time", line.Time.ToString(format, CultureInfo.InvariantCulture)); Attribute("Line", line.LogLine);
                    writer.WriteEndElement();
                }
            foreach (var ally in encounter.GetAllies()) writer.WriteElementString("Ally", ally.Name);
            writer.WriteEndElement();
        }
        writer.WriteEndElement(); writer.WriteEndDocument();
    }

    public List<EncounterData> XmlToEncounters(Stream stream) => ReadEncounterXml(stream, false);

    private List<EncounterData> ReadEncounterXml(Stream stream, bool history)
    {
        using var reader = XmlReader.Create(stream, new XmlReaderSettings
        { DtdProcessing = DtdProcessing.Prohibit, XmlResolver = null, MaxCharactersInDocument = ArchiveLimit, CloseInput = true });
        var document = new XmlDocument { XmlResolver = null };
        document.Load(reader);
        if (document.DocumentElement?.Name != "Encounters") throw new XmlException("Expected Encounters root");
        // Legacy private payloads lost log identities; reject them rather than inventing action links.
        if (history && document.DocumentElement.GetAttribute("AftercastHistoryVersion") != "1")
            throw new XmlException("Unsupported aftercast history version; expected 1");
        var result = new List<EncounterData>();
        var tagsSerializer = SwingTags();
        var sorter = GlobalTimeSorter;
        foreach (XmlNode node in document.DocumentElement.ChildNodes)
        {
            if (node is not XmlElement element) continue;
            if (element.Name != "Encounter") throw new XmlException("Unexpected encounter element: " + element.Name);
            var encounter = new EncounterData(element.GetAttribute("CharName") is { Length: > 0 } name ? name : ActGlobals.charName!, "Import", null!)
            { Title = RequiredAttribute(element, "Name"),
                // ACT interchange reconstructs log sorters; equal numbers alone cannot validate a source link.
                LogSourceIdentityKnown = history && element.HasAttribute("AftercastLogIdentityKnown") && bool.Parse(element.GetAttribute("AftercastLogIdentityKnown")) };
            if (history && element.HasAttribute("AftercastClipboardImport"))
                encounter.ClipboardImport = bool.Parse(element.GetAttribute("AftercastClipboardImport"));
            if (history && element.HasAttribute("AftercastDroppedLogLines"))
            {
                encounter.DroppedLogLines = long.Parse(element.GetAttribute("AftercastDroppedLogLines"), CultureInfo.InvariantCulture);
                if (encounter.DroppedLogLines < 0) throw new XmlException("Negative dropped log count");
            }
            foreach (XmlNode child in element.ChildNodes)
            {
                if (child is not XmlElement item) continue;
                switch (item.Name)
                {
                    case "StartTime": encounter.StartTimes.Add(ParseArchiveTime(item.InnerText)); break;
                    case "EndTime": encounter.EndTimes.Add(ParseArchiveTime(item.InnerText)); break;
                    case "Swing":
                        var action = new MasterSwing(int.TryParse(item.GetAttribute("SwingType"), out var swingType) ? swingType : 1,
                            bool.TryParse(item.GetAttribute("Critical"), out var critical) && critical,
                            string.IsNullOrWhiteSpace(item.GetAttribute("Special")) ? "None" : item.GetAttribute("Special"),
                            item.HasAttribute("AftercastNumber") ? new Dnum(long.Parse(item.GetAttribute("AftercastNumber"), CultureInfo.InvariantCulture), item.HasAttribute("AftercastLabel") ? item.GetAttribute("AftercastLabel") : null)
                                { DamageString2 = item.HasAttribute("AftercastSuffix") ? item.GetAttribute("AftercastSuffix") : null }
                                : ParseArchiveDamage(RequiredAttribute(item, "Damage")), ParseArchiveTime(RequiredAttribute(item, "Time")),
                            history ? int.Parse(RequiredAttribute(item, "TimeSorter"), CultureInfo.InvariantCulture)
                                : int.TryParse(item.GetAttribute("TimeSorter"), out var timeSorter) ? timeSorter : 0, RequiredAttribute(item, "AttackType"),
                            RequiredAttribute(item, "Combatant"), RequiredAttribute(item, "DamageType"), RequiredAttribute(item, "Victim"));
                        var tagCount = 0;
                        foreach (XmlNode tag in item.ChildNodes)
                            if (tag is XmlElement tags)
                            {
                                if (++tagCount > 1) throw new XmlException("Multiple swing tag dictionaries");
                                using var tagReader = new XmlNodeReader(tags);
                                action.Tags = tagsSerializer.ReadObject(tagReader) as Dictionary<string, object>
                                    ?? throw new XmlException("Swing tags must be a dictionary");
                            }
                        if (!CombatantData.SwingTypeToDamageTypeDataLinksOutgoing.TryGetValue(action.SwingType, out var categories) || categories.Count == 0)
                            throw new InvalidOperationException("Register outgoing categories for swing type " + action.SwingType + " before reconstruction");
                        encounter.AddCombatAction(action);
                        break;
                    case "LogLine":
                        var text = RequiredAttribute(item, "Line");
                        var time = item.HasAttribute("Time") ? ParseArchiveTime(item.GetAttribute("Time")) : GetDateTimeFromLog(text);
                        var logSorter = history ? int.Parse(RequiredAttribute(item, "AftercastTimeSorter"), CultureInfo.InvariantCulture) : checked(++sorter);
                        encounter.LogLines.Add(new(time, text, int.Parse(RequiredAttribute(item, "Type"), CultureInfo.InvariantCulture), logSorter));
                        break;
                    case "Ally": break;
                    default: throw new XmlException("Unexpected encounter child: " + item.Name);
                }
            }
            var allies = element.SelectNodes("Ally")!.Cast<XmlElement>().Select(ally => encounter.GetCombatant(ally.InnerText)
                ?? throw new XmlException("Unknown ally: " + ally.InnerText)).ToList();
            if (allies.Count > 0) encounter.SetAllies(allies);
            result.Add(encounter);
        }
        if (!history) GlobalTimeSorter = sorter;
        return result;
    }

    private static string RequiredAttribute(XmlElement element, string name) => element.HasAttribute(name)
        ? element.GetAttribute(name) : throw new XmlException("Missing " + name + " on " + element.Name);
    private static DateTime ParseArchiveTime(string value) => DateTime.Parse(value, CultureInfo.InvariantCulture, DateTimeStyles.RoundtripKind);

    private static Dnum ParseArchiveDamage(string text)
    {
        try { return ParseArchiveDamageValue(text); }
        catch (Exception error) when (error is FormatException or OverflowException) { return new Dnum(-9, text); }
    }
    private static Dnum ParseArchiveDamageValue(string text)
    {
        if (text == "Miss") return Dnum.Miss;
        if (text == "No Damage") return Dnum.NoDamage;
        if (text == "Death") return Dnum.Death;
        if (text.Contains('/'))
        {
            var total = 0L;
            foreach (Match match in Regex.Matches(text, @"\d+(?:,\d+)*", RegexOptions.None, TimeSpan.FromMilliseconds(100)))
                total = unchecked(total + long.Parse(match.Value, NumberStyles.AllowThousands, CultureInfo.CurrentCulture));
            return new(total, text);
        }
        if (text.Length > 0 && char.IsDigit(text[^1])) return long.Parse(text, NumberStyles.AllowThousands, CultureInfo.CurrentCulture);
        var parts = text.Split(['(', ')'], StringSplitOptions.RemoveEmptyEntries);
        return text.Contains('(') && parts.Length > 1 ? new Dnum(-9, parts[0]) { DamageString2 = "(" + parts[1] + ")" } : new Dnum(-9, text);
    }

    public byte[] MemCompressGZip(byte[] bytes)
    {
        ArgumentNullException.ThrowIfNull(bytes);
        return CompressArchive(bytes);
    }

    private static byte[] CompressArchive(ReadOnlySpan<byte> bytes)
    {
        if (bytes.Length > ArchiveLimit) throw new InvalidDataException("Archive exceeds 64 MiB");
        // .NET skips an unwritten gzip stream; emit a valid empty member so even [] round-trips.
        if (bytes.IsEmpty) return Convert.FromHexString("1F8B080000000000000303000000000000000000");
        using var output = new MemoryStream();
        using (var gzip = new GZipStream(output, CompressionMode.Compress, leaveOpen: true)) gzip.Write(bytes);
        return output.ToArray();
    }
    public byte[] MemDecompressGzip(byte[] bytes)
    {
        if (bytes.Length is < 18 or > ArchiveLimit || bytes[0] != 0x1f || bytes[1] != 0x8b) throw new InvalidDataException("Invalid gzip archive");
        using var input = new MemoryStream(bytes);
        using var gzip = new GZipStream(input, CompressionMode.Decompress);
        using var output = new MemoryStream();
        var buffer = new byte[8192];
        var checksum = uint.MaxValue;
        int length;
        while ((length = gzip.Read(buffer)) > 0)
        {
            if (output.Length + length > ArchiveLimit) throw new InvalidDataException("Expanded archive exceeds 64 MiB");
            output.Write(buffer, 0, length);
            foreach (var value in buffer.AsSpan(0, length))
                checksum = (checksum >> 8) ^ ArchiveCrc32[(byte)(checksum ^ value)];
        }
        // GZipStream can accept a missing footer; verify the single-member ACT checksum and size explicitly.
        if (BinaryPrimitives.ReadUInt32LittleEndian(bytes.AsSpan(bytes.Length - 4)) != output.Length ||
            BinaryPrimitives.ReadUInt32LittleEndian(bytes.AsSpan(bytes.Length - 8)) != ~checksum) throw new InvalidDataException("Gzip checksum or length mismatch");
        return output.ToArray();
    }

    private static readonly uint[] ArchiveCrc32 = CreateArchiveCrc32();
    private static uint[] CreateArchiveCrc32()
    {
        // Precompute the IEEE CRC-32 byte transitions once instead of branching eight times per expanded byte.
        var table = new uint[256];
        for (var index = 0; index < table.Length; index++)
        {
            var value = (uint)index;
            for (var bit = 0; bit < 8; bit++) value = (value >> 1) ^ ((value & 1) != 0 ? 0xedb88320U : 0);
            table[index] = value;
        }
        return table;
    }
}
