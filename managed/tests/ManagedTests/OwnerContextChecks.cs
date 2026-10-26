using System.Collections.Concurrent;
using Aftercast.Bridge;

internal static class OwnerContextChecks
{
    private static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception("Owner context check failed: " + message);
    }

    internal static void Run()
    {
        DispatchAndFailures();
        CapacityAndPumpBounds();
        ShutdownReleasesSend();
        OperationContinuesUnderSaturation();
        Console.WriteLine("Owner context dispatch, exception identity, bounded queue/pump and shutdown checks passed.");
    }

    private static void OperationContinuesUnderSaturation()
    {
        var errors = new ConcurrentQueue<Exception>();
        using var context = new OwnerContext(errors.Enqueue);
        var owner = Environment.CurrentManagedThreadId;
        var released = new TaskCompletionSource<object?>();
        var pluginCalls = 0;
        var operation = context.RunOperation(async () =>
        {
            await released.Task;
            Check(Environment.CurrentManagedThreadId == owner, "operation resumed away from owner");
            Check(pluginCalls == 0, "plugin backlog ran before the reserved continuation");
        });
        for (var index = 0; index < 256; index++) context.Post(_ => pluginCalls++, null);
        // Complete on a worker while admission is full, before allowing the owner to drain any callbacks.
        Task.Run(() => released.SetResult(null)).GetAwaiter().GetResult();
        Check(!operation.IsCompleted, "operation bypassed owner dispatch");
        context.Pump();
        Check(operation.IsCompletedSuccessfully && errors.IsEmpty, "operation was stranded by plugin saturation");
    }

    private static void DispatchAndFailures()
    {
        var errors = new ConcurrentQueue<Exception>();
        using var context = new OwnerContext(errors.Enqueue);
        var previous = SynchronizationContext.Current;
        var owner = Environment.CurrentManagedThreadId;
        Check(!context.IsDispatching, "new context was marked dispatching");
        Check(context.Run(() => ReferenceEquals(SynchronizationContext.Current, context)), "Run did not install context");
        Check(!context.IsDispatching, "ordinary Run was marked as queued dispatch");
        Check(ReferenceEquals(SynchronizationContext.Current, previous), "Run did not restore context");
        var callbackFailure = new InvalidOperationException("send callback");
        var postedFailure = new InvalidOperationException("post callback");
        var ownerCalls = 0;
        Exception? workerFailure = null;
        using var done = new ManualResetEventSlim();
        var worker = new Thread(() =>
        {
            try
            {
                context.Send(_ =>
                {
                    Check(Environment.CurrentManagedThreadId == owner && ReferenceEquals(SynchronizationContext.Current, context), "Send ran outside owner context");
                    Check(context.IsDispatching, "queued Send was not marked dispatching");
                    ownerCalls++;
                }, null);
                Check(Volatile.Read(ref ownerCalls) == 1, "Send returned before its callback completed");
                try { context.Send(_ => throw callbackFailure, null); throw new Exception("Send swallowed callback failure"); }
                catch (InvalidOperationException error) { Check(ReferenceEquals(error, callbackFailure), "Send replaced callback exception"); }
                context.Post(_ => throw postedFailure, null);
                context.Post(_ => ownerCalls++, null);
                try { context.Pump(); throw new Exception("Worker pumped owner callbacks"); }
                catch (InvalidOperationException) { }
            }
            catch (Exception error) { workerFailure = error; }
            finally { done.Set(); }
        })
        { IsBackground = true };
        worker.Start();
        var elapsed = System.Diagnostics.Stopwatch.StartNew();
        while ((!done.IsSet || errors.Count == 0 || ownerCalls != 2) && elapsed.Elapsed < TimeSpan.FromSeconds(10))
        {
            context.Pump();
            Thread.Yield();
        }
        Check(done.IsSet && worker.Join(TimeSpan.FromSeconds(1)) && workerFailure is null, "worker did not finish successfully");
        Check(!context.IsDispatching, "Pump did not restore dispatch state");
        Check(ownerCalls == 2 && errors.Count == 1 && errors.TryPeek(out var postedError) && ReferenceEquals(postedError, postedFailure), "Post failure prevented later callbacks or lost exception identity");
        try { context.Run<int>(() => throw callbackFailure); throw new Exception("Run swallowed callback failure"); }
        catch (InvalidOperationException error) { Check(ReferenceEquals(error, callbackFailure), "Run replaced exception"); }
        Check(ReferenceEquals(SynchronizationContext.Current, previous), "failed Run did not restore context");
    }

    private static void CapacityAndPumpBounds()
    {
        using var context = new OwnerContext(_ => throw new Exception("Unexpected posted callback failure"));
        var calls = 0;
        for (var index = 0; index < 256; index++) context.Post(_ => calls++, null);
        try { context.Post(_ => calls++, null); throw new Exception("Post exceeded queue capacity"); }
        catch (InvalidOperationException) { }
        Exception? sendFailure = null;
        var sender = new Thread(() =>
        {
            try { context.Send(_ => calls++, null); }
            catch (Exception error) { sendFailure = error; }
        })
        { IsBackground = true };
        sender.Start();
        Check(sender.Join(TimeSpan.FromSeconds(5)) && sendFailure is InvalidOperationException, "full queue blocked or accepted Send");
        var first = 0;
        Check(SpinWait.SpinUntil(() => (first = context.Pump()) != 0, TimeSpan.FromSeconds(5)), "queued callbacks were not pumped");
        Check(first > 0 && first <= 64 && calls == first, "Pump exceeded item budget or returned wrong count");
        Check(SpinWait.SpinUntil(() => { context.Pump(); return calls == 256; }, TimeSpan.FromSeconds(5)), "queue did not drain");
        Check(calls == 256, "overflow changed existing queued callbacks");
        context.Post(_ => calls++, null);
        Check(SpinWait.SpinUntil(() => { context.Pump(); return calls == 257; }, TimeSpan.FromSeconds(5)), "drained queue did not accept new work");
    }

    private static void ShutdownReleasesSend()
    {
        using var context = new OwnerContext(_ => throw new Exception("Unexpected posted callback failure"));
        var executed = false;
        Exception? failure = null;
        var worker = new Thread(() =>
        {
            try { context.Send(_ => executed = true, null); }
            catch (Exception error) { failure = error; }
        })
        { IsBackground = true };
        worker.Start();
        // This worker has no other blocking operation: waiting proves its synchronous Send is queued.
        Check(SpinWait.SpinUntil(() => (worker.ThreadState & ThreadState.WaitSleepJoin) != 0, TimeSpan.FromSeconds(5)), "Send did not wait for owner dispatch");
        context.Dispose();
        Check(worker.Join(TimeSpan.FromSeconds(5)) && failure is ObjectDisposedException && !executed, "shutdown stranded or executed a queued Send");
        Check(context.Pump() == 0, "closed context dispatched work");
        try { context.Post(_ => executed = true, null); throw new Exception("Closed context accepted Post"); }
        catch (ObjectDisposedException) { }
        try { context.Send(_ => executed = true, null); throw new Exception("Closed context accepted Send"); }
        catch (ObjectDisposedException) { }
        Check(!executed, "closed context executed a callback");
    }
}
