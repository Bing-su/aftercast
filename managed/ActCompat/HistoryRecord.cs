namespace Advanced_Combat_Tracker;

public class HistoryRecord
{
    public HistoryRecord(int type, DateTime startTime, DateTime endTime, string label, string charName, string folderHint)
    {
        Type = type;
        StartTime = startTime;
        EndTime = endTime;
        Label = string.Intern(label);
        CharName = string.Intern(charName);
        FolderHint = string.Intern(folderHint);
    }
    public int Type { get; set; }
    public DateTime StartTime { get; set; }
    public DateTime EndTime { get; set; }
    public string Label { get; set; }
    public string CharName { get; set; }
    public string FolderHint { get; set; }
    public TimeSpan Duration => EndTime - StartTime;
}
