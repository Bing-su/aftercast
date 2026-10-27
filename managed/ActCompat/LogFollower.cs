using System.Text;
using Aftercast.ActCompat;

namespace Advanced_Combat_Tracker;

// Follow only appended complete lines; decoding state belongs to this single reader thread.
internal sealed class LogFollower : LogReader
{
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
    private readonly Action? imageChanged;
    private readonly bool detectBom;
    private long observedPosition, imageVersion = 1, dropped;
    private string lastDiagnostic = "";
    private long InitialPosition { get; }
    internal override bool IsAlive => worker.IsAlive;
    internal override bool CallbackActive { get { lock (gate) return callbackActive; } }

    internal LogFollower(string path, Action<string> onLine, Action<Exception> onError, Func<bool> paused)
        : this(path, onLine, onError, paused, false) { }

    internal LogFollower(string path, Action<string> onLine, Action<Exception> onError, Func<bool> paused, bool initialSnapshot)
        : this(path, onLine, onError, paused, initialSnapshot, new ImportTextOptions().Resolve()) { }
    internal LogFollower(string path, Action<string> onLine, Action<Exception> onError, Func<bool> paused, bool initialSnapshot, Encoding encoding)
        : this(path, onLine, onError, paused, initialSnapshot, encoding, null) { }
    internal LogFollower(string path, Action<string> onLine, Action<Exception> onError, Func<bool> paused, bool initialSnapshot, Encoding encoding, Action? imageChanged)
        : this(path, onLine, onError, paused, initialSnapshot, encoding, imageChanged, true) { }
    internal LogFollower(string path, Action<string> onLine, Action<Exception> onError, Func<bool> paused, bool initialSnapshot, Encoding encoding, Action? imageChanged, bool detectBom)
    {
        this.detectBom = detectBom;
        this.imageChanged = imageChanged;
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
            observedPosition = InitialPosition;
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
        using var decoder = new LiveLogDecoder(selectedEncoding, InitialPosition, discardInitialContinuation, initialCr, detectBom);
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
                    Interlocked.Exchange(ref observedPosition, 0);
                    Interlocked.Increment(ref imageVersion);
                    imageChanged?.Invoke();
                }
                var count = stream.Read(bytes, 0, bytes.Length);
                Interlocked.Exchange(ref observedPosition, stream.Position);
                if (count == 0)
                {
                    // Drain the old complete tail before switching; absence during rename/create is temporary.
                    if (ReopenReplacement())
                    {
                        if (decoder.HasPending) Report(new InvalidDataException("Log replacement discarded an incomplete source record"));
                        decoder.Reset();
                        Interlocked.Exchange(ref observedPosition, 0);
                        Interlocked.Increment(ref imageVersion);
                        imageChanged?.Invoke();
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

    internal override string? ReadInitialZone(System.Text.RegularExpressions.Regex? regex, Func<bool> canceled)
    {
        ArgumentNullException.ThrowIfNull(canceled);
        if (regex is null) return null;
        return InitialZoneReader.Find(InitialPosition, ReadInitialBytes, regex, canceled, Report);
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

    internal override void CompleteInitialSnapshot()
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
            lock (gate) { stream = candidate; identity = nextIdentity; }
            candidate = null;
            previous.Dispose();
            return true;
        }
        finally { candidate?.Dispose(); }
    }

    internal string SourcePath => path;
    internal long ImageVersion => Interlocked.Read(ref imageVersion);
    internal long Dropped => Interlocked.Read(ref dropped);
    internal string LastDiagnostic => Volatile.Read(ref lastDiagnostic);
    internal LogFileIdentity Identity { get { lock (gate) return identity; } }
    internal bool IsCurrentImage()
    {
        FileStream candidate;
        try { candidate = Open(); }
        catch (FileNotFoundException) { return true; }
        catch (DirectoryNotFoundException) { return true; }
        using (candidate)
            return LogFileIdentity.Read(candidate.SafeFileHandle) == Identity && candidate.Length >= Interlocked.Read(ref observedPosition);
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
        Volatile.Write(ref lastDiagnostic, error.Message);
        if (error is DecoderFallbackException or InvalidDataException) Interlocked.Increment(ref dropped);
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

    internal override void Suspend(Action? pump)
    {
        RequestSuspension();
        try { while (!callbackIdle.Wait(10)) pump?.Invoke(); }
        catch { Resume(); throw; }
    }

    // Reserve suspension without joining a callback, e.g. during an asynchronous import's prepare phase.
    internal override void RequestSuspension()
    {
        lock (gate)
        {
            if (Thread.CurrentThread == worker && callbackActive)
                throw new InvalidOperationException("A log callback cannot wait for its own completion");
            suspensionDepth++;
        }
    }

    internal override void Resume()
    {
        lock (gate)
        {
            if (suspensionDepth > 0) suspensionDepth--;
        }
    }

    public override void Dispose() => Dispose(null);

    internal override void Dispose(Action? pump)
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
