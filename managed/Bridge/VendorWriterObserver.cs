using System.Diagnostics;
using System.Reflection;

namespace Aftercast.Bridge;

// Observe the inspected writer; do not dequeue, append, retry or acknowledge its records.
internal sealed class VendorWriterObserver
{
    private readonly Queue<string> queue;
    private readonly object queueGate;
    private readonly Thread[] threads;
    private readonly object mediator;
    private readonly PropertyInfo callbackProperty;
    private readonly Action<DateTime, string>? previous;
    private readonly Action<DateTime, string> callback;
    private readonly Action<string> report;
    private long ioErrors, diagnosticFailures;

    internal VendorWriterObserver(object writer, object mediator, IEnumerable<Thread> producers, Action<string> report)
    {
        var type = writer.GetType();
        if (type.FullName != "FFXIV_ACT_Plugin.Logfile.LogOutput" ||
            type.Module.ModuleVersionId != new Guid("ff6af951-0fd5-413c-b114-d048ca6b23b4"))
            throw new InvalidOperationException("Unsupported vendor disk writer observation contract");
        queue = (Queue<string>)Field(writer, "_LogQueue");
        queueGate = Field(writer, "_LogQueueLock");
        threads = producers.Append(ThreadOf(writer)).Distinct().ToArray();
        this.mediator = mediator;
        this.report = report;
        callbackProperty = mediator.GetType().GetProperty("ProcessException")
            ?? throw new InvalidOperationException("Missing vendor exception callback");
        previous = (Action<DateTime, string>?)callbackProperty.GetValue(mediator);
        callback = OnException;
        callbackProperty.SetValue(mediator, callback);
    }

    internal static object Field(object instance, string name) =>
        instance.GetType().GetField(name, BindingFlags.NonPublic | BindingFlags.Instance)?.GetValue(instance)
        ?? throw new InvalidOperationException("Missing vendor writer lifecycle field: " + name);

    internal static Thread ThreadOf(object instance) =>
        (Thread)(instance.GetType().GetProperty("ScanThread")?.GetValue(instance)
        ?? throw new InvalidOperationException("Missing vendor scan thread"));

    private void OnException(DateTime timestamp, string message)
    {
        if (message.StartsWith("FFXIV_ACT_Plugin.LogOutput Error writing to log file.", StringComparison.Ordinal))
        {
            Interlocked.Increment(ref ioErrors);
            // Diagnostics cannot escape onto the vendor's writer thread or replace its UI callback.
            try { report(message); }
            catch { Interlocked.Increment(ref diagnosticFailures); }
        }
        previous?.Invoke(timestamp, message);
    }

    internal WriterStopObservation Finish(Action pump, TimeSpan? wait = null)
    {
        var elapsed = Stopwatch.StartNew();
        var timeout = wait ?? TimeSpan.FromSeconds(1);
        while (threads.Any(thread => thread.IsAlive) && elapsed.Elapsed < timeout)
        {
            pump();
            Thread.Sleep(10);
        }
        var active = threads.Where(thread => thread.IsAlive).Select(thread => thread.Name ?? "unnamed").ToArray();
        int pending;
        lock (queueGate) pending = queue.Count;
        // Never replace a callback installed later by plugin code. Live threads keep this observer
        // bound to the old Host; its worker must exit before another plugin can be constructed.
        if (active.Length == 0 && ReferenceEquals(callbackProperty.GetValue(mediator), callback))
            callbackProperty.SetValue(mediator, previous);
        return new(pending, Interlocked.Read(ref ioErrors), Interlocked.Read(ref diagnosticFailures), active);
    }
}

internal sealed record WriterStopObservation(int PendingRecords, long IoErrors, long DiagnosticFailures, string[] ActiveThreads)
{
    internal bool RequiresRestart => ActiveThreads.Length != 0;
    internal bool Failed => PendingRecords != 0 || IoErrors != 0 || DiagnosticFailures != 0 || RequiresRestart;
    internal string Describe() => $"Vendor disk writer: {PendingRecords} pending record(s) at inspection, " +
        $"{IoErrors} observed I/O error(s), {DiagnosticFailures} diagnostic failure(s), " +
        $"{ActiveThreads.Length} active thread(s) [{string.Join(", ", ActiveThreads)}]. " +
        "Pending count excludes dequeued, partially written or unacknowledged records; no automatic replay was attempted." +
        (RequiresRestart ? " This worker cannot be reused; stop the Host and start a new worker process." : "");
}
