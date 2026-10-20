using System.Text;

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
    private Exception? reportingFailure;
    internal long InitialPosition { get; }
    internal bool IsAlive => worker.IsAlive;
    internal bool CallbackActive { get { lock (gate) return callbackActive; } }

    internal LogFollower(string path, Action<string> onLine, Action<Exception> onError, Func<bool> paused)
        : this(path, onLine, onError, paused, false) { }

    internal LogFollower(string path, Action<string> onLine, Action<Exception> onError, Func<bool> paused, bool initialSnapshot)
    {
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
                // A suffix appended to a pre-existing partial record is historical input, not a new complete live record.
                var count = InitialPosition == 3 ? 3 : 1;
                Span<byte> boundary = stackalloc byte[count];
                if (RandomAccess.Read(opened.SafeFileHandle, boundary, InitialPosition - count) != count)
                    throw new EndOfStreamException("Log was truncated while checking its initial record boundary");
                // A BOM-only image has encoding metadata but no historical partial record to discard.
                discardInitialContinuation = boundary[^1] != (byte)'\n' &&
                    !(InitialPosition == 3 && boundary.SequenceEqual(new byte[] { 0xef, 0xbb, 0xbf }));
            }
            stream = opened;
            worker = new Thread(Read) { IsBackground = true, Name = "Bxivact log follower" };
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
        var encoding = new UTF8Encoding(false, true);
        using var pending = new MemoryStream();
        var discarding = discardInitialContinuation;
        var firstRecord = InitialPosition == 0;
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
                    // Truncation discards an incomplete record, including any split UTF-8 character.
                    stream.Seek(0, SeekOrigin.Begin);
                    pending.SetLength(0);
                    discarding = false;
                    firstRecord = true;
                }
                var count = stream.Read(bytes, 0, bytes.Length);
                if (count == 0)
                {
                    // Drain the old complete tail before switching; absence during rename/create is temporary.
                    if (ReopenReplacement())
                    {
                        if (pending.Length > 0) Report(new InvalidDataException("Log replacement discarded an incomplete UTF-8 record"));
                        pending.SetLength(0);
                        discarding = false;
                        firstRecord = true;
                        continue;
                    }
                    stopped.WaitOne(10);
                    continue;
                }
                var remaining = bytes.AsSpan(0, count);
                while (!remaining.IsEmpty)
                {
                    var newline = remaining.IndexOf((byte)'\n');
                    var fragment = newline < 0 ? remaining : remaining[..newline];
                    if (!discarding)
                    {
                        if (pending.Length + fragment.Length > MaximumLineLength)
                        {
                            // Bound malformed input while preserving all later complete records.
                            pending.SetLength(0);
                            discarding = true;
                            Report(new InvalidDataException($"Pending log line exceeds {MaximumLineLength} UTF-8 bytes"));
                        }
                        else pending.Write(fragment);
                    }
                    if (newline < 0) break;
                    if (!discarding)
                    {
                        try
                        {
                            var content = pending.GetBuffer().AsSpan(0, (int)pending.Length);
                            if (!content.IsEmpty && content[^1] == '\r') content = content[..^1];
                            var line = encoding.GetString(content);
                            if (firstRecord && line.StartsWith('\uFEFF')) line = line[1..];
                            if (!Deliver(line)) return;
                        }
                        catch (Exception error) { Report(error); }
                    }
                    pending.SetLength(0);
                    discarding = false;
                    firstRecord = false;
                    remaining = remaining[(newline + 1)..];
                }
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

    private FileStream Open() => new(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);

    internal bool ReadInitialBytes(byte[] destination, int count, long offset)
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
        lock (gate)
        {
            if (Thread.CurrentThread == worker && callbackActive)
                throw new InvalidOperationException("A log callback cannot wait for its own completion");
            suspensionDepth++;
        }
        try
        {
            while (!callbackIdle.Wait(10)) pump?.Invoke();
        }
        catch
        {
            Resume();
            throw;
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
