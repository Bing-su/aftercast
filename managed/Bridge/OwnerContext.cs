using System.Diagnostics;

namespace Aftercast.Bridge;

// The caller pumps this queue on its owner thread; dispatch never creates a substitute UI thread.
internal sealed class OwnerContext : SynchronizationContext, IDisposable
{
    private sealed record Work(SendOrPostCallback Callback, object? State, TaskCompletionSource<object?>? Completion);
    private readonly int ownerThread = Environment.CurrentManagedThreadId;
    private readonly object gate = new();
    private readonly Queue<Work> pending = new();
    private readonly Action<Exception> onPostError;
    private bool closed;
    private int dispatchDepth;
    internal bool IsDispatching => Environment.CurrentManagedThreadId == ownerThread && dispatchDepth > 0;

    internal OwnerContext(Action<Exception> onPostError)
    {
        this.onPostError = onPostError ?? throw new ArgumentNullException(nameof(onPostError));
    }

    private void RequireOwner()
    {
        if (Environment.CurrentManagedThreadId != ownerThread)
            throw new InvalidOperationException("The host context must be operated on its owner thread");
    }

    internal T Run<T>(Func<T> action)
    {
        ArgumentNullException.ThrowIfNull(action);
        RequireOwner();
        lock (gate) ObjectDisposedException.ThrowIf(closed, this);
        var previous = Current;
        try { SetSynchronizationContext(this); return action(); }
        finally { SetSynchronizationContext(previous); }
    }

    private void Enqueue(Work work)
    {
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(closed, this);
            // Bound pending plugin work and report overload to its producer instead of losing callbacks.
            if (pending.Count == 256) throw new InvalidOperationException("The host context queue is full");
            pending.Enqueue(work);
        }
    }

    public override void Send(SendOrPostCallback callback, object? state)
    {
        ArgumentNullException.ThrowIfNull(callback);
        if (Environment.CurrentManagedThreadId == ownerThread)
        {
            Run(() => { callback(state); return 0; });
            return;
        }
        var completion = new TaskCompletionSource<object?>(TaskCreationOptions.RunContinuationsAsynchronously);
        Enqueue(new(callback, state, completion));
        // Awaiter propagation preserves the callback exception rather than exposing an AggregateException.
        completion.Task.GetAwaiter().GetResult();
    }

    public override void Post(SendOrPostCallback callback, object? state)
    {
        ArgumentNullException.ThrowIfNull(callback);
        Enqueue(new(callback, state, null));
    }

    internal int Pump()
    {
        RequireOwner();
        var started = Stopwatch.GetTimestamp();
        var processed = 0;
        while (processed < 64 && Stopwatch.GetElapsedTime(started) < TimeSpan.FromMilliseconds(5))
        {
            Work work;
            lock (gate)
            {
                if (closed || pending.Count == 0) break;
                work = pending.Dequeue();
            }
            // Run arbitrary plugin code outside the queue gate, including callbacks that dispose this context.
            try
            {
                dispatchDepth++;
                try { Run(() => { work.Callback(work.State); return 0; }); }
                finally { dispatchDepth--; }
                work.Completion?.TrySetResult(null);
            }
            catch (Exception error)
            {
                if (work.Completion is not null) work.Completion.TrySetException(error);
                else onPostError(error);
            }
            processed++;
        }
        return processed;
    }

    public void Dispose()
    {
        RequireOwner();
        lock (gate)
        {
            if (closed) return;
            closed = true;
            while (pending.TryDequeue(out var work))
                work.Completion?.TrySetException(new ObjectDisposedException(nameof(OwnerContext)));
        }
    }
}
