using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

// Compile public file-follow metadata against original ACT without replacing the active host.
public static class LogHostProbe
{
    public static string Run()
    {
        var main = ActGlobals.oFormActMain;
        var path = main.LogFilePath;
        var filter = main.LogFileFilter;
        var time = main.LastKnownTime;
        var zone = main.CurrentZone;
        LogLineEventDelegate handler = (_, _) => { };
        try
        {
            main.LogFilePath = "fixture.log";
            main.LogFileFilter = "Network_*.log";
            main.LastKnownTime = new DateTime(2026, 10, 20, 12, 34, 56, DateTimeKind.Utc);
            main.CurrentZone = "Fixture zone";
            main.OnLogLineRead += handler;
            main.OnLogLineRead -= handler;
            if (main.LogFilePath != "fixture.log" || main.LogFileFilter != "Network_*.log" ||
                main.LastKnownTime.Year != 2026 || main.CurrentZone != "Fixture zone")
                throw new Exception("Original-reference log host probe failed");
            return main.LogFileFilter;
        }
        finally
        {
            main.LogFilePath = path;
            main.LogFileFilter = filter;
            main.LastKnownTime = time;
            main.CurrentZone = zone;
        }
    }
}
