namespace Advanced_Combat_Tracker;

// Keep caller-owned encounter collections and their ACT action-routing policy together.
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

    public void AddCombatAction(MasterSwing action)
    {
        // Require a selection provider before changing segments, e.g. a standalone selective graph must not half-apply an action.
        if (PopulateAll) Items[0].EnsureActionSupported();
        Items[^1].EnsureActionSupported();
        if (PopulateAll) Deliver(Items[0], action);
        Deliver(Items[^1], action);
    }

    private static void Deliver(EncounterData encounter, MasterSwing action)
    {
        if (!encounter.Active)
        {
            encounter.StartTimes.Add(action.Time);
            encounter.Active = true;
        }
        encounter.AddCombatAction(action);
    }
}
