namespace Aftercast.Bridge;

// Sticky for the process-owned PluginSession: resetting a session cannot stop a leaked native producer.
internal sealed class WorkerReusePolicy
{
    private bool unsafeToReuse;
    internal void MarkUnsafe() => unsafeToReuse = true;
    internal void Observe(WriterStopObservation observation) => unsafeToReuse |= observation.RequiresRestart;
    internal void RequireReusable()
    {
        if (unsafeToReuse)
            throw new InvalidOperationException("This worker cannot be reused after incomplete vendor shutdown; stop the Host and start a new worker process");
    }
}
