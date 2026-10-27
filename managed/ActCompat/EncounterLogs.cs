namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    public bool RecordEncounterLogs { get; set; } = true;
    public bool OriginalLoglineViewLogs { get; set; }

    private void RecordEncounterLog(LogLineEventArgs info, int sorter, bool wasInCombat)
    {
        lock (CombatGate)
        {
            // ACT records after BeforeLogLineRead, including the line that starts or ends combat.
            if (!RecordEncounterLogs || !(inCombat || wasInCombat) || combatZone.ActiveEncounter is not { } encounter) return;
            encounter.RetainLog(new(info.detectedTime, OriginalLoglineViewLogs ? info.originalLogLine : info.logLine, info.detectedType, sorter));
        }
    }
}
