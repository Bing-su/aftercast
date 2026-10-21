namespace Advanced_Combat_Tracker;

// These payloads retain actual caller data for encounter logs/history; no synthetic combat totals.
public class LogLineEntry
{
    public LogLineEntry(DateTime time, string logLine, int type, int globalTimeSorter)
    {
        Time = time;
        LogLine = logLine;
        Type = type;
        GlobalTimeSorter = globalTimeSorter;
    }
    public int GlobalTimeSorter { get; }
    public DateTime Time { get; set; }
    public string LogLine { get; }
    public int Type { get; }
    public bool SearchSelected { get; set; }
}
