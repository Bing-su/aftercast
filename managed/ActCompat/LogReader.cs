using System.Text.RegularExpressions;

namespace Advanced_Combat_Tracker;

// Root lifecycle treats a single follower and an explicitly selected source group as one owned reader.
internal abstract class LogReader : IDisposable
{
    internal abstract bool IsAlive { get; }
    internal abstract bool CallbackActive { get; }
    internal virtual int ResourceCost => 1;
    internal abstract string? ReadInitialZone(Regex? regex, Func<bool> canceled);
    internal abstract void CompleteInitialSnapshot();
    internal abstract void Suspend(Action? pump);
    internal abstract void RequestSuspension();
    internal abstract void Resume();
    internal abstract void Dispose(Action? pump);
    public abstract void Dispose();
}
