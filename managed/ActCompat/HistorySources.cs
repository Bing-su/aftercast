using System.IO.Enumeration;
using System.Security;
using System.Text;

namespace Aftercast.ActCompat
{
    public sealed record HistorySourceQuery(string[] Paths, string[] Roots, string Filter = "*.log");
    public enum HistorySourceState { Available, Missing, Unavailable }
    public enum HistorySourceChange { New, MetadataUnchanged, MetadataChanged, Replaced, Moved }
    [Flags] public enum HistorySourceOrigin { Explicit = 1, AllowedRoot = 2 }
    public sealed record HistorySourceDiagnostic(string Path, HistorySourceState State, string Message);

    // No public constructor/ID accepts caller-supplied file stamps. These are proposals, never leases.
    public sealed class HistorySourceCandidate
    {
        public string Path { get; }
        public IReadOnlyList<string> Aliases { get; }
        public HistorySourceOrigin Origin { get; }
        public HistorySourceState State { get; }
        public HistorySourceChange Change { get; }
        public string? MovedFrom { get; }
        public long? Length => Observation?.Length;
        /// <summary>UTC filesystem modification ticks, not the parser's source wall clock.</summary>
        public long? ModifiedTicks => Observation?.ModifiedTicks;
        public bool CharacterHintMatches { get; }
        public bool FolderHintMatches { get; }
        public bool WrittenAfterEnd { get; }
        public string Message { get; }
        internal Advanced_Combat_Tracker.LogFileObservation? Observation { get; }
        internal HistorySourceCandidate(string path, string[] aliases, HistorySourceOrigin origin, HistorySourceState state,
            HistorySourceChange change, string? movedFrom, Advanced_Combat_Tracker.LogFileObservation? observation,
            string character, string folder, long end, string message)
        {
            Path = path; Aliases = Array.AsReadOnly(aliases); Origin = origin; State = state;
            Change = change; MovedFrom = movedFrom; Observation = observation; Message = message;
            CharacterHintMatches = character.Length != 0 && path.Contains(character, StringComparison.Ordinal);
            FolderHintMatches = folder.Length != 0 && string.Equals(System.IO.Path.GetFileName(System.IO.Path.GetDirectoryName(path)), folder, StringComparison.OrdinalIgnoreCase);
            // ACT's hint compares FileInfo.LastWriteTime (local ticks) with source EndTime.
            WrittenAfterEnd = observation is { } image && new DateTime(image.ModifiedTicks, DateTimeKind.Utc).ToLocalTime().Ticks > end;
        }
    }
    public sealed class HistorySourceSnapshot
    {
        public string IndexRevision { get; }
        public string HistoryId { get; }
        public DateTime Start { get; }
        public DateTime End { get; }
        public string CharacterHint { get; }
        public string FolderHint { get; }
        public IReadOnlyList<HistorySourceCandidate> Candidates { get; }
        public IReadOnlyList<HistorySourceDiagnostic> Diagnostics { get; }
        public bool Ambiguous => Candidates.Count(candidate => candidate.State == HistorySourceState.Available) > 1;
        public bool Missing => !Incomplete && !Candidates.Any(candidate => candidate.State == HistorySourceState.Available);
        public bool Incomplete => Diagnostics.Any(item => item.State == HistorySourceState.Unavailable) || Candidates.Any(candidate => candidate.State == HistorySourceState.Unavailable);
        public bool RequiresExplicitSelection => true;
        internal Advanced_Combat_Tracker.FormActMain Owner { get; }
        internal string[] Paths { get; }
        internal string[] Roots { get; }
        internal string Filter { get; }
        internal HistorySourceSnapshot(Advanced_Combat_Tracker.FormActMain owner, HistorySelection selection, DateTime start, DateTime end,
            string character, string folder, string[] paths, string[] roots, string filter,
            HistorySourceCandidate[] candidates, HistorySourceDiagnostic[] diagnostics)
        {
            Owner = owner; IndexRevision = selection.IndexRevision; HistoryId = selection.Ids[0]; Start = start; End = end;
            CharacterHint = character; FolderHint = folder; Paths = paths; Roots = roots; Filter = filter;
            Candidates = Array.AsReadOnly(candidates); Diagnostics = Array.AsReadOnly(diagnostics);
        }
    }
}

namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;
    public partial class FormActMain
    {
        private const int SourcePathLimit = 64, SourceEntryLimit = 4096;
        private static StringComparer SourcePathComparer => OperatingSystem.IsWindows() ? StringComparer.OrdinalIgnoreCase : StringComparer.Ordinal;
        private static string SourcePath(string path)
        {
            if (string.IsNullOrWhiteSpace(path) || !Path.IsPathFullyQualified(path) || path.Any(char.IsControl) || new UTF8Encoding(false, true).GetByteCount(path) > 8192)
                throw new ArgumentException("Source locations must be absolute, control-free and at most 8192 UTF-8 bytes");
            return Path.GetFullPath(path);
        }
        private static string ShortSourceError(Exception error)
        {
            var length = Math.Min(error.Message.Length, 2048);
            if (length != 0 && char.IsHighSurrogate(error.Message[length - 1])) length--;
            return error.Message[..length];
        }
        private static bool SourceIoError(Exception error) => error is IOException or UnauthorizedAccessException or NotSupportedException or SecurityException or OverflowException;
        private static HistorySourceState SourceErrorState(Exception error) => error is FileNotFoundException or DirectoryNotFoundException
            ? HistorySourceState.Missing : HistorySourceState.Unavailable;
        internal Task<HistorySourceSnapshot> ReadHistorySourcesAsync(HistorySelection selection, HistorySourceQuery query,
            HistorySourceSnapshot? previous = null, bool asynchronous = true, CancellationToken cancellation = default)
        {
            RequireHistoryOwner(); ObjectDisposedException.ThrowIf(IsDisposed, this);
            ArgumentNullException.ThrowIfNull(selection); ArgumentNullException.ThrowIfNull(query);
            if (selection.IndexRevision is not { Length: 64 } revision || revision.AsSpan().ContainsAnyExcept("0123456789abcdef") || selection.Ids is not { Length: 1 })
                throw new ArgumentException("Choose exactly one entry from a current history snapshot");
            var file = selection.Ids[0]; ArgumentNullException.ThrowIfNull(file); ValidateArchiveName(file);
            if (query.Paths is null || query.Roots is null || query.Paths.Length > 16 || query.Roots.Length > 8 ||
                string.IsNullOrWhiteSpace(query.Filter) || query.Filter.Length > 256 || query.Filter.Any(c => char.IsControl(c) || "/\\:".Contains(c)))
                throw new ArgumentException("Invalid bounded source search scope");
            // Copy caller arrays before yielding; hints never add paths to these authorized locations.
            var paths = query.Paths.Select(SourcePath).Distinct(SourcePathComparer).Order(SourcePathComparer).ToArray();
            var roots = query.Roots.Select(SourcePath).Distinct(SourcePathComparer).Order(SourcePathComparer).ToArray();
            if (previous is not null && (!ReferenceEquals(previous.Owner, this) || previous.IndexRevision != revision || previous.HistoryId != file ||
                previous.Filter != query.Filter || !previous.Paths.SequenceEqual(paths, SourcePathComparer) || !previous.Roots.SequenceEqual(roots, SourcePathComparer)))
                throw new ArgumentException("Previous source snapshot belongs to another owner, history or search scope");
            var ownedSelection = new HistorySelection(revision, [file]);
            return ArchiveIo(asynchronous, () =>
            {
                cancellation.ThrowIfCancellationRequested();
                var catalog = ReadHistoryCatalog();
                if (catalog.Revision != revision) throw new InvalidOperationException("History index changed; refresh before proposing sources");
                var entry = catalog.Entries.SingleOrDefault(item => item.File == file) ?? throw new InvalidOperationException("Selected history entry no longer exists");
                var proposed = CollectSourcePaths(paths, roots, query.Filter, previous, cancellation);
                var candidates = ObserveSources(proposed.Paths, proposed.Diagnostics, paths, previous, entry.Character, entry.FolderHint, entry.End.Ticks, cancellation);
                cancellation.ThrowIfCancellationRequested();
                if (ReadHistoryCatalog().Revision != revision) throw new InvalidOperationException("History index changed during source discovery");
                return new HistorySourceSnapshot(this, ownedSelection, entry.Start, entry.End, entry.Character, entry.FolderHint,
                    paths, roots, query.Filter, candidates, proposed.Diagnostics.ToArray());
            });
        }
        private sealed record SourcePaths(SortedDictionary<string, HistorySourceOrigin> Paths, List<HistorySourceDiagnostic> Diagnostics);
        private static SourcePaths CollectSourcePaths(string[] paths, string[] roots, string filter, HistorySourceSnapshot? previous, CancellationToken cancellation)
        {
            var found = new SortedDictionary<string, HistorySourceOrigin>(SourcePathComparer);
            var diagnostics = new List<HistorySourceDiagnostic>();
            void Add(string path, HistorySourceOrigin origin)
            {
                found.TryGetValue(path, out var prior); found[path] = prior | origin;
                if (found.Count > SourcePathLimit) throw new InvalidDataException("Source search exceeds 64 paths including previous observations");
            }
            foreach (var path in paths) Add(path, HistorySourceOrigin.Explicit);
            var visited = 0;
            foreach (var root in roots)
            {
                cancellation.ThrowIfCancellationRequested();
                try
                {
                    var attributes = File.GetAttributes(root);
                    if ((attributes & FileAttributes.ReparsePoint) != 0 || (attributes & FileAttributes.Directory) == 0)
                        throw new IOException("Allowed root must be a directory without a final symbolic link");
                    foreach (var path in Directory.EnumerateFileSystemEntries(root))
                    {
                        cancellation.ThrowIfCancellationRequested();
                        if (++visited > SourceEntryLimit) throw new InvalidDataException("Source roots exceed 4096 total directory entries");
                        if (!FileSystemName.MatchesSimpleExpression(filter, Path.GetFileName(path), ignoreCase: true)) continue;
                        Add(SourcePath(path), HistorySourceOrigin.AllowedRoot);
                    }
                }
                catch (InvalidDataException) { throw; } // Never turn a bound violation into a silently partial catalog.
                catch (Exception error) when (SourceIoError(error)) { diagnostics.Add(new(root, SourceErrorState(error), ShortSourceError(error))); }
            }
            // Only previously observed exact paths in this identical authorized scope are revisited.
            if (previous is not null) foreach (var candidate in previous.Candidates)
                foreach (var path in new[] { candidate.Path }.Concat(candidate.Aliases)) Add(path, candidate.Origin);
            return new(found, diagnostics);
        }
        private static HistorySourceCandidate[] ObserveSources(SortedDictionary<string, HistorySourceOrigin> paths, List<HistorySourceDiagnostic> roots,
            string[] explicitPaths, HistorySourceSnapshot? previous,
            string character, string folder, long end, CancellationToken cancellation)
        {
            var candidates = new List<HistorySourceCandidate>();
            var observed = new Dictionary<LogFileIdentity, int>();
            foreach (var (path, origin) in paths)
            {
                cancellation.ThrowIfCancellationRequested();
                LogFileObservation? stamp = null; var state = HistorySourceState.Available; var message = "";
                // A failed/linked root must not be traversed through an old proposal path.
                var blocked = explicitPaths.Contains(path, SourcePathComparer) ? null : roots.FirstOrDefault(root =>
                    SourcePathComparer.Equals(Path.GetDirectoryName(path), root.Path));
                if (blocked is not null) { state = blocked.State; message = blocked.Message; }
                else
                {
                    try { stamp = LogFileIdentity.Observe(path); }
                    catch (Exception error) when (SourceIoError(error)) { state = SourceErrorState(error); message = ShortSourceError(error); }
                }
                if (stamp is { } current && observed.TryGetValue(current.Identity, out var duplicate))
                {
                    var first = candidates[duplicate];
                    candidates[duplicate] = new(first.Path, [..first.Aliases, path], first.Origin | origin, first.State,
                        first.Change, first.MovedFrom, first.Observation, character, folder, end, first.Message);
                    continue;
                }
                var old = previous?.Candidates.FirstOrDefault(candidate => SourcePathComparer.Equals(candidate.Path, path) || candidate.Aliases.Contains(path, SourcePathComparer));
                var moved = stamp is { } image ? previous?.Candidates.FirstOrDefault(candidate => candidate.Observation?.Identity == image.Identity) : null;
                var change = old?.Observation is { } before && stamp is { } after
                    ? before.Identity != after.Identity ? HistorySourceChange.Replaced : before != after ? HistorySourceChange.MetadataChanged : HistorySourceChange.MetadataUnchanged
                    : moved is not null ? HistorySourceChange.Moved : HistorySourceChange.New;
                if (stamp is { } value) observed.Add(value.Identity, candidates.Count);
                candidates.Add(new(path, [], origin, state, change, change == HistorySourceChange.Moved ? moved!.Path : null,
                    stamp, character, folder, end, message));
            }
            return candidates.ToArray();
        }
    }
}
