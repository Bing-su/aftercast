namespace Aftercast.ActCompat;

// Keep session identities monotonic across stop/restart in the same CLR.
public sealed class HostSession
{
    private static long nextId;
    public ulong Id { get; } = unchecked((ulong)Interlocked.Increment(ref nextId));
}
