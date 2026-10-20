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
    public delegate void NullDelegate();
    public delegate DateTime DateTimeLogParser(string logLine);

    // Preserve the writable field so plugins can install and restore their own timestamp parser.
    public DateTimeLogParser GetDateTimeFromLog = static _ =>
        throw new PlatformNotSupportedException("The embedded host has no default ACT log timestamp parser");

    public event LogLineEventDelegate? BeforeLogLineRead;
    public event LogFileChangedDelegate? LogFileChanged;
    public event NullDelegate? UpdateCheckClicked;

    // An embedded library must leave application updates to its consumer.
    public bool GetAutomaticUpdatesAllowed() => false;
    public int TimeStampLen { get; set; }
    public bool LogPathHasCharName { get; set; }
    public System.Text.RegularExpressions.Regex? ZoneChangeRegex { get; set; }

    internal void DispatchLogLine(bool isImport, LogLineEventArgs logInfo) => BeforeLogLineRead?.Invoke(isImport, logInfo);
    internal void DispatchLogFileChanged(bool isImport, string newLogFileName) => LogFileChanged?.Invoke(isImport, newLogFileName);
    internal void DispatchUpdateCheck() => UpdateCheckClicked?.Invoke();
}
