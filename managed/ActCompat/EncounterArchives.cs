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
        ApplyHistoryIndex(ReadArchiveDocument(HistoryIndex, "Records"));
    }

    private void ApplyHistoryIndex(XmlDocument document)
    {
        var loaded = ParseHistoryCatalog(document).Select(entry => entry.Stored()).ToList();
        storedEncounters = loaded;
        foreach (var entry in loaded) EncDatabaseAdd(entry.Record);
    }

    internal int ArchiveOperation(int command, string text, ulong id, bool compressed) =>
        ArchiveOperationAsync(command, text, id, compressed, false).GetAwaiter().GetResult();

    // The synchronous compatibility path uses already-completed tasks; the bridge path yields for disk I/O.
    private static Task<T> ArchiveIo<T>(bool asynchronous, Func<T> operation) =>
        asynchronous ? Task.Run(operation) : Task.FromResult(operation());

    private async Task LoadHistoryIndexAsync(bool asynchronous)
    {
        var document = await ArchiveIo(asynchronous, () => File.Exists(HistoryIndex) ? ReadArchiveDocument(HistoryIndex, "Records") : null);
        lock (CombatGate)
        {
            if (document is not null) ApplyHistoryIndex(document);
            else storedEncounters = []; // A removed index must not leave stale cached restore candidates.
        }
    }

    internal async Task<int> ArchiveOperationAsync(int command, string text, ulong id, bool compressed, bool asynchronous = true, Aftercast.ActCompat.HistorySelection? selection = null)
    {
        if (Environment.CurrentManagedThreadId != ownerThread || IsDispatchingCallback?.Invoke() == true)
            throw new InvalidOperationException("Archive operations require the owner thread outside dispatched callbacks");
        EnsureCanWaitForLogReader();
        using var logs = SuspendLogReading(false, wait: !asynchronous);
        // Yield while admitted callbacks/actions settle; native owner messages keep running between checks.
        while (!logs.IsReady) await Task.Delay(1);
        using var combat = SuspendCombatActions(wait: !asynchronous);
        while (!combat.IsReady) await Task.Delay(1);
        lock (CombatGate)
        {
            EnsureImportAdmission();
            if (inCombat) throw new InvalidOperationException("End live combat before archive operations");
        }
        switch (command)
        {
            case 1:
                var bytes = await ArchiveIo(asynchronous, () => ExpandEncounterFile(ReadBoundedFile(text)));
                lock (CombatGate) return AttachArchives(DecodeEncounterXml(bytes).Select(encounter => (encounter, (StoredEncounter?)null)));
            case 2: return await ExportEncounterAsync(text, id, compressed, asynchronous);
            case 3: return await SaveEncounterHistoryAsync(asynchronous);
            case 4: return await RestoreEncounterHistoryAsync(text, asynchronous);
            case 5: return await RestoreSelectedEntriesAsync(selection ?? throw new ArgumentException("Missing history selection"),asynchronous);
            default: throw new ArgumentException("Unknown archive command");
        }
    }

    private async Task<int> ExportEncounterAsync(string path, ulong id, bool compressed, bool asynchronous)
    {
        ArraySegment<byte> xml;
        lock (CombatGate)
        {
            var encounter = QueryEncounters().Select(item => item.Encounter).SingleOrDefault(item => Identity(item).Id == id)
                ?? throw new ArgumentException("Encounter does not belong to this session");
            xml = EncodeEncounter(encounter, history: false);
        }
        return await ArchiveIo(asynchronous, () => {
            var payload = compressed ? CompressArchive(xml.AsSpan()).AsMemory() : xml.AsMemory();
            WriteAtomic(path, stream => stream.Write(payload.Span), overwrite: false);
            return 1;
        });
    }

    private async Task<int> RestoreEncounterHistoryAsync(string filter, bool asynchronous)
    {
        await LoadHistoryIndexAsync(asynchronous);
        var selected = storedEncounters.Where(entry => entry.Record.Label.Contains(filter,StringComparison.OrdinalIgnoreCase) ||
            entry.Zone.Contains(filter,StringComparison.OrdinalIgnoreCase) || entry.Record.CharName.Contains(filter,StringComparison.OrdinalIgnoreCase)).ToArray();
        return await RestoreEntriesAsync(selected,asynchronous,long.MaxValue,null);
    }

    private async Task<int> RestoreEntriesAsync(IEnumerable<StoredEncounter> entries,bool asynchronous,long expandedLimit,string? indexRevision)
    {
        var restored = new List<(EncounterData, StoredEncounter?)>();
        long expanded = 0;
        var existing = QueryEncounters().Select(item => archivedEncounters.TryGetValue(item.Encounter, out var identity) ? identity.File : null).ToHashSet();
        var sorter = GlobalTimeSorter;
        try
        {
            foreach (var entry in entries)
            {
                if (existing.Contains(entry.File)) continue;
                var bytes = await ArchiveIo(asynchronous, () => {
                    var payload = ReadBoundedFile(Path.Combine(HistoryDirectory, entry.File));
                    if (Convert.ToHexStringLower(SHA256.HashData(payload)) + ".act" != entry.File) throw new InvalidDataException("History archive hash mismatch");
                    return ExpandEncounterFile(payload);
                });
                expanded += bytes.Length;
                if (expanded > expandedLimit) throw new InvalidDataException("Selected history exceeds 128 MiB expanded XML budget");
                List<EncounterData> items;
                lock (CombatGate) items = DecodeEncounterXml(bytes, history: true);
                if (items.Count != 1) throw new InvalidDataException("Expected one encounter per history archive");
                restored.Add((items[0], entry));
            }
            if (indexRevision is not null && (await ArchiveIo(asynchronous,ReadHistoryCatalog)).Revision != indexRevision)
                throw new InvalidOperationException("History index changed during restoration");
        }
        catch { GlobalTimeSorter = sorter; throw; }
        // Parse every selected payload before attaching any graph, e.g. a corrupt second archive publishes neither.
        lock (CombatGate) return AttachArchives(restored);
    }

    private int AttachArchives(IEnumerable<(EncounterData Encounter, StoredEncounter? Stored)> encounters)
    {
        var target = combatZones[0];
        var count = 0;
        foreach (var (encounter, stored) in encounters)
        {
            encounter.Parent = target;
            if (stored is not null)
            {
                encounter.ZoneName = stored.Zone;
                encounter.HistoryRecord = stored.Record;
                EncDatabaseAdd(stored.Record);
                archivedEncounters.Add(encounter, new(stored.File, stored.ZoneStart));
                if (stored.Partial) Identity(encounter).Status = Aftercast.ActCompat.CombatSnapshotStatus.PartialImport;
            }
            target.Items.Add(encounter);
            count++;
        }
        return count;
    }

    private ArraySegment<byte> EncodeEncounter(EncounterData encounter, bool history)
    {
        using var output = new MemoryStream();
        WriteEncounterXml([encounter], output, true, history);
        if (output.Length > ArchiveLimit) throw new InvalidDataException("Encounter XML exceeds 64 MiB");
        // Retain only the used range, e.g. compression can read the XML without making a second full-size array.
        return new(output.GetBuffer(), 0, checked((int)output.Length));
    }
    private static byte[] ReadBoundedFile(string path)
    {
        using var stream = File.OpenRead(path);
        if (stream.Length > ArchiveLimit) throw new InvalidDataException("Archive file exceeds 64 MiB");
        var result = new byte[checked((int)stream.Length)];
        stream.ReadExactly(result);
        return result;
    }
    private List<EncounterData> DecodeEncounterXml(byte[] xml, bool history = false)
    {
        // Expansion and its size budget belong to the I/O stage; never expand another layer here.
        using var input = new MemoryStream(xml);
        return ReadEncounterXml(input, history);
    }

    private byte[] ExpandEncounterFile(byte[] bytes) =>
        bytes.Length >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b ? MemDecompressGzip(bytes) : bytes;

    private async Task<int> SaveEncounterHistoryAsync(bool asynchronous)
    {
        // Coordinate checkpoints across processes sharing the state directory; merge the last committed index under this lease.
        using var checkpoint = await ArchiveIo(asynchronous, () => {
            Directory.CreateDirectory(HistoryDirectory);
            return new FileStream(Path.Combine(HistoryDirectory, "checkpoint.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
        });
        await LoadHistoryIndexAsync(asynchronous);
        var next = new List<StoredEncounter>(storedEncounters);
        var identities = new List<(EncounterData Encounter, string File, DateTime ZoneStart)>();
        foreach (var (encounter, _) in QueryEncounters())
        {
            if (encounter.Active || encounter.Items.Count == 0) continue;
            if (!encounter.Items.Values.Any(actor => actor.AllOut.TryGetValue("All", out var all) && all.Items.Count > 0))
                throw new InvalidOperationException("Encounter has no outgoing swings to archive; its manual graph cannot be reconstructed");
            ArraySegment<byte> xml;
            lock (CombatGate) xml = EncodeEncounter(encounter, history: true);
            // Compress, name and persist owned bytes together; hashing a large archive must not stall owner dispatch.
            var file = await ArchiveIo(asynchronous, () => {
                var bytes = CompressArchive(xml.AsSpan());
                var file = Convert.ToHexStringLower(SHA256.HashData(bytes)) + ".act";
                var path = Path.Combine(HistoryDirectory, file);
                if (!File.Exists(path)) WriteAtomic(path, stream => stream.Write(bytes), overwrite: false);
                else if (!ReadBoundedFile(path).SequenceEqual(bytes)) throw new InvalidDataException("Existing history payload is corrupt");
                return file;
            });
            if (archivedEncounters.TryGetValue(encounter, out var previous)) next.RemoveAll(item => item.File == previous.File);
            var record = new HistoryRecord(1, encounter.StartTime, encounter.EndTime, encounter.Title, encounter.CharName,
                encounter.HistoryRecord?.FolderHint ?? Path.GetDirectoryName(logFilePath) ?? "");
            next.RemoveAll(item => item.File == file);
            var zoneStart = archivedEncounters.TryGetValue(encounter, out var identity) ? identity.ZoneStart : encounter.Parent.StartTime;
            next.Add(new(file, encounter.ZoneName, zoneStart, record, Identity(encounter).Status == Aftercast.ActCompat.CombatSnapshotStatus.PartialImport));
            identities.Add((encounter, file, zoneStart));
        }
        // Immutable payloads are written first; a failed checkpoint leaves the previous complete index readable.
        await ArchiveIo(asynchronous, () => { WriteAtomic(HistoryIndex, stream =>
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
        }, overwrite: true); return 0; });
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
