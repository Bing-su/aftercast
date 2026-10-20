namespace Advanced_Combat_Tracker;

// Keep mutable graph state owned by its ACT object; these collections do not imply aggregation.
public class ZoneData : IComparable<ZoneData>
{
    public ZoneData(DateTime startTime, string zoneName, bool populateAll, bool selectiveParsing, bool ignoreEnemies)
    {
        StartTime = startTime;
        ZoneName = zoneName;
        PopulateAll = populateAll;
        if (populateAll)
            Items.Add(selectiveParsing
                ? new EncounterData(ActGlobals.charName!, "All", ignoreEnemies, this)
                : new EncounterData(ActGlobals.charName!, "All", this));
    }

    public bool PopulateAll { get; set; }
    public DateTime StartTime { get; set; }
    public string ZoneName { get; set; }
    public List<EncounterData> Items { get; set; } = new();
    public EncounterData? ActiveEncounter { get; set; }
    public Dictionary<string, object> Tags { get; set; } = new();
    public int CompareTo(ZoneData? other) => StartTime.CompareTo(other!.StartTime);
}

public partial class EncounterData
{
    private string zoneName;
    private string title = "Encounter";
    private List<DateTime> startTimes = new();
    private List<DateTime> endTimes = new();
    // Preserve constructor policy for the later selective combat-action implementation.
    internal readonly bool SelectiveParsing;
    internal readonly bool IgnoreEnemies;

    public EncounterData(string charName, string zoneName, bool ignoreEnemies, ZoneData parent)
        : this(charName, zoneName, parent)
    {
        SelectiveParsing = true;
        IgnoreEnemies = ignoreEnemies;
    }

    public EncounterData(string charName, string zoneName, ZoneData parent)
    {
        CharName = charName;
        this.zoneName = zoneName;
        Parent = parent;
    }

    public ZoneData Parent { get; set; }
    public string CharName { get; set; }
    // shortcut: these verified built-in English terms need ActGlobals.Trans for custom localization.
    public string ZoneName { get => zoneName == "All" && Parent is not null ? Parent.ZoneName : zoneName; set => zoneName = value; }
    public string Title { get => zoneName == "All" ? "All" : title; set => title = value; }
    public bool Active { get; set; }
    public bool DuplicateDetection { get; set; }
    public HistoryRecord? HistoryRecord { get; set; }
    public SortedList<string, CombatantData> Items { get; set; } = new();
    public List<LogLineEntry> LogLines { get; set; } = new();
    public Dictionary<string, object> Tags { get; set; } = new();
    public int NumCombatants => Items.Count;
    public List<DateTime> StartTimes
    {
        // ACT removes unset sentinels on read while returning the same caller-mutable list.
        get { while (startTimes.Remove(DateTime.MaxValue)) { } return startTimes; }
        set => startTimes = value;
    }
    public List<DateTime> EndTimes
    {
        get { while (endTimes.Remove(DateTime.MinValue)) { } return endTimes; }
        set => endTimes = value;
    }
}

public partial class CombatantData
{
    private DamageTypeData? outAll;
    private DamageTypeData? incAll;

    public CombatantData(string name, EncounterData parent)
    {
        Name = name;
        // ACT keeps the last category object, so registration insertion order is observable.
        foreach (var key in OutgoingDamageTypeDataObjects.Keys)
        {
            outAll = new DamageTypeData(true, key, this);
            Items.Add(key, outAll);
        }
        foreach (var key in IncomingDamageTypeDataObjects.Keys)
        {
            incAll = new DamageTypeData(false, key, this);
            Items.Add(key, incAll);
        }
        Parent = parent;
    }

    public string Name { get; }
    public EncounterData Parent { get; }
    public Dictionary<string, DamageTypeData> Items { get; set; } = new();
    public SortedList<string, int> Allies { get; set; } = new();
    public Dictionary<string, object> Tags { get; set; } = new();
    public SortedList<string, AttackType> AllOut => outAll!.Items;
    public SortedList<string, AttackType> AllInc => incAll!.Items;
}

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
