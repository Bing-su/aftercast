using System.Collections.Concurrent;
using System.Windows.Forms;

internal static class ControlInvokeContracts
{
    private sealed class PumpContext : SynchronizationContext, IDisposable
    {
        private readonly BlockingCollection<(SendOrPostCallback Callback, object? State, ManualResetEventSlim Done)> pending = new();
        public override void Send(SendOrPostCallback callback, object? state)
        {
            using var done = new ManualResetEventSlim();
            pending.Add((callback, state, done));
            if (!done.Wait(TimeSpan.FromSeconds(10))) throw new TimeoutException("Owner thread did not pump Invoke");
        }
        internal void PumpOne(Action? beforeDispatch = null)
        {
            if (!pending.TryTake(out var item, TimeSpan.FromSeconds(10))) throw new TimeoutException("Invoke was not queued");
            try { beforeDispatch?.Invoke(); item.Callback(item.State); }
            finally { item.Done.Set(); }
        }
        public void Dispose() => pending.Dispose();
    }

    // Inheriting the inline base Send must not make a context a valid owner-thread dispatcher.
    private sealed class InlineContext : SynchronizationContext { }

    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Control Invoke contract check failed");
    }

    private static Control Create(SynchronizationContext context)
    {
        var previous = SynchronizationContext.Current;
        try { SynchronizationContext.SetSynchronizationContext(context); return new Control(); }
        finally { SynchronizationContext.SetSynchronizationContext(previous); }
    }

    public static void Run()
    {
        using var context = new PumpContext();
        using var control = Create(context);
        var ownerThread = Environment.CurrentManagedThreadId;
        var callbackError = new InvalidOperationException("owner callback failure");
        var wrapped = new System.Reflection.TargetInvocationException(callbackError);
        try { control.Invoke((Action)(() => throw wrapped)); throw new Exception("Callback exception swallowed"); }
        catch (System.Reflection.TargetInvocationException error) { Check(ReferenceEquals(error, wrapped)); }
        Exception? failure = null;
        var executed = 0;
        var worker = new Thread(() =>
        {
            try
            {
                Check(control.InvokeRequired);
                Check((int)control.Invoke((Func<int>)(() =>
                {
                    Check(Environment.CurrentManagedThreadId == ownerThread && !control.InvokeRequired);
                    executed++;
                    return 42;
                }))! == 42);
                try { control.Invoke((Action)(() => throw callbackError)); throw new Exception("Callback exception swallowed"); }
                catch (InvalidOperationException error) { Check(ReferenceEquals(error, callbackError)); }
                try { control.Invoke((Action)(() => executed++)); throw new Exception("Disposed control callback executed"); }
                catch (ObjectDisposedException) { }
            }
            catch (Exception error) { failure = error; }
        }) { IsBackground = true };
        worker.Start();
        context.PumpOne();
        context.PumpOne();
        context.PumpOne(control.Dispose);
        Check(worker.Join(TimeSpan.FromSeconds(10)) && failure is null && executed == 1);

        foreach (var unsupported in new SynchronizationContext[] { new SynchronizationContext(), new InlineContext() })
        {
            using var inline = Create(unsupported);
            failure = null;
            var rejected = false;
            worker = new Thread(() =>
            {
                try
                {
                    try { inline.Invoke((Action)(() => executed++)); }
                    catch (PlatformNotSupportedException) { rejected = true; }
                }
                catch (Exception error) { failure = error; }
            }) { IsBackground = true };
            worker.Start();
            Check(worker.Join(TimeSpan.FromSeconds(10)) && failure is null && rejected && executed == 1);
        }
        Console.WriteLine("Caller-owned context dispatch, owner-thread validation, callback exception identity and queued disposal checks passed.");
    }
}
