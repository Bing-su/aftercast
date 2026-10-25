using System.Globalization;
using System.Runtime.CompilerServices;
using System.Security.Cryptography;
using System.Xml;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    private sealed record StoredEncounter(string File, string Zone, DateTime ZoneStart, HistoryRecord Record, bool Partial);
    private sealed record ArchiveIdentity(string File, DateTime ZoneStart);
    private List<StoredEncounter> storedEncounters = new();
    private readonly ConditionalWeakTable<EncounterData, ArchiveIdentity> archivedEncounters = new();
    private string HistoryDirectory => Path.Combine(AppDataFolder.FullName, "history");
    private string HistoryIndex => Path.Combine(HistoryDirectory, "index.xml");

    internal void LoadHistoryIndex()
    {
        if (!File.Exists(HistoryIndex)) return;
        var loaded = new List<StoredEncounter>();
        foreach (XmlElement element in ReadArchiveDocument(HistoryIndex, "Records").SelectNodes("/Records/Record")!)
        {
            var file = RequiredAttribute(element, "Archive");
            // Only content-addressed sibling files are allowed; a modified index cannot read e.g. ../../private.xml.
            if (file.Length != 68 || !file.EndsWith(".act", StringComparison.Ordinal) || !file[..64].All(char.IsAsciiHexDigit)) throw new XmlException("Invalid archive filename");
            var record = ReadHistoryRecord(element);
            if (record.Type != 1) throw new XmlException("Expected encounter history record");
            loaded.Add(new(file, RequiredAttribute(element, "ZoneName"), DateTime.FromBinary(long.Parse(RequiredAttribute(element, "ZoneStart"), CultureInfo.InvariantCulture)),
                record, bool.Parse(RequiredAttribute(element, "Partial"))));
        }
        if (loaded.Select(item => item.File).Distinct(StringComparer.Ordinal).Count() != loaded.Count) throw new XmlException("Duplicate history archive");
        storedEncounters = loaded;
        foreach (var entry in loaded) EncDatabaseAdd(entry.Record);
    }

    internal int ArchiveOperation(int command, string text, ulong id, bool compressed)
    {
        if (Environment.CurrentManagedThreadId != ownerThread || IsDispatchingCallback?.Invoke() == true)
            throw new InvalidOperationException("Archive operations require the owner thread outside dispatched callbacks");
        EnsureCanWaitForLogReader();
        var count = 0;
        RunWithLogReadingSuspended(() => RunWithCombatActionsSuspended(() =>
        {
            lock (CombatGate)
            {
                EnsureImportAdmission();
                if (inCombat) throw new InvalidOperationException("End live combat before archive operations");
                switch (command)
                {
                    case 1:
                        var imported = ReadEncounterFile(text);
                        count = AttachArchives(imported.Select(encounter => (encounter, (StoredEncounter?)null)).ToList());
                        break;
                    case 2:
                        var encounter = QueryEncounters().Select(item => item.Encounter).SingleOrDefault(item => Identity(item).Id == id)
                            ?? throw new ArgumentException("Encounter does not belong to this session");
                        var xml = EncodeEncounter(encounter, history: false);
                        var payload = compressed ? MemCompressGZip(xml) : xml;
                        WriteAtomic(text, stream => stream.Write(payload), overwrite: false);
                        count = 1;
                        break;
                    case 3: count = SaveEncounterHistory(); break;
                    case 4:
                        LoadHistoryIndex();
                        var restored = new List<(EncounterData, StoredEncounter?)>();
                        var existing = QueryEncounters().Select(item => archivedEncounters.TryGetValue(item.Encounter, out var identity) ? identity.File : null).ToHashSet();
                        var sorter = GlobalTimeSorter;
                        try
                        {
                            foreach (var entry in storedEncounters)
                                if (!existing.Contains(entry.File) && (entry.Record.Label.Contains(text, StringComparison.OrdinalIgnoreCase) ||
                                    entry.Zone.Contains(text, StringComparison.OrdinalIgnoreCase) || entry.Record.CharName.Contains(text, StringComparison.OrdinalIgnoreCase)))
                                {
                                    var bytes = ReadBoundedFile(Path.Combine(HistoryDirectory, entry.File));
                                    if (Convert.ToHexStringLower(SHA256.HashData(bytes)) + ".act" != entry.File) throw new InvalidDataException("History archive hash mismatch");
                                    var items = DecodeEncounterFile(bytes);
                                    if (items.Count != 1) throw new InvalidDataException("Expected one encounter per history archive");
                                    restored.Add((items[0], entry));
                                }
                        }
                        catch { GlobalTimeSorter = sorter; throw; }
                        count = AttachArchives(restored);
                        break;
                    default: throw new ArgumentException("Unknown archive command");
                }
            }
        }));
        return count;
    }

    private int AttachArchives(List<(EncounterData Encounter, StoredEncounter? Stored)> encounters)
    {
        var target = combatZones[0];
        foreach (var (encounter, stored) in encounters)
        {
            encounter.Parent = target;
            if (stored is not null)
            {
                encounter.ZoneName = stored.Zone;
                encounter.HistoryRecord = stored.Record;
                archivedEncounters.Add(encounter, new(stored.File, stored.ZoneStart));
                if (stored.Partial) Identity(encounter).Status = Aftercast.ActCompat.CombatSnapshotStatus.PartialImport;
            }
            target.Items.Add(encounter);
        }
        return encounters.Count;
    }

    private byte[] EncodeEncounter(EncounterData encounter, bool history)
    {
        using var output = new MemoryStream();
        WriteEncounterXml([encounter], output, true, history);
        if (output.Length > ArchiveLimit) throw new InvalidDataException("Encounter XML exceeds 64 MiB");
        return output.ToArray();
    }
    private static byte[] ReadBoundedFile(string path)
    {
        using var stream = File.OpenRead(path);
        if (stream.Length > ArchiveLimit) throw new InvalidDataException("Archive file exceeds 64 MiB");
        var result = new byte[checked((int)stream.Length)]; stream.ReadExactly(result); return result;
    }
    private List<EncounterData> ReadEncounterFile(string path) => DecodeEncounterFile(ReadBoundedFile(path));
    private List<EncounterData> DecodeEncounterFile(byte[] bytes)
    {
        var xml = bytes.Length >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b ? MemDecompressGzip(bytes) : bytes;
        using var input = new MemoryStream(xml); return XmlToEncounters(input);
    }

    private int SaveEncounterHistory()
    {
        Directory.CreateDirectory(HistoryDirectory);
        // Coordinate checkpoints across processes sharing the state directory; merge the last committed index under this lease.
        using var checkpoint = new FileStream(Path.Combine(HistoryDirectory, "checkpoint.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
        LoadHistoryIndex();
        var next = new List<StoredEncounter>(storedEncounters);
        var identities = new List<(EncounterData Encounter, string File, DateTime ZoneStart)>();
        foreach (var (encounter, _) in QueryEncounters())
        {
            if (encounter.Active || encounter.Items.Count == 0) continue;
            var bytes = MemCompressGZip(EncodeEncounter(encounter, history: true));
            var file = Convert.ToHexStringLower(SHA256.HashData(bytes)) + ".act";
            var path = Path.Combine(HistoryDirectory, file);
            if (!File.Exists(path)) WriteAtomic(path, stream => stream.Write(bytes), overwrite: false);
            else if (!ReadBoundedFile(path).SequenceEqual(bytes)) throw new InvalidDataException("Existing history payload is corrupt");
            if (archivedEncounters.TryGetValue(encounter, out var previous)) next.RemoveAll(item => item.File == previous.File);
            var record = new HistoryRecord(1, encounter.StartTime, encounter.EndTime, encounter.Title, encounter.CharName,
                encounter.HistoryRecord?.FolderHint ?? Path.GetDirectoryName(logFilePath) ?? "");
            next.RemoveAll(item => item.File == file);
            var zoneStart = archivedEncounters.TryGetValue(encounter, out var identity) ? identity.ZoneStart : encounter.Parent.StartTime;
            next.Add(new(file, encounter.ZoneName, zoneStart, record, Identity(encounter).Status == Aftercast.ActCompat.CombatSnapshotStatus.PartialImport));
            identities.Add((encounter, file, zoneStart));
        }
        // Immutable payloads are written first; a failed checkpoint leaves the previous complete index readable.
        WriteAtomic(HistoryIndex, stream =>
        {
            using var writer = XmlWriter.Create(stream, new XmlWriterSettings { Indent = true, CloseOutput = false });
            writer.WriteStartElement("Records");
            foreach (var entry in next)
            {
                writer.WriteStartElement("Record"); WriteHistoryAttributes(writer, entry.Record);
                writer.WriteAttributeString("Archive", entry.File); writer.WriteAttributeString("ZoneName", entry.Zone);
                writer.WriteAttributeString("ZoneStart", entry.ZoneStart.ToBinary().ToString(CultureInfo.InvariantCulture));
                writer.WriteAttributeString("Partial", entry.Partial.ToString()); writer.WriteEndElement();
            }
            writer.WriteEndElement();
        }, overwrite: true);
        storedEncounters = next;
        // shortcut: superseded payloads remain on disk; add explicit retention before maintaining large archives.
        foreach (var (encounter, file, zoneStart) in identities)
        {
            archivedEncounters.Remove(encounter); archivedEncounters.Add(encounter, new(file, zoneStart));
            encounter.HistoryRecord = next.First(item => item.File == file).Record;
            EncDatabaseAdd(encounter.HistoryRecord);
        }
        return next.Count;
    }
}
