using System.Collections.Concurrent;
using System.Diagnostics;
using System.Text;
using System.Text.RegularExpressions;
using Aftercast.ActCompat;

namespace Aftercast.ActCompat
{
    public sealed record LiveLogSourceStatus(string Name, string Path, string State, int Queued,
        DateTime? PendingTime, long Generation, long Dropped, string LastError);
}

namespace Advanced_Combat_Tracker
{
    // Disk workers only frame bounded records; a single dispatcher owns timestamp and raw parser callbacks.
    internal sealed class LiveCompanionGroup : LogReader
    {
        private sealed record Queued(string Line, long Version);
        private sealed record Head(string Line, DateTime Time, long Version);
        private sealed class Source(string path, string name)
        {
            internal readonly string Path = path, Name = name;
            internal readonly BlockingCollection<Queued> Queue = new(1);
            internal LogFollower Reader = null!;
            internal Head? Head;
            internal long Version = 1, Dropped, WaitStarted;
            internal int Invalidating, QueuedCount, Waiting;
            internal string? Failure;
            internal string Error = "";
        }
        private readonly object gate = new();
        private readonly Source[] sources;
        private readonly Thread worker;
        private readonly CancellationTokenSource stop = new();
        private readonly CancellationToken token;
        private readonly ManualResetEventSlim idle = new(true);
        private readonly Func<string, DateTime> timestamp;
        private readonly Func<DateTime> clock;
        private readonly Func<bool> paused;
        private readonly Action<string, string, string, DateTime, Func<bool>> dispatch;
        private readonly Action<Exception> report;
        private readonly TimeSpan maxClockWait;
        private int stopped, suspensionDepth;
        private bool ready, callbackActive;
        private long bufferedCharacters, lastImageCheck;
        private Exception? reportingFailure;

        internal LiveCompanionGroup(string path, Encoding encoding, ImportCompanion[] selected, TimeSpan maxClockWait,
            Func<string, DateTime> timestamp, Func<DateTime> clock, Func<bool> paused,
            Action<string, string, string, DateTime, Func<bool>> dispatch, Action<Exception> report, bool mainDetectBom = true)
        {
            this.timestamp = timestamp; this.clock = clock; this.paused = paused;
            this.dispatch = dispatch; this.report = report; this.maxClockWait = maxClockWait;
            token = stop.Token;
            sources = [];
            try
            {
                var catalog = CompanionLogCatalog.Discover(path);
                var companions = selected.Select(selection => catalog.SingleOrDefault(source => string.Equals(source.Name, selection.Name, StringComparison.OrdinalIgnoreCase))
                    ?? throw new FileNotFoundException("Selected live companion is missing: " + selection.Name)).ToArray();
                sources = [new(path, ""), .. companions.Select(source => new Source(source.Path, source.Name))];
                var identities = new HashSet<LogFileIdentity>();
                for (var index = 0; index < sources.Length; index++)
                {
                    var source = sources[index];
                    var text = index == 0 ? null : selected[index - 1].Text;
                    if (index != 0) RejectLinks(source.Path);
                    source.Reader = new(source.Path, line => Enqueue(source, line), error => ReaderError(source, error), () => false,
                        true, text?.Resolve() ?? encoding, () => ImageChanged(source), index == 0 ? mainDetectBom : text?.DetectBom ?? true);
                    if (!identities.Add(source.Reader.Identity)) throw new InvalidDataException("Live sources alias the same file");
                    if (index != 0)
                    {
                        RejectLinks(source.Path);
                        if (!source.Reader.IsCurrentImage()) throw new IOException("Live companion changed during admission");
                    }
                }
                worker = new Thread(Run) { IsBackground = true, Name = "Aftercast live source dispatch" };
                worker.Start();
            }
            catch (Exception failure)
            {
                Stop();
                var cleanup = new List<Exception>();
                foreach (var source in sources)
                {
                    try { source.Reader?.Dispose(); } catch (Exception error) { cleanup.Add(error); }
                    source.Queue.Dispose();
                }
                stop.Dispose();
                idle.Dispose();
                if (cleanup.Count != 0) throw new AggregateException("Live source admission and cleanup failed", new[] { failure }.Concat(cleanup));
                throw;
            }
        }
        private static void RejectLinks(string path)
        {
            if ((File.GetAttributes(Path.GetDirectoryName(path)!) & FileAttributes.ReparsePoint) != 0 ||
                (File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0)
                throw new IOException("Live companion cannot use a linked source or directory");
        }
        internal override bool IsAlive => worker.IsAlive;
        internal override bool CallbackActive { get { lock (gate) return callbackActive; } }
        internal override int ResourceCost => sources.Length + 1;
        private bool Valid(Source source, long version) => !token.IsCancellationRequested && Volatile.Read(ref source.Failure) is null &&
            Volatile.Read(ref source.Invalidating) == 0 && Interlocked.Read(ref source.Version) == version;
        private void Enqueue(Source source, string line)
        {
            var record = new Queued(line, Interlocked.Read(ref source.Version));
            if (!Valid(source, record.Version)) { Interlocked.Increment(ref source.Dropped); return; }
            bool overBudget;
            lock (gate)
            {
                overBudget = bufferedCharacters + line.Length > 32 * 1024 * 1024;
                if (!overBudget) bufferedCharacters += line.Length;
            }
            if (overBudget)
            {
                Interlocked.Increment(ref source.Dropped);
                Fail(source, new InvalidDataException("Live source buffered text exceeds 64 MiB"));
                return;
            }
            try
            {
                while (Valid(source, record.Version))
                    if (source.Queue.TryAdd(record, 10, token)) { Interlocked.Increment(ref source.QueuedCount); return; }
            }
            catch (OperationCanceledException) when (token.IsCancellationRequested) { }
            Release(line);
            if (!token.IsCancellationRequested) Interlocked.Increment(ref source.Dropped);
        }
        private void Release(string line) { lock (gate) bufferedCharacters -= line.Length; }
        private void ImageChanged(Source source)
        {
            bool hadQueued;
            lock (gate)
            {
                hadQueued = Volatile.Read(ref source.Head) is not null || Volatile.Read(ref source.QueuedCount) != 0;
                Interlocked.Increment(ref source.Version);
                Volatile.Write(ref source.Invalidating, 0);
            }
            if (hadQueued) Report(new InvalidDataException("File generation changed; queued prior-generation records are excluded"), source);
        }
        private void ReaderError(Source source, Exception error)
        {
            // Framing diagnostics recover at the next complete record; a failed stream cannot keep dispatching.
            if (error is DecoderFallbackException or InvalidDataException) Report(error, source);
            else Fail(source, error);
        }
        private void Report(Exception error, Source source)
        {
            Volatile.Write(ref source.Error, error.Message);
            try { report(new IOException("Live source " + (source.Name.Length == 0 ? "main" : source.Name) + " (" + source.Path + "): " + error.Message, error)); }
            catch (Exception sinkError) { lock (gate) reportingFailure = new AggregateException(error, sinkError); Stop(); }
        }
        private void Fail(Source source, Exception error)
        {
            // Publish one immutable terminal cause instead of a flag followed by a separate diagnostic.
            // Readers can never observe failed + empty, and later recoverable reports cannot erase the cause.
            var message = error.Message;
            if (string.IsNullOrEmpty(message)) message = error.GetType().Name;
            if (Interlocked.CompareExchange(ref source.Failure, message, null) is not null) return;
            Report(error, source);
            if (source.Name.Length == 0) Stop();
            source.Reader?.Dispose();
        }
        private void Stop()
        {
            if (Interlocked.Exchange(ref stopped, 1) == 0) stop.Cancel();
        }
        private bool Enter()
        {
            var externallyPaused = paused();
            lock (gate)
            {
                if (token.IsCancellationRequested || !ready || externallyPaused || suspensionDepth != 0) return false;
                callbackActive = true; idle.Reset(); return true;
            }
        }
        private void Run()
        {
            try
            {
                while (!token.IsCancellationRequested)
                {
                    if (!Enter())
                    {
                        foreach (var source in sources) { source.WaitStarted = 0; Volatile.Write(ref source.Waiting, 0); }
                        token.WaitHandle.WaitOne(10); continue;
                    }
                    var delivered = 0;
                    try
                    {
                        var started = Stopwatch.GetTimestamp();
                        for (; delivered < 64 && Stopwatch.GetElapsedTime(started) < TimeSpan.FromMilliseconds(5) && !token.IsCancellationRequested; delivered++)
                        {
                            CheckImages();
                            foreach (var source in sources) LoadHead(source);
                            var available = clock();
                            var main = sources[0].Head;
                            if (main is not null && Valid(sources[0], main.Version) && main.Time > available) available = main.Time;
                            Source? chosen = null;
                            foreach (var source in sources)
                            {
                                var head = source.Head;
                                if (head is null || !Valid(source, head.Version)) continue;
                                if (source.Name.Length != 0 && head.Time != DateTime.MinValue && head.Time > available)
                                {
                                    Volatile.Write(ref source.Waiting, 1);
                                    if (source.WaitStarted == 0) source.WaitStarted = Stopwatch.GetTimestamp();
                                    else if (Stopwatch.GetElapsedTime(source.WaitStarted) >= maxClockWait)
                                        Fail(source, new TimeoutException("Companion clock wait exceeded " + maxClockWait));
                                    continue;
                                }
                                // Eligibility ends a continuous clock wait even when another ready head wins the tie.
                                source.WaitStarted = 0;
                                Volatile.Write(ref source.Waiting, 0);
                                if (chosen is null || head.Time < chosen.Head!.Time) chosen = source;
                            }
                            if (chosen is null) break;
                            var next = chosen.Head!;
                            chosen.Head = null; chosen.WaitStarted = 0; Volatile.Write(ref chosen.Waiting, 0);
                            try
                            {
                                if (Valid(chosen, next.Version))
                                    dispatch(chosen.Path, chosen.Name, next.Line, next.Time, () => Valid(chosen, next.Version));
                            }
                            catch (Exception error) { Interlocked.Increment(ref chosen.Dropped); Report(error, chosen); }
                            finally { Release(next.Line); }
                        }
                    }
                    finally { lock (gate) { callbackActive = false; idle.Set(); } }
                    if (delivered == 0) token.WaitHandle.WaitOne(10);
                }
            }
            catch (Exception error) { Report(error, sources[0]); Stop(); }
            finally
            {
                Stop();
                foreach (var source in sources)
                {
                    try { source.Reader.Dispose(); } catch (Exception error) { Report(error, source); }
                    if (source.Head is { } head) { source.Head = null; Release(head.Line); }
                    while (source.Queue.TryTake(out var queued)) Release(queued.Line);
                    Interlocked.Exchange(ref source.QueuedCount, 0);
                    source.Queue.Dispose();
                }
                stop.Dispose();
            }
        }
        private void CheckImages()
        {
            if (lastImageCheck != 0 && Stopwatch.GetElapsedTime(lastImageCheck) < TimeSpan.FromMilliseconds(100)) return;
            lastImageCheck = Stopwatch.GetTimestamp();
            foreach (var source in sources)
            {
                if (Volatile.Read(ref source.Failure) is not null || source.Head is null && Volatile.Read(ref source.QueuedCount) == 0) continue;
                var version = Interlocked.Read(ref source.Version);
                try
                {
                    if (source.Reader.IsCurrentImage()) continue;
                    var changed = false;
                    lock (gate)
                        if (version == Interlocked.Read(ref source.Version) && source.Invalidating == 0)
                        { Volatile.Write(ref source.Invalidating, 1); changed = true; }
                    if (changed) Report(new InvalidDataException("File image changed; queued old-generation records are invalidated"), source);
                }
                catch (Exception error) { Fail(source, error); }
            }
        }
        private void LoadHead(Source source)
        {
            if (source.Head is { } old && !Valid(source, old.Version))
            { source.Head = null; Release(old.Line); Interlocked.Increment(ref source.Dropped); }
            while (source.Head is null && source.Queue.TryTake(out var queued))
            {
                Interlocked.Decrement(ref source.QueuedCount);
                if (!Valid(source, queued.Version)) { Release(queued.Line); Interlocked.Increment(ref source.Dropped); continue; }
                try { source.Head = new(queued.Line, queued.Line.Length == 0 ? DateTime.MinValue : timestamp(queued.Line), queued.Version); }
                catch (Exception error) { Release(queued.Line); Interlocked.Increment(ref source.Dropped); Report(error, source); }
            }
        }
        internal LiveLogSourceStatus[] ReadStatus()
        {
            var isPaused = paused();
            return sources.Select(source =>
            {
                var failure = Volatile.Read(ref source.Failure);
                return new LiveLogSourceStatus(source.Name, source.Path,
                    failure is not null ? "failed" : Volatile.Read(ref stopped) != 0 ? "stopped" : isPaused ? "paused" :
                    source.Invalidating != 0 ? "changing" : source.Waiting != 0 ? "waiting_for_clock" : "following",
                    Math.Max(0, Volatile.Read(ref source.QueuedCount)), Volatile.Read(ref source.Head)?.Time,
                    Interlocked.Read(ref source.Version), Interlocked.Read(ref source.Dropped), failure ?? Volatile.Read(ref source.Error));
            }).ToArray();
        }
        internal override string? ReadInitialZone(Regex? regex, Func<bool> canceled) => sources[0].Reader.ReadInitialZone(regex, canceled);
        internal override void CompleteInitialSnapshot()
        {
            foreach (var source in sources) source.Reader.CompleteInitialSnapshot();
            lock (gate) ready = true;
        }
        internal override void RequestSuspension()
        {
            lock (gate)
            {
                if (Thread.CurrentThread == worker && callbackActive) throw new InvalidOperationException("A live source callback cannot wait for itself");
                suspensionDepth++;
            }
        }
        internal override void Suspend(Action? pump)
        {
            RequestSuspension();
            try { while (!idle.Wait(10)) pump?.Invoke(); }
            catch { Resume(); throw; }
        }
        internal override void Resume() { lock (gate) if (suspensionDepth != 0) suspensionDepth--; }
        public override void Dispose() => Dispose(null);
        internal override void Dispose(Action? pump)
        {
            Stop();
            if (Thread.CurrentThread != worker) while (!worker.Join(10)) pump?.Invoke();
            lock (gate) if (reportingFailure is { } failure) throw new AggregateException("Live source error sink failed", failure);
        }
    }
}
