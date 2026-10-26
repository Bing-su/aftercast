namespace Aftercast.ActCompat;

// A fresh process must reject results from an older worker, e.g. a saved CombatRevision.
public sealed class HostSession
{
    public ulong Id { get; } = unchecked((ulong)System.Security.Cryptography.RandomNumberGenerator.GetInt32(1, int.MaxValue)) << 32
        | unchecked((uint)System.Security.Cryptography.RandomNumberGenerator.GetInt32(1, int.MaxValue));
}
