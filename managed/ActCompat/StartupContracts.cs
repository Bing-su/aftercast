namespace Advanced_Combat_Tracker;

public delegate void LogLineEventDelegate(bool isImport, LogLineEventArgs logInfo);
public delegate void LogFileChangedDelegate(bool isImport, string newLogFileName);

public class LogLineEventArgs : EventArgs
{
    public string logLine;
    public int detectedType;
    public readonly DateTime detectedTime;
    public readonly string detectedZone;
    public readonly bool inCombat;
    public readonly string originalLogLine;
    public readonly string companionLogName;

    public LogLineEventArgs(string logLine, int detectedType, DateTime detectedTime, string detectedZone, bool inCombat)
        : this(logLine, detectedType, detectedTime, detectedZone, inCombat, string.Empty) { }

    public LogLineEventArgs(string logLine, int detectedType, DateTime detectedTime, string detectedZone, bool inCombat, string companionLogName)
    {
        this.logLine = originalLogLine = logLine;
        this.detectedType = detectedType;
        this.detectedTime = detectedTime;
        this.detectedZone = detectedZone;
        this.inCombat = inCombat;
        this.companionLogName = companionLogName;
    }
}

public partial class FormActMain
{
    private volatile bool readThreadLock;
    // Workers observe the real pause flag; companion-log propagation awaits companion-log support.
    public bool ReadThreadLock { get => readThreadLock; set => readThreadLock = value; }
    public delegate void NullDelegate();
    public delegate DateTime DateTimeLogParser(string logLine);

    // Preserve the writable field so plugins can install and restore their own timestamp parser.
    public DateTimeLogParser GetDateTimeFromLog = ParseDateTimeFromEQ2Log;

    public event LogLineEventDelegate? BeforeLogLineRead;
    public event LogFileChangedDelegate? LogFileChanged;
    public event NullDelegate? UpdateCheckClicked;

    // An embedded library must leave application updates to its consumer.
    public bool GetAutomaticUpdatesAllowed() => false;
    public int TimeStampLen { get; set; } = 39;
    public bool LogPathHasCharName { get; set; } = true;
    public System.Text.RegularExpressions.Regex? ZoneChangeRegex { get; set; } =
        new(@"\(\d{10}\)\[.{24}\] You have entered (?::.+?:)?(?<zone>.+)\.");

    private static DateTime ParseDateTimeFromEQ2Log(string line)
    {
        try
        {
            if (line.Length < 39) return DateTime.MinValue;
            // ACT reads the displayed date, not the leading Unix counter; unknown month names fall back to January.
            var month = line.Substring(17, 3) switch
            {
                "Feb" => 2, "Mar" => 3, "Apr" => 4, "May" => 5, "Jun" => 6, "Jul" => 7,
                "Aug" => 8, "Sep" => 9, "Oct" => 10, "Nov" => 11, "Dec" => 12, _ => 1
            };
            return new(Convert.ToInt32(line.Substring(33, 4)), month, Convert.ToInt32(line.Substring(21, 2)),
                Convert.ToInt32(line.Substring(24, 2)), Convert.ToInt32(line.Substring(27, 2)), Convert.ToInt32(line.Substring(30, 2)));
        }
        catch { return DateTime.MinValue; }
    }

    internal void DispatchLogLine(bool isImport, LogLineEventArgs logInfo) => InvokeLogHandlers(BeforeLogLineRead, isImport, logInfo);
    internal void DispatchLogFileChanged(bool isImport, string newLogFileName) => LogFileChanged?.Invoke(isImport, newLogFileName);
    internal void DispatchUpdateCheck() => UpdateCheckClicked?.Invoke();
}
