namespace Advanced_Combat_Tracker;

public static partial class ActGlobals
{
    // Preserve the original startup policy for hit classification and append-only scalar caches.
    public static bool blockIsHit = true;
    public static bool disableIncrementalCaching;
}
