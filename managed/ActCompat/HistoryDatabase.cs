using System.Globalization;
using System.Xml;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    public SortedList<DateTime, HistoryRecord> ZoneDatabase { get; set; } = new();
    public SortedList<DateTime, HistoryRecord> EncDatabase { get; set; } = new();
    public void EncDatabaseAdd(HistoryRecord record)
    {
        if (record.EndTime == DateTime.MinValue) record.EndTime = record.StartTime;
        lock (ActGlobals.actionDataLock) EncDatabase[record.StartTime] = record;
    }
    public void ZoneDatabaseAdd(HistoryRecord record)
    {
        if (record.Label is "Unknown Zone" or "Import Zone" || record.StartTime > record.EndTime) return;
        if (record.EndTime == DateTime.MaxValue) record.EndTime = record.StartTime.AddDays(1);
        lock (ActGlobals.actionDataLock)
        {
            if (ZoneDatabase.TryGetValue(record.StartTime, out var previous) && previous.Duration > record.Duration && previous.Duration != TimeSpan.FromHours(24)) return;
            ZoneDatabase[record.StartTime] = record;
        }
    }
    public void SaveXmlRecords(string path) => WriteAtomic(path, stream =>
    {
        using var writer = XmlWriter.Create(stream, new XmlWriterSettings { Indent = true, CloseOutput = false });
        writer.WriteStartElement("Records");
        lock (ActGlobals.actionDataLock)
            foreach (var record in ZoneDatabase.Values.Concat(EncDatabase.Values)) WriteHistoryRecord(writer, record);
        writer.WriteEndElement();
    }, overwrite: true);

    private static void WriteHistoryRecord(XmlWriter writer, HistoryRecord record)
    {
        writer.WriteStartElement("Record");
        WriteHistoryAttributes(writer, record);
        writer.WriteEndElement();
    }
    private static void WriteHistoryAttributes(XmlWriter writer, HistoryRecord record)
    {
        writer.WriteAttributeString("Type", record.Type.ToString(CultureInfo.InvariantCulture));
        writer.WriteAttributeString("StartTime", record.StartTime.ToBinary().ToString(CultureInfo.InvariantCulture));
        writer.WriteAttributeString("EndTime", record.EndTime.ToBinary().ToString(CultureInfo.InvariantCulture));
        writer.WriteAttributeString("Label", record.Label); writer.WriteAttributeString("CharName", record.CharName); writer.WriteAttributeString("FolderHint", record.FolderHint);
    }
    private static HistoryRecord ReadHistoryRecord(XmlElement element) => new(
        int.Parse(RequiredAttribute(element, "Type"), CultureInfo.InvariantCulture),
        DateTime.FromBinary(long.Parse(RequiredAttribute(element, "StartTime"), CultureInfo.InvariantCulture)),
        DateTime.FromBinary(long.Parse(RequiredAttribute(element, "EndTime"), CultureInfo.InvariantCulture)),
        RequiredAttribute(element, "Label"), element.GetAttribute("CharName"), element.GetAttribute("FolderHint"));

    public void LoadXmlRecords(string path)
    {
        if (!File.Exists(path)) return;
        var records = ReadArchiveDocument(path, "Records").SelectNodes("/Records/Record")!.Cast<XmlElement>().Select(ReadHistoryRecord).ToArray();
        if (records.Any(record => record.Type is not (0 or 1))) throw new XmlException("Unknown history record type");
        if (records.Any(record => record.Type == 0 && record.EndTime == DateTime.MaxValue && record.StartTime > DateTime.MaxValue.AddDays(-1)))
            throw new XmlException("Unclosed zone start cannot be normalized");
        lock (ActGlobals.actionDataLock)
            foreach (var record in records)
                if (record.Type == 0) ZoneDatabaseAdd(record); else EncDatabaseAdd(record);
    }

    private static XmlDocument ReadArchiveDocument(string path, string root)
    {
        using var reader = XmlReader.Create(path, new XmlReaderSettings { DtdProcessing = DtdProcessing.Prohibit, XmlResolver = null, MaxCharactersInDocument = ArchiveLimit });
        var document = new XmlDocument { XmlResolver = null }; document.Load(reader);
        if (document.DocumentElement?.Name != root) throw new XmlException("Expected " + root + " root");
        return document;
    }

    private static void WriteAtomic(string path, Action<Stream> write, bool overwrite)
    {
        var temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            using (var stream = new FileStream(temporary, FileMode.CreateNew, FileAccess.Write, FileShare.None))
            { write(stream); if (stream.Length > ArchiveLimit) throw new InvalidDataException("Archive output exceeds 64 MiB"); stream.Flush(true); }
            File.Move(temporary, path, overwrite);
        }
        finally { if (File.Exists(temporary)) File.Delete(temporary); }
    }
}
