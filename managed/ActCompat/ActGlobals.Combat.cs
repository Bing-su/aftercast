namespace Advanced_Combat_Tracker;

public static partial class ActGlobals
{
    // These original defaults control category detail and encounter end-time selection.
    public static bool restrictToAll;
    public static bool longDuration;
    internal static object actionDataLock = new();
}
