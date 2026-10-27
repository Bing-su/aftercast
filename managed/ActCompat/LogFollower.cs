using System.Text;
using Aftercast.ActCompat;
using System.Runtime.InteropServices;

namespace Advanced_Combat_Tracker;

// Follow only appended complete lines; decoding state belongs to this single reader thread.
internal sealed class LogFollower : IDisposable
{
    private const int MaximumLineLength = 8 * 1024 * 1024;
    private readonly object gate = new();
    private FileStream stream;
    private readonly string path;
    private LogFileIdentity identity;
    private readonly ManualResetEvent stopped;
    private readonly Thread worker;
    private readonly Action<string> onLine;
    private readonly Action<Exception> onError;
    private readonly Func<bool> paused;
    private readonly ManualResetEventSlim callbackIdle = new(initialState: true, spinCount: 0);
    private bool callbackActive;
    private int suspensionDepth;
    private bool disposed;
    private bool initialSnapshotPending;
    private readonly bool discardInitialContinuation;
    private readonly bool initialCr;
    private readonly Encoding selectedEncoding;
    private Exception? reportingFailure;
    private long InitialPosition { get; }
    internal bool IsAlive => worker.IsAlive;
    internal bool CallbackActive { get { lock (gate) return callbackActive; } }

    internal LogFollower(string path, Action<string> onLine, Action<Exception> onError, Func<bool> paused)
        : this(path, onLine, onError, paused, false) { }

    internal LogFollower(string path, Action<string> onLine, Action<Exception> onError, Func<bool> paused, bool initialSnapshot)
        : this(path, onLine, onError, paused, initialSnapshot, new ImportTextOptions().Resolve()) { }
    internal LogFollower(string path, Action<string> onLine, Action<Exception> onError, Func<bool> paused, bool initialSnapshot, Encoding encoding)
    {
        selectedEncoding = ImportTextOptions.Validate(encoding);
        ArgumentNullException.ThrowIfNull(onLine);
        ArgumentNullException.ThrowIfNull(onError);
        ArgumentNullException.ThrowIfNull(paused);
        this.onLine = onLine;
        this.onError = onError;
        this.paused = paused;
        initialSnapshotPending = initialSnapshot;
        this.path = Path.GetFullPath(path);
        stopped = new(false);
        FileStream? opened = null;
        try
        {
            opened = Open();
            identity = LogFileIdentity.Read(opened.SafeFileHandle);
            InitialPosition = opened.Seek(0, SeekOrigin.End);
            if (InitialPosition > 0)
            {
                var width = LiveLogDecoder.Width(selectedEncoding);
                var preamble = selectedEncoding.GetPreamble();
                Span<byte> boundary = stackalloc byte[4];
                var completeUnit = InitialPosition % width == 0 && InitialPosition >= width;
                if (completeUnit && RandomAccess.Read(opened.SafeFileHandle, boundary[..width], InitialPosition - width) != width)
                    throw new EndOfStreamException("Log was truncated while checking its initial record boundary");
                initialCr = completeUnit && LiveLogDecoder.IsControl(boundary[..width], selectedEncoding, (byte)'\r');
                var terminated = completeUnit && (initialCr || LiveLogDecoder.IsControl(boundary[..width], selectedEncoding, (byte)'\n'));
                var bomOnly = InitialPosition == preamble.Length && preamble.Length != 0 &&
                    RandomAccess.Read(opened.SafeFileHandle, boundary[..preamble.Length], 0) == preamble.Length && boundary[..preamble.Length].SequenceEqual(preamble);
                discardInitialContinuation = !terminated && !bomOnly;
            }
            stream = opened;
            worker = new Thread(Read) { IsBackground = true, Name = "Aftercast log follower" };
            worker.Start();
        }
        catch
        {
            opened?.Dispose();
            stopped.Dispose();
            throw;
        }
    }

    private void Read()
    {
        var bytes = new byte[4096];
        using var decoder = new LiveLogDecoder(selectedEncoding, InitialPosition, discardInitialContinuation, initialCr);
        try
        {
            while (!stopped.WaitOne(0))
            {
                bool isPaused;
                lock (gate) { if (disposed) return; isPaused = suspensionDepth > 0 || initialSnapshotPending; }
                isPaused |= paused();
                if (isPaused) { stopped.WaitOne(10); continue; }
                if (stream.Length < stream.Position)
                {
                    // A new image starts with fresh prefix/alignment state; no partial character survives truncation.
                    if (decoder.HasPending) Report(new InvalidDataException("Log truncation discarded an incomplete source record"));
                    stream.Seek(0, SeekOrigin.Begin);
                    decoder.Reset();
                }
                var count = stream.Read(bytes, 0, bytes.Length);
                if (count == 0)
                {
                    // Drain the old complete tail before switching; absence during rename/create is temporary.
                    if (ReopenReplacement())
                    {
                        if (decoder.HasPending) Report(new InvalidDataException("Log replacement discarded an incomplete source record"));
                        decoder.Reset();
                        continue;
                    }
                    stopped.WaitOne(10);
                    continue;
                }
                if (!decoder.Feed(bytes.AsSpan(0, count), Deliver, Report)) return;
            }
        }
        catch (Exception error) { Report(error); }
        finally
        {
            // Exclude new initial reads before closing the owned handle; no callback runs under this gate.
            lock (gate) initialSnapshotPending = false;
            try { stream.Dispose(); }
            catch (Exception error) { Report(error); }
            lock (gate)
            {
                disposed = true;
                stopped.Dispose();
            }
        }
    }

    internal string? ReadInitialZone(System.Text.RegularExpressions.Regex? regex, Func<bool> canceled)
    {
        ArgumentNullException.ThrowIfNull(canceled);
        if (regex is null) return null;
        // Search complete records backwards so large files do not require a full forward replay.
        var position = InitialPosition;
        var bytes = new byte[4096];
        var reverseLine = new List<byte>();
        var encoding = new UTF8Encoding(false, true);
        var skipTail = true;
        var oversized = false;
        string? MatchLine(bool atStart = false)
        {
            if (oversized) return null;
            reverseLine.Reverse();
            try
            {
                var text = encoding.GetString(CollectionsMarshal.AsSpan(reverseLine)).TrimEnd('\r');
                if (atStart && text.StartsWith('\uFEFF')) text = text[1..];
                var match = regex.Match(text);
                return match.Success && match.Groups["zone"].Success ? match.Groups["zone"].Value : null;
            }
            catch (DecoderFallbackException error)
            {
                Report(new InvalidDataException("Skipping invalid UTF-8 zone snapshot record", error));
                return null;
            }
        }
        while (position > 0)
        {
            // A large history search must not delay shutdown or a newer file selection.
            if (canceled()) return null;
            var count = (int)Math.Min(position, bytes.Length);
            position -= count;
            if (!ReadInitialBytes(bytes, count, position)) return null;
            for (var index = count - 1; index >= 0; index--)
            {
                if (bytes[index] == (byte)'\n')
                {
                    if (!skipTail && MatchLine() is { } zone) return zone;
                    skipTail = false;
                    reverseLine.Clear();
                    oversized = false;
                }
                else if (!skipTail && !oversized)
                {
                    if (reverseLine.Count == MaximumLineLength)
                    {
                        oversized = true;
                        reverseLine.Clear();
                        Report(new InvalidDataException("Zone snapshot record exceeds 8 MiB"));
                    }
                    else reverseLine.Add(bytes[index]);
                }
            }
        }
        return !skipTail ? MatchLine(atStart: true) : null;
    }

    private FileStream Open() => new(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);

    private bool ReadInitialBytes(byte[] destination, int count, long offset)
    {
        ArgumentNullException.ThrowIfNull(destination);
        if (count < 0 || count > destination.Length || offset < 0 || offset > InitialPosition - count)
            throw new ArgumentOutOfRangeException(nameof(offset));
        lock (gate)
        {
            if (disposed) return false;
            if (!initialSnapshotPending) throw new InvalidOperationException("Initial log snapshot is no longer available");
            // Positional reads use the same identity/offset captured at open and never move the worker's stream position.
            var read = 0;
            while (read < count)
            {
                var received = RandomAccess.Read(stream.SafeFileHandle, destination.AsSpan(read, count - read), offset + read);
                if (received == 0) throw new EndOfStreamException("Log was truncated during its initial zone snapshot");
                read += received;
            }
            return true;
        }
    }

    internal void CompleteInitialSnapshot()
    {
        lock (gate) initialSnapshotPending = false;
    }

    private bool ReopenReplacement()
    {
        FileStream? candidate;
        try { candidate = Open(); }
        catch (FileNotFoundException) { return false; }
        catch (DirectoryNotFoundException) { return false; }
        try
        {
            var nextIdentity = LogFileIdentity.Read(candidate.SafeFileHandle);
            if (nextIdentity == identity) return false;
            var previous = stream;
            stream = candidate;
            candidate = null;
            identity = nextIdentity;
            previous.Dispose();
            return true;
        }
        finally { candidate?.Dispose(); }
    }

    private bool Deliver(string line)
    {
        while (true)
        {
            var isPaused = paused();
            lock (gate)
            {
                if (disposed) return false;
                if (!isPaused && suspensionDepth == 0)
                {
                    callbackActive = true;
                    callbackIdle.Reset();
                    break;
                }
            }
            // Keep this complete line pending until delivery is resumed or the follower is stopped.
            stopped.WaitOne(10);
        }
        try { onLine(line); }
        catch (Exception error) { Report(error); }
        finally
        {
            lock (gate)
            {
                callbackActive = false;
                callbackIdle.Set();
            }
        }
        lock (gate) return !disposed;
    }

    private void Report(Exception error)
    {
        lock (gate) { if (disposed) return; }
        try { onError(error); }
        catch (Exception callbackError)
        {
            lock (gate)
            {
                // Surface an unusable error sink from Dispose rather than losing it on a background thread.
                reportingFailure = new AggregateException(error, callbackError);
                disposed = true;
                stopped.Set();
            }
        }
    }

    internal void Suspend(Action? pump)
    {
        RequestSuspension();
        try { while (!callbackIdle.Wait(10)) pump?.Invoke(); }
        catch { Resume(); throw; }
    }

    // Reserve suspension without joining a callback, e.g. during an asynchronous import's prepare phase.
    internal void RequestSuspension()
    {
        lock (gate)
        {
            if (Thread.CurrentThread == worker && callbackActive)
                throw new InvalidOperationException("A log callback cannot wait for its own completion");
            suspensionDepth++;
        }
    }

    internal void Resume()
    {
        lock (gate)
        {
            if (suspensionDepth > 0) suspensionDepth--;
        }
    }

    public void Dispose() => Dispose(null);

    internal void Dispose(Action? pump)
    {
        lock (gate)
        {
            if (!disposed)
            {
                disposed = true;
                stopped.Set();
            }
        }
        // A callback may dispose its own follower; cancellation then ends the reader after that callback.
        if (Thread.CurrentThread != worker)
            while (!worker.Join(10)) pump?.Invoke();
        Exception? failure;
        lock (gate) { failure = reportingFailure; }
        if (failure is not null) throw new AggregateException("Log follower error callback failed", failure);
    }
}
