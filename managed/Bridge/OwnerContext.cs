using System.Diagnostics;

namespace Aftercast.Bridge;

// The host loop pumps this queue on its owner thread, preserving plugin control affinity.
internal sealed class OwnerContext : SynchronizationContext, IDisposable
{
    private sealed record Work(SendOrPostCallback Callback, object? State, TaskCompletionSource<object?>? Completion, bool Operation = false);
    private readonly int ownerThread = Environment.CurrentManagedThreadId;
    private readonly object gate = new();
    private readonly Queue<Work> pending = new();
    private readonly Queue<Work> continuations = new();
    private readonly SynchronizationContext operationContext;
    private readonly Action<Exception> onPostError;
    private bool closed;
    private int dispatchDepth;
    internal bool IsDispatching => Environment.CurrentManagedThreadId == ownerThread && dispatchDepth > 0;

    internal OwnerContext(Action<Exception> onPostError)
    {
        this.onPostError = onPostError ?? throw new ArgumentNullException(nameof(onPostError));
        operationContext = new OperationContext(this);
    }

    private void RequireOwner()
    {
        if (Environment.CurrentManagedThreadId != ownerThread)
            throw new InvalidOperationException("The host context must be operated on its owner thread");
    }

    internal T Run<T>(Func<T> action) => RunWithContext(action, this);

    // Only one library archive task is active; its continuations must survive plugin queue saturation.
    internal T RunOperation<T>(Func<T> action) => RunWithContext(action, operationContext);

    private T RunWithContext<T>(Func<T> action, SynchronizationContext context)
    {
        ArgumentNullException.ThrowIfNull(action);
        RequireOwner();
        lock (gate) ObjectDisposedException.ThrowIf(closed, this);
        var previous = Current;
        try { SetSynchronizationContext(context); return action(); }
        finally { SetSynchronizationContext(previous); }
    }

    private void Enqueue(Work work)
    {
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(closed, this);
            // Bound pending plugin work and report overload to its producer instead of losing callbacks.
            if (!work.Operation && pending.Count == 256) throw new InvalidOperationException("The host context queue is full");
            (work.Operation ? continuations : pending).Enqueue(work);
        }
    }

    private sealed class OperationContext(OwnerContext owner) : SynchronizationContext
    {
        public override void Post(SendOrPostCallback callback, object? state) => owner.Enqueue(new(callback, state, null, true));
        public override void Send(SendOrPostCallback callback, object? state) =>
            owner.Send(_ => owner.RunOperation(() => { callback(state); return 0; }), null);
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
                if (closed || !continuations.TryDequeue(out work!) && !pending.TryDequeue(out work!)) break;
            }
            // Run arbitrary plugin code outside the queue gate, including callbacks that dispose this context.
            try
            {
                dispatchDepth++;
                try
                {
                    if (work.Operation) RunOperation(() => { work.Callback(work.State); return 0; });
                    else Run(() => { work.Callback(work.State); return 0; });
                }
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
        if (OperatingSystem.IsWindows() && !closed)
        {
            // Native Invoke callbacks must carry the same reentrant-wait guard as queued Send callbacks.
            dispatchDepth++;
            try { Run(() => { PumpWindows(); return 0; }); }
            finally { dispatchDepth--; }
        }
        return processed;
    }

    // Keep native WinForms types out of the Linux JIT path.
    private static void PumpWindows() => System.Windows.Forms.Application.DoEvents();

    public void Dispose()
    {
        RequireOwner();
        lock (gate)
        {
            if (closed) return;
            closed = true;
            while (pending.TryDequeue(out var work))
                work.Completion?.TrySetException(new ObjectDisposedException(nameof(OwnerContext)));
            continuations.Clear();
        }
    }
}
