using System.Globalization;
using System.Security.Cryptography;
using System.Xml;

namespace Aftercast.ActCompat
{
    public enum HistorySearchScope { Any, Zone, Title, Character }
    public sealed record HistoryQuery(string Text = "", HistorySearchScope Scope = HistorySearchScope.Any,
        long? StartTicks = null, long? EndTicks = null, int Offset = 0, int Limit = 16);
    public sealed record HistoryEntry(string Id, string Title, string Zone, string Character, string FolderHint,
        long StartTicks, int StartKind, long EndTicks, int EndKind, bool Partial, bool PayloadPresent, bool AlreadyRestored);
    public sealed record HistoryPage(string IndexRevision, bool IndexPresent, int IndexedCount, int Total, HistoryEntry[] Items);
    public sealed record HistorySelection(string IndexRevision, string[] Ids);
}

namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;

    public partial class FormActMain
    {
        private sealed record CatalogEntry(string File, string Zone, DateTime ZoneStart, DateTime Start, DateTime End,
            string Title, string Character, string FolderHint, bool Partial)
        {
            internal StoredEncounter Stored() => new(File,Zone,ZoneStart,new HistoryRecord(1,Start,End,Title,Character,FolderHint),Partial);
        }
        private sealed record HistoryCatalog(string Revision, bool Present, List<CatalogEntry> Entries);
        private const int HistorySelectionLimit = 64;
        private const long SelectedHistoryXmlLimit = 128L * 1024 * 1024;
        private static void ValidateArchiveName(string file)
        {
            if (file.Length != 68 || !file.EndsWith(".act",StringComparison.Ordinal) ||
                file.AsSpan(0,64).ContainsAnyExcept("0123456789abcdefABCDEF")) throw new XmlException("Invalid archive filename");
        }
        private static List<CatalogEntry> ParseHistoryCatalog(XmlDocument document)
        {
            var entries = new List<CatalogEntry>();
            var files = new HashSet<string>(StringComparer.Ordinal);
            foreach (XmlElement element in document.SelectNodes("/Records/Record")!)
            {
                if (entries.Count >= 100000) throw new InvalidDataException("History index exceeds 100000 records");
                var file = RequiredAttribute(element,"Archive"); ValidateArchiveName(file);
                if (!files.Add(file)) throw new XmlException("Duplicate history archive");
                if (int.Parse(RequiredAttribute(element,"Type"),CultureInfo.InvariantCulture) != 1) throw new XmlException("Expected encounter history record");
                DateTime Time(string name) => DateTime.FromBinary(long.Parse(RequiredAttribute(element,name),CultureInfo.InvariantCulture));
                var start = Time("StartTime"); var end = Time("EndTime");
                if (end == DateTime.MinValue) end = start;
                if (end < start) throw new XmlException("History end precedes start");
                // Browse metadata without HistoryRecord's string interning or public database mutation.
                entries.Add(new(file,RequiredAttribute(element,"ZoneName"),Time("ZoneStart"),start,end,
                    RequiredAttribute(element,"Label"),element.GetAttribute("CharName"),element.GetAttribute("FolderHint"),
                    bool.Parse(RequiredAttribute(element,"Partial"))));
            }
            return entries;
        }
        private HistoryCatalog ReadHistoryCatalog()
        {
            byte[] bytes;
            try
            {
                // One open descriptor binds hashing and parsing to the same committed index bytes.
                using var stream = new FileStream(HistoryIndex,FileMode.Open,FileAccess.Read,FileShare.Read | FileShare.Delete);
                if (stream.Length > ArchiveLimit) throw new InvalidDataException("History index exceeds 64 MiB");
                bytes = new byte[checked((int)stream.Length)]; stream.ReadExactly(bytes);
            }
            catch (FileNotFoundException) { return new(Convert.ToHexStringLower(SHA256.HashData(Array.Empty<byte>())),false,[]); }
            catch (DirectoryNotFoundException) { return new(Convert.ToHexStringLower(SHA256.HashData(Array.Empty<byte>())),false,[]); }
            using var input = new MemoryStream(bytes,false);
            using var reader = XmlReader.Create(input,new XmlReaderSettings
            { DtdProcessing = DtdProcessing.Prohibit, XmlResolver = null, MaxCharactersInDocument = ArchiveLimit });
            var document = new XmlDocument { XmlResolver = null }; document.Load(reader);
            if (document.DocumentElement?.Name != "Records") throw new XmlException("Expected Records root");
            return new(Convert.ToHexStringLower(SHA256.HashData(bytes)),true,ParseHistoryCatalog(document));
        }
        private void RequireHistoryOwner()
        {
            if (Environment.CurrentManagedThreadId != ownerThread || IsDispatchingCallback?.Invoke() == true)
                throw new InvalidOperationException("History operations require the owner outside dispatched callbacks");
        }
        private static void ValidateHistoryQuery(HistoryQuery query)
        {
            ArgumentNullException.ThrowIfNull(query); ArgumentNullException.ThrowIfNull(query.Text);
            if (query.Text.Length > 4096 || query.Text.Contains('\0') || query.Offset < 0 || query.Limit is < 1 or > 16 ||
                (int)query.Scope is < 0 or > 3 || query.StartTicks is < 0 or > 3155378975999999999L ||
                query.EndTicks is < 0 or > 3155378976000000000L || query.StartTicks is { } start && query.EndTicks is { } end && start >= end)
                throw new ArgumentException("Invalid history query bounds");
        }
        internal async Task<HistoryPage> ReadHistoryAsync(HistoryQuery query,bool asynchronous = true)
        {
            RequireHistoryOwner(); ValidateHistoryQuery(query);
            HashSet<string?> restored;
            lock (CombatGate) restored = QueryEncounters().Select(item => archivedEncounters.TryGetValue(item.Encounter,out var identity) ? identity.File : null).ToHashSet();
            return await ArchiveIo(asynchronous,() =>
            {
                var catalog = ReadHistoryCatalog();
                bool Contains(string text) => text.Contains(query.Text,StringComparison.OrdinalIgnoreCase);
                bool Match(CatalogEntry entry) => (query.StartTicks is null || entry.Start.Ticks >= query.StartTicks) &&
                    (query.EndTicks is null || entry.Start.Ticks < query.EndTicks) && (query.Scope switch
                    { HistorySearchScope.Zone => Contains(entry.Zone), HistorySearchScope.Title => Contains(entry.Title),
                      HistorySearchScope.Character => Contains(entry.Character), _ => Contains(entry.Zone) || Contains(entry.Title) || Contains(entry.Character) });
                var matches = catalog.Entries.Where(Match).OrderByDescending(entry => entry.Start.Ticks).ThenBy(entry => entry.File,StringComparer.Ordinal).ToArray();
                var items = matches.Skip(query.Offset).Take(query.Limit).Select(entry => new HistoryEntry(entry.File,entry.Title,entry.Zone,
                    entry.Character,entry.FolderHint,entry.Start.Ticks,(int)entry.Start.Kind,entry.End.Ticks,(int)entry.End.Kind,
                    entry.Partial,File.Exists(Path.Combine(HistoryDirectory,entry.File)),restored.Contains(entry.File))).ToArray();
                return new HistoryPage(catalog.Revision,catalog.Present,catalog.Entries.Count,matches.Length,items);
            });
        }
        internal Task<int> RestoreSelectedHistoryAsync(HistorySelection selection,bool asynchronous = true)
        {
            RequireHistoryOwner(); ArgumentNullException.ThrowIfNull(selection); ArgumentNullException.ThrowIfNull(selection.Ids);
            if (selection.IndexRevision is not { Length: 64 } revision || revision.AsSpan().ContainsAnyExcept("0123456789abcdef") ||
                selection.Ids.Length is < 1 or > HistorySelectionLimit) throw new ArgumentException("Invalid history selection");
            var owned = selection.Ids.ToArray();
            foreach (var id in owned) { ArgumentNullException.ThrowIfNull(id); ValidateArchiveName(id); }
            if (owned.Distinct(StringComparer.Ordinal).Count() != owned.Length) throw new ArgumentException("Duplicate history selection");
            return ArchiveOperationAsync(5,"",0,false,asynchronous,new(revision,owned));
        }
        private async Task<int> RestoreSelectedEntriesAsync(HistorySelection selection,bool asynchronous)
        {
            // Cooperating checkpoint writers use the same lease; keep it through payload reads and final attach.
            using var checkpoint = await ArchiveIo(asynchronous,() => new FileStream(Path.Combine(HistoryDirectory,"checkpoint.lock"),
                FileMode.OpenOrCreate,FileAccess.ReadWrite,FileShare.None));
            // Yield the owner even when the lease I/O completed inline, before catalog/payload work.
            if (asynchronous) await Task.Yield();
            var catalog = await ArchiveIo(asynchronous,ReadHistoryCatalog);
            if (catalog.Revision != selection.IndexRevision) throw new InvalidOperationException("History index changed; refresh before restoring");
            var selected = new List<StoredEncounter>();
            foreach (var id in selection.Ids)
            {
                var entry = catalog.Entries.Find(entry => entry.File == id) ?? throw new InvalidOperationException("Selected history entry no longer exists");
                selected.Add(entry.Stored());
            }
            return await RestoreEntriesAsync(selected,asynchronous,SelectedHistoryXmlLimit,selection.IndexRevision);
        }
    }
}
