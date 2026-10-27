using System.Text;

namespace Aftercast.ActCompat
{
    public enum HistorySourceCaptureState { Ready, Consumed, Disposed }
    public sealed record CapturedSourceInfo(string Path, string Name, long Length, string Sha256, ImportTextOptions? Text);
    public sealed record HistoryCaptureProgress(long BytesCopied, long TotalBytes, int FilesCopied, int FilesTotal);
    public sealed class HistorySourceCapture : IDisposable
    {
        private readonly object gate = new();
        private Advanced_Combat_Tracker.CapturedImportSource[] sources;
        private HistorySourceCaptureState state;
        internal Advanced_Combat_Tracker.FormActMain Owner { get; }
        internal HistorySourceSnapshot Snapshot { get; }
        public IReadOnlyList<CapturedSourceInfo> Files { get; }
        public ImportRange Range { get; }
        public HistorySourceCaptureState State { get { lock (gate) return state; } }
        internal HistorySourceCapture(Advanced_Combat_Tracker.FormActMain owner, HistorySourceSnapshot snapshot,
            Advanced_Combat_Tracker.CapturedImportSource[] sources)
        {
            Owner = owner; Snapshot = snapshot; this.sources = sources;
            Range = new(snapshot.Start, snapshot.End);
            Files = Array.AsReadOnly(sources.Select(source => new CapturedSourceInfo(source.Path, source.Name,
                source.Image.Length, source.Image.Digest, source.Text)).ToArray());
        }
        internal Advanced_Combat_Tracker.CapturedImportSource[] Take()
        {
            lock (gate)
            {
                if (state != HistorySourceCaptureState.Ready) throw new InvalidOperationException("Capture is disposed or already consumed");
                var owned = sources; sources = []; state = HistorySourceCaptureState.Consumed; return owned;
            }
        }
        public void Dispose()
        {
            Advanced_Combat_Tracker.CapturedImportSource[] owned;
            lock (gate) { if (state != HistorySourceCaptureState.Ready) return; state = HistorySourceCaptureState.Disposed; owned = sources; sources = []; }
            Advanced_Combat_Tracker.CapturedImportSource.DisposeAll(owned);
        }
    }
}

namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;
    internal sealed record CapturedImportSource(string Path, string Name, Encoding Encoding, bool DetectBom, ImportTextOptions? Text, CapturedLogImage Image)
    {
        internal static void DisposeAll(IEnumerable<CapturedImportSource> sources)
        {
            var failures = new List<Exception>();
            foreach (var source in sources) try { source.Image.Dispose(); } catch (Exception error) { failures.Add(error); }
            if (failures.Count != 0) throw new AggregateException("Captured source cleanup failed", failures);
        }
    }
    public partial class FormActMain
    {
        private const long MaximumSourceCaptureBytes = 4L * 1024 * 1024 * 1024;
        private SourceCaptureWork? sourceCapture;
        private readonly object sourceCaptureGate = new();
        private bool sourceCaptureClosed;
        private int sourceArchiveBusy;
        private sealed class SourceCaptureWork : IDisposable
        {
            internal readonly CancellationTokenSource Stop;
            internal Task<HistorySourceCapture> Task = null!;
            internal long Bytes, Total;
            internal int Files, Count;
            internal SourceCaptureWork(CancellationToken cancellation) => Stop = CancellationTokenSource.CreateLinkedTokenSource(cancellation);
            internal HistoryCaptureProgress Progress => new(Interlocked.Read(ref Bytes), Interlocked.Read(ref Total), Volatile.Read(ref Files), Count);
            public void Dispose()
            {
                Stop.Cancel();
                try { if (Task.IsCompletedSuccessfully) Task.Result.Dispose(); }
                finally { Stop.Dispose(); }
            }
        }
        internal HistoryCaptureProgress CurrentHistoryCaptureProgress => sourceCapture?.Progress ?? throw new InvalidOperationException("No source capture request");
        private void EnsureSourceCaptureIdle()
        {
            lock (sourceCaptureGate)
            {
                ObjectDisposedException.ThrowIf(sourceCaptureClosed, this);
                if (sourceCapture is { } work && (!work.Task.IsCompleted || work.Task.IsCompletedSuccessfully && work.Task.Result.State == HistorySourceCaptureState.Ready))
                    throw new InvalidOperationException("Dispose or consume the pending source capture first");
            }
        }
        private void RequireSourceHistory(HistorySourceSnapshot snapshot)
        {
            if (!ReferenceEquals(snapshot.Owner, this)) throw new ArgumentException("Source snapshot belongs to another owner");
            var catalog = ReadHistoryCatalog();
            if (catalog.Revision != snapshot.IndexRevision || !catalog.Entries.Any(entry => entry.File == snapshot.HistoryId))
                throw new InvalidOperationException("History changed; refresh source selection");
        }
        internal Task<HistorySourceCapture> CaptureHistorySourceAsync(HistorySourceSnapshot snapshot, string path,
            ImportTextOptions? text, IReadOnlyList<ImportCompanion>? companions, long maximumBytes, CancellationToken cancellation)
        {
            RequireHistoryOwner(); ObjectDisposedException.ThrowIf(IsDisposed, this); ArgumentNullException.ThrowIfNull(snapshot);
            lock (CombatGate) EnsureCombatOpen();
            if (activeImport is not null || Volatile.Read(ref sourceArchiveBusy) != 0) throw new InvalidOperationException("Import/archive work is active");
            EnsureSourceCaptureIdle(); RequireSourceHistory(snapshot);
            if (maximumBytes <= 0 || maximumBytes > MaximumSourceCaptureBytes) throw new ArgumentOutOfRangeException(nameof(maximumBytes), "Capture budget must be 1..4 GiB");
            path = SourcePath(path);
            var candidate = snapshot.Candidates.SingleOrDefault(candidate => SourcePathComparer.Equals(candidate.Path, path) || candidate.Aliases.Contains(path, SourcePathComparer));
            if (candidate is not { State: HistorySourceState.Available, Observation: { } expected }) throw new InvalidOperationException("Explicitly select an available candidate path");
            var selected = companions?.ToArray() ?? [];
            if (selected.Length > 64) throw new ArgumentException("At most 64 companions can be captured");
            var names = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            foreach (var source in selected) { ArgumentNullException.ThrowIfNull(source); source.Validate(); if (!names.Add(source.Name)) throw new ArgumentException("Duplicate companion"); }
            selected = selected.OrderBy(source => source.Name, StringComparer.OrdinalIgnoreCase).ToArray();
            var encoding = text?.Resolve() ?? ImportTextOptions.Validate(LogEncoding);
            lock (sourceCaptureGate)
            {
                ObjectDisposedException.ThrowIf(sourceCaptureClosed, this);
                sourceCapture?.Dispose();
                var work = new SourceCaptureWork(cancellation) { Count = selected.Length + 1 };
                try { work.Task = Task.Run(() => CaptureSourceImages(snapshot, path, expected, text, encoding, selected, maximumBytes, work)); }
                catch { work.Stop.Dispose(); throw; }
                sourceCapture = work; return work.Task;
            }
        }
        private sealed record OpenCapture(string Path, string Name, ImportTextOptions? Text, Encoding Encoding, bool DetectBom,
            LogFileObservation Expected, FileStream File);
        private static void RejectCaptureLinks(string path)
        {
            if ((File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0 || (File.GetAttributes(Path.GetDirectoryName(path)!) & FileAttributes.ReparsePoint) != 0)
                throw new IOException("Selected capture cannot follow a linked file or parent directory");
        }
        private async Task<HistorySourceCapture> CaptureSourceImages(HistorySourceSnapshot snapshot, string path, LogFileObservation expected,
            ImportTextOptions? text, Encoding encoding, ImportCompanion[] selected, long maximumBytes, SourceCaptureWork work)
        {
            var token = work.Stop.Token; var opened = new List<OpenCapture>(); var captured = new List<CapturedImportSource>();
            var failures = new List<Exception>();
            try
            {
                token.ThrowIfCancellationRequested(); RequireSourceHistory(snapshot);
                var catalog = selected.Length == 0 ? [] : CompanionLogCatalog.Discover(path);
                var identities = new HashSet<LogFileIdentity>();
                for (var index = 0; index <= selected.Length; index++)
                {
                    token.ThrowIfCancellationRequested();
                    var side = index == 0 ? null : selected[index - 1];
                    var match = side is null ? null : catalog.SingleOrDefault(candidate => string.Equals(candidate.Name, side.Name, StringComparison.OrdinalIgnoreCase))
                        ?? throw new FileNotFoundException("Selected companion is missing: " + side.Name);
                    var sourcePath = match?.Path ?? path; RejectCaptureLinks(sourcePath);
                    var before = LogFileIdentity.Observe(sourcePath);
                    if (index == 0 && before != expected) throw new IOException("Selected main source changed; refresh candidates");
                    var file = new FileStream(sourcePath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete, 1, FileOptions.Asynchronous | FileOptions.SequentialScan);
                    try
                    {
                        if (LogFileIdentity.Observe(file.SafeFileHandle) != before) throw new IOException("Source changed during open");
                        if (!identities.Add(before.Identity)) throw new IOException("Selected sources alias the same file");
                        if (before.Length < 0 || before.Length > maximumBytes - work.Total) throw new InvalidDataException("Selected capture exceeds its aggregate byte budget");
                        var total = work.Total + before.Length; // Bounded subtraction avoids overflow even for huge observed files.
                        Interlocked.Exchange(ref work.Total, total);
                        var policy = side?.Text;
                        opened.Add(new(sourcePath, match?.Name ?? "", index == 0 ? text : policy,
                            policy?.Resolve() ?? encoding, index == 0 ? text?.DetectBom ?? true : policy?.DetectBom ?? text?.DetectBom ?? true, before, file));
                    }
                    catch { file.Dispose(); throw; }
                }
                foreach (var source in opened)
                {
                    token.ThrowIfCancellationRequested();
                    var image = await CapturedLogImage.CopyAsync(source.File, source.Expected.Length, AppDataFolder.FullName,
                        token, count => Interlocked.Add(ref work.Bytes, count)).ConfigureAwait(false);
                    captured.Add(new(source.Path, source.Name, source.Encoding, source.DetectBom, source.Text, image));
                    Interlocked.Increment(ref work.Files);
                }
                // Every source must still refer to the admitted version until the whole set is sealed.
                foreach (var source in opened)
                {
                    RejectCaptureLinks(source.Path);
                    if (LogFileIdentity.Observe(source.File.SafeFileHandle) != source.Expected || LogFileIdentity.Observe(source.Path) != source.Expected)
                        throw new IOException("Source moved, changed or was replaced during capture");
                }
                RequireSourceHistory(snapshot); token.ThrowIfCancellationRequested();
            }
            catch (Exception error) { failures.Add(error); }
            // Close every original handle before publishing. A close failure must also retire all images.
            foreach (var source in opened) try { source.File.Dispose(); } catch (Exception error) { failures.Add(error); }
            if (failures.Count != 0)
            {
                try { CapturedImportSource.DisposeAll(captured); } catch (Exception cleanup) { failures.Add(cleanup); }
                if (failures.Count == 1) System.Runtime.ExceptionServices.ExceptionDispatchInfo.Capture(failures[0]).Throw();
                throw new AggregateException("Source capture failed", failures);
            }
            try { token.ThrowIfCancellationRequested(); return new(this, snapshot, captured.ToArray()); }
            catch { CapturedImportSource.DisposeAll(captured); throw; }
        }
        internal void BeginCapturedHistoryImport(HistorySourceCapture capture)
        {
            RequireHistoryOwner(); ObjectDisposedException.ThrowIf(IsDisposed, this); ArgumentNullException.ThrowIfNull(capture);
            if (activeImport is not null || Volatile.Read(ref sourceArchiveBusy) != 0) throw new InvalidOperationException("Import/archive work is active");
            EnsureCanWaitForLogReader(); lock (CombatGate) { EnsureCombatOpen(); if (inCombat) throw new InvalidOperationException("End live combat before importing"); }
            RequireSourceHistory(capture.Snapshot);
            CapturedImportSource[] sources;
            lock (sourceCaptureGate)
            {
                ObjectDisposedException.ThrowIf(sourceCaptureClosed, this);
                if (!ReferenceEquals(capture.Owner, this) || sourceCapture is not { Task.IsCompletedSuccessfully: true } work || !ReferenceEquals(work.Task.Result, capture))
                    throw new InvalidOperationException("Captured source belongs to another or retired request");
                sources = capture.Take(); sourceCapture = null; work.Dispose();
            }
            try { BeginImport(sources[0].Path, capture.Range, null, sources.Skip(1).Select(source => new ImportCompanion(source.Name, source.Text)).ToArray(), sources, capture.Snapshot.CharacterHint); }
            catch (Exception error)
            {
                try { CapturedImportSource.DisposeAll(sources); }
                catch (Exception cleanup) { throw new AggregateException("Import admission and capture cleanup failed", error, cleanup); }
                throw;
            }
        }
        internal void CloseHistorySourceCapture()
        {
            SourceCaptureWork? work;
            lock (sourceCaptureGate) { sourceCaptureClosed = true; work = sourceCapture; sourceCapture = null; }
            if (work is null) return; work.Stop.Cancel();
            try { work.Task.GetAwaiter().GetResult(); }
            catch (OperationCanceledException) { }
            catch (Exception error) { WriteExceptionLog(error, "History source capture"); }
            finally { work.Dispose(); }
        }
    }
}
