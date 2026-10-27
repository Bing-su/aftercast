using System.Text;
using System.Threading.Channels;
using Aftercast.ActCompat;

namespace Aftercast.ActCompat
{
    // Explicit companion selection is copied at admission; null Text inherits the admitted main codec.
    public sealed record ImportCompanion(string Name, ImportTextOptions? Text = null)
    {
        public void Validate() { Advanced_Combat_Tracker.CompanionLogCatalog.ValidateName(Name); _ = new UTF8Encoding(false, true).GetByteCount(Name); Text?.Validate(); }
    }
}

namespace Advanced_Combat_Tracker
{
    internal readonly record struct ImportSourceRecord(string Line, DateTime Time, string Path, string Name, int Source, bool BeyondMain = false);

    // One record per source is exposed to the owner. Timestamp callbacks never run on the disk task.
    internal sealed class ImportSourceInput(string path, Encoding encoding, bool detectBom, ImportCompanion[] selected, Action<string> warning, CapturedImportSource[]? captured = null)
    {
        private sealed class Source
        {
            internal string Path = "", Name = "";
            internal ImportLogReader? Reader;
            internal ImportSourceRecord? Head;
            internal bool Ended;
            internal long Bytes;
        }
        private readonly Source[] sources = Enumerable.Range(0, selected.Length + 1).Select(_ => new Source()).ToArray();
        private readonly Channel<(int Source, string? Line, bool Incomplete)> responses = Channel.CreateBounded<(int, string?, bool)>(1);
        // At most one command per source can be outstanding; heads must return before another consume.
        private readonly Channel<int> requests = Channel.CreateBounded<int>(selected.Length + 1);
        private long totalBytes, bytesRead, headCharacters;
        private bool hasMainRecord, companionsStarted;
        private DateTime mainWatermark = DateTime.MinValue;
        internal long TotalBytes => Interlocked.Read(ref totalBytes);
        internal long BytesRead => Interlocked.Read(ref bytesRead);
        internal Exception? Failure { get; private set; }
        internal bool Completed => sources.All(source => source.Ended);

        internal Task Start(CancellationToken cancellation) => Task.Run(async () =>
        {
            try
            {
                var catalog = captured is null ? CompanionLogCatalog.Discover(path) : Array.Empty<CompanionLogSource>();
                if (captured is not null && captured.Length != sources.Length) throw new InvalidOperationException("Captured source count mismatch");
                var identities = new HashSet<LogFileIdentity>();
                for (var index = 0; index < sources.Length; index++)
                {
                    cancellation.ThrowIfCancellationRequested();
                    var source = sources[index];
                    var text = index == 0 ? null : selected[index - 1].Text;
                    if (captured is not null)
                    {
                        var image = captured[index]; source.Path = image.Path; source.Name = image.Name;
                        source.Reader = new ImportLogReader(image.Image.OpenReader(), image.Encoding, image.DetectBom) { RequireCompleteRecords = index != 0 };
                        totalBytes = checked(totalBytes + source.Reader.TotalBytes); continue;
                    }
                    if (index == 0) source.Path = path;
                    else
                    {
                        var name = selected[index - 1].Name;
                        var match = catalog.SingleOrDefault(item => string.Equals(item.Name, name, StringComparison.OrdinalIgnoreCase))
                            ?? throw new FileNotFoundException("Selected companion is missing: " + name);
                        source.Path = match.Path;
                        source.Name = match.Name;
                        RejectLinks(source.Path);
                    }
                    var stream = new FileStream(source.Path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete,
                        4096, FileOptions.Asynchronous | FileOptions.SequentialScan);
                    try
                    {
                        var identity = LogFileIdentity.Read(stream.SafeFileHandle);
                        if (!identities.Add(identity)) throw new InvalidDataException("Import sources alias the same file");
                        if (index != 0)
                        {
                            RejectLinks(source.Path);
                            using var check = File.OpenHandle(source.Path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
                            if (LogFileIdentity.Read(check) != identity) throw new IOException("Companion changed during admission");
                        }
                        source.Reader = new ImportLogReader(stream, text?.Resolve() ?? encoding, text?.DetectBom ?? detectBom) { RequireCompleteRecords = index != 0 };
                    }
                    catch { stream.Dispose(); throw; }
                    totalBytes = checked(totalBytes + source.Reader.TotalBytes);
                }
                // Every source is open before any record is published; later renames do not redirect readers.
                var remaining = sources.Length;
                async Task Read(int index)
                {
                    var source = sources[index];
                    string? line;
                    try { line = await source.Reader!.ReadAsync(cancellation).ConfigureAwait(false); }
                    finally
                    {
                        if (source.Reader is { } reader)
                        {
                            var current = reader.BytesRead;
                            Interlocked.Add(ref bytesRead, current - source.Bytes);
                            source.Bytes = current;
                        }
                    }
                    var incomplete = source.Reader!.DiscardedIncompleteTail;
                    if (line is null)
                    {
                        source.Reader!.Dispose(); source.Reader = null; remaining--;
                    }
                    await responses.Writer.WriteAsync((index, line, incomplete), cancellation).ConfigureAwait(false);
                }
                // Do not decode companions for an empty/blank main source. The owner opens their gate after a meaningful main head.
                await Read(0).ConfigureAwait(false);
                while (remaining != 0 && await requests.Reader.WaitToReadAsync(cancellation).ConfigureAwait(false))
                    while (requests.Reader.TryRead(out var command))
                    {
                        if (command < 0)
                        {
                            var source = sources[-command - 1];
                            source.Reader!.Dispose(); source.Reader = null; remaining--;
                        }
                        else await Read(command).ConfigureAwait(false);
                    }
            }
            catch (OperationCanceledException) when (cancellation.IsCancellationRequested) { }
            catch (Exception error) { Failure = error; }
            finally
            {
                foreach (var source in sources)
                    try { source.Reader?.Dispose(); }
                    catch (Exception error) { Failure = Failure is null ? error : new AggregateException(Failure, error); }
                responses.Writer.TryComplete(Failure);
            }
        });

        private static void RejectLinks(string path)
        {
            if ((File.GetAttributes(System.IO.Path.GetDirectoryName(path)!) & FileAttributes.ReparsePoint) != 0 ||
                (File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0)
                throw new IOException("Selected companion cannot use a linked source or directory");
        }

        internal bool TryNext(FormActMain.DateTimeLogParser timestamp, out ImportSourceRecord record)
        {
            while (responses.Reader.TryRead(out var response))
            {
                var source = sources[response.Source];
                if (response.Line is null)
                {
                    source.Ended = true;
                    if (response.Incomplete) warning("Companion " + source.Name + " excluded an incomplete EOF record");
                }
                else
                {
                    var time = response.Line.Length == 0 ? DateTime.MinValue : timestamp(response.Line);
                    headCharacters = checked(headCharacters + response.Line.Length);
                    if (headCharacters > 32 * 1024 * 1024) throw new InvalidDataException("Companion import heads exceed 64 MiB of UTF-16 text");
                    source.Head = new(response.Line, time, source.Path, source.Name, response.Source);
                    if (response.Source == 0 && response.Line.Length != 0)
                    {
                        hasMainRecord = true;
                        if (time != DateTime.MinValue) mainWatermark = time;
                    }
                }
            }
            // Blank main records cannot authorize companion time; consume them until a real main head or EOF is known.
            if (sources[0].Head is { Line.Length: 0 } blank)
            {
                record = blank;
                return true;
            }
            if (!companionsStarted && hasMainRecord)
            {
                companionsStarted = true;
                for (var index = 1; index < sources.Length; index++)
                {
                    Request(index);
                }
            }
            if (sources[0].Ended && !companionsStarted)
                for (var index = 1; index < sources.Length; index++)
                    if (!sources[index].Ended)
                    {
                        sources[index].Ended = true;
                        Request(-index - 1);
                    }
            if (sources.Any(source => !source.Ended && source.Head is null))
            {
                if (responses.Reader.Completion.IsCompleted) responses.Reader.Completion.GetAwaiter().GetResult();
                record = default;
                return false;
            }
            ImportSourceRecord? best = null;
            foreach (var source in sources)
                if (source.Head is { } head && (best is null || head.Time < best.Value.Time)) best = head;
            record = best.GetValueOrDefault();
            if (best is not null && record.Source != 0 && sources[0].Ended && record.Time != DateTime.MinValue && record.Time > mainWatermark)
                record = record with { BeyondMain = true };
            return best is not null;
        }

        internal void Consume(int index, bool retire)
        {
            var source = sources[index];
            headCharacters -= source.Head!.Value.Line.Length;
            source.Head = null;
            if (retire) source.Ended = true;
            Request(retire ? -index - 1 : index);
        }
        private void Request(int command)
        {
            if (!requests.Writer.TryWrite(command)) throw new InvalidOperationException("Companion read request bound was exceeded");
        }

    }
}
