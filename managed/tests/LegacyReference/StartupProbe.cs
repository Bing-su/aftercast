using Advanced_Combat_Tracker;

namespace Bxivact.LegacyReference;

// Emit the original event, nested delegate, and field signatures for runtime binding checks.
public static class StartupProbe
{
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Original-reference ACT startup contract probe failed");
    }

    public static string Run()
    {
        var timestamp = new DateTime(2026, 10, 20, 12, 34, 56, DateTimeKind.Utc);
        var payload = new LogLineEventArgs("original", 1, timestamp, "zone", true, "companion");
        payload.logLine = "changed";
        payload.detectedType = 2;
        Check(payload.logLine == "changed" && payload.detectedType == 2 && payload.originalLogLine == "original");
        Check(payload.detectedTime == timestamp && payload.detectedZone == "zone" && payload.inCombat && payload.companionLogName == "companion");
        Check(new LogLineEventArgs("short", 0, timestamp, "zone", false).companionLogName == "");

        LogLineEventDelegate lineHandler = (isImport, info) => Check(isImport && ReferenceEquals(info, payload));
        LogFileChangedDelegate fileHandler = (isImport, file) => Check(isImport && file == "fixture.log");
        FormActMain.NullDelegate updateHandler = () => { };
        lineHandler(true, payload);
        fileHandler(true, "fixture.log");
        updateHandler();

        var main = ActGlobals.oFormActMain;
        main.BeforeLogLineRead += lineHandler;
        main.LogFileChanged += fileHandler;
        main.UpdateCheckClicked += updateHandler;
        main.BeforeLogLineRead -= lineHandler;
        main.LogFileChanged -= fileHandler;
        main.UpdateCheckClicked -= updateHandler;
        Check(!main.GetAutomaticUpdatesAllowed());
        main.TimeStampLen = 13;
        main.LogPathHasCharName = false;
        main.ZoneChangeRegex = new System.Text.RegularExpressions.Regex("^zone$");
        Check(main.TimeStampLen == 13 && !main.LogPathHasCharName && main.ZoneChangeRegex.IsMatch("zone"));

        var previousParser = main.GetDateTimeFromLog;
        try
        {
            main.GetDateTimeFromLog = line => line == "fixture" ? timestamp : throw new FormatException("Unexpected log line");
            Check(main.GetDateTimeFromLog("fixture") == timestamp);
        }
        finally
        {
            main.GetDateTimeFromLog = previousParser;
        }
        return payload.originalLogLine;
    }
}
