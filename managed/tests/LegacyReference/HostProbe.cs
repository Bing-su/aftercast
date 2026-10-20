using Advanced_Combat_Tracker;

namespace Bxivact.LegacyReference;

// Emit real ldsfld/stsfld and method calls against the original ACT declarations.
public static class HostProbe
{
    public static object Root() => ActGlobals.oFormActMain;
    public static string Character(string value)
    {
        ActGlobals.charName = value;
        return ActGlobals.charName;
    }
    public static string AppDataPath() => ActGlobals.oFormActMain.AppDataFolder.FullName;
    public static bool MissingRegistration() => ActGlobals.oFormActMain.PluginGetSelfData(null) is null;
    public static void Restart() => ActGlobals.oFormActMain.RestartACT(false, "fixture restart");
    public static void BackgroundDiagnostic()
    {
        var thread = new Thread(() => ActGlobals.oFormActMain.WriteExceptionLog(new InvalidOperationException("background diagnostic"), "probe"));
        thread.Start();
        thread.Join();
    }
}
