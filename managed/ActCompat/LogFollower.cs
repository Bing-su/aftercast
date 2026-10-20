using System.Text;

namespace Advanced_Combat_Tracker;

// Follow only appended complete lines; decoding state belongs to this single reader thread.
// shortcut: file replacement keeps the original open file; add identity checks and reopening for log rotation.
internal sealed class LogFollower : IDisposable
{
    private const int MaximumLineLength = 8 * 1024 * 1024;
    private readonly object gate = new();
    private readonly FileStream stream;
    private readonly ManualResetEvent stopped;
    private readonly Thread worker;
    private readonly Action<string> onLine;
    private readonly Action<Exception> onError;
    private readonly Func<bool> paused;
    private readonly ManualResetEventSlim callbackIdle = new(initialState: true, spinCount: 0);
    private bool callbackActive;
    private int suspensionDepth;
    private bool disposed;
    private Exception? reportingFailure;
    internal long InitialPosition { get; }
    internal bool CallbackActive { get { lock (gate) return callbackActive; } }

    internal LogFollower(string path, Action<string> onLine, Action<Exception> onError, Func<bool> paused)
    {
        ArgumentNullException.ThrowIfNull(onLine);
        ArgumentNullException.ThrowIfNull(onError);
        ArgumentNullException.ThrowIfNull(paused);
        this.onLine = onLine;
        this.onError = onError;
        this.paused = paused;
        stopped = new(false);
        FileStream? opened = null;
        try
        {
            opened = new(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
            InitialPosition = opened.Seek(0, SeekOrigin.End);
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
        var discarding = false;
        try
        {
            while (!stopped.WaitOne(0))
            {
                bool isPaused;
                lock (gate) { if (disposed) return; isPaused = suspensionDepth > 0; }
                isPaused |= paused();
                if (isPaused) { stopped.WaitOne(10); continue; }
                if (stream.Length < stream.Position)
                {
                    // Truncation discards an incomplete record, including any split UTF-8 character.
                    stream.Seek(0, SeekOrigin.Begin);
                    pending.SetLength(0);
                    discarding = false;
                }
                var count = stream.Read(bytes, 0, bytes.Length);
                if (count == 0) { stopped.WaitOne(10); continue; }
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
                            if (!Deliver(encoding.GetString(content))) return;
                        }
                        catch (Exception error) { Report(error); }
                    }
                    pending.SetLength(0);
                    discarding = false;
                    remaining = remaining[(newline + 1)..];
                }
            }
        }
        catch (Exception error) { Report(error); }
        finally
        {
            try { stream.Dispose(); }
            catch (Exception error) { Report(error); }
            lock (gate)
            {
                disposed = true;
                stopped.Dispose();
            }
        }
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
