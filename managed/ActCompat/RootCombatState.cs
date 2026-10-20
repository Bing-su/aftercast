namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    // Share ACT's action-data monitor; queue waits and plugin callbacks must happen outside it.
    internal readonly object CombatGate = ActGlobals.actionDataLock;
    private List<ZoneData> combatZones = new();
    private ZoneData combatZone = null!;
    private bool inCombat, combatStateInitialized;
    private DateTime lastHostileTime = DateTime.MinValue;
    private (string Name, DateTime Start) zoneContext;

    public List<ZoneData> ZoneList { get { lock (CombatGate) return combatZones; } set { lock (CombatGate) combatZones = value; } }
    public ZoneData ActiveZone { get { lock (CombatGate) return combatZone; } set { lock (CombatGate) combatZone = value; } }
    public bool InCombat { get { lock (CombatGate) return inCombat; } set { lock (CombatGate) inCombat = value; } }
    public DateTime LastHostileTime { get { lock (CombatGate) return lastHostileTime; } private set { lock (CombatGate) lastHostileTime = value; } }

    internal void InitializeCombatState(DateTime now)
    {
        lock (CombatGate)
        {
            if (combatStateInitialized) throw new InvalidOperationException("Combat state is already initialized.");
            // ACT reserves index zero for imported encounters, then adds the initial live zone.
            combatZones = new() { new(now, "Import", false, false, false) };
            combatZone = new(now, "Unknown Zone", true, false, false);
            combatZones.Add(combatZone);
            // Retain actual zone grouping context without claiming a UI/history database implementation.
            zoneContext = ("Unknown Zone", now);
            combatStateInitialized = true;
        }
    }

    internal void ChangeZoneState(string zone, DateTime time)
    {
        lock (CombatGate)
        {
            EnsureCombatStateInitialized();
            // A zone notification records context; an ongoing encounter keeps its existing zone graph.
            zoneContext = (zone, time);
        }
    }

    internal CombatToggleEventArgs? BeginEncounterState(DateTime time, string currentZone)
    {
        lock (CombatGate)
        {
            EnsureCombatStateInitialized();
            if (inCombat)
            {
                lastHostileTime = time;
                return null;
            }
            if (zoneContext.Name != combatZone.ZoneName && currentZone != combatZone.ZoneName ||
                zoneContext.Start < combatZone.StartTime)
            {
                var existing = combatZones.Find(zone => zone.StartTime == zoneContext.Start && zone.ZoneName == zoneContext.Name);
                if (existing is not null) combatZone = existing;
                else
                {
                    combatZone = new(zoneContext.Name == currentZone ? zoneContext.Start : time, currentZone, true, false, false);
                    var insert = combatZones.Count;
                    for (var index = 1; index < combatZones.Count; index++)
                        if (combatZones[index].StartTime > time) { insert = index; break; }
                    combatZones.Insert(insert, combatZone);
                }
            }
            var encounter = new EncounterData(ActGlobals.charName!, currentZone, combatZone);
            combatZone.ActiveEncounter = encounter;
            combatZone.Items.Add(encounter);
            // Preserve ACT's historical last-zone index quirk when an older zone was inserted or reused.
            return LastEncounterEvent(encounter);
        }
    }

    internal void MarkEncounterStartedState(DateTime time)
    {
        lock (CombatGate)
        {
            // The public wrapper invokes OnCombatStart first, while InCombat still has its previous value.
            lastHostileTime = time;
            inCombat = true;
        }
    }

    internal CombatToggleEventArgs CompleteEncounterState()
    {
        lock (CombatGate)
        {
            EnsureCombatStateInitialized();
            var encounter = combatZone.ActiveEncounter!;
            if (encounter.Active)
            {
                if (combatZone.PopulateAll) combatZone.Items[0].EndCombat(false);
                encounter.EndCombat(true);
            }
            // The public wrapper clears InCombat and drains queued actions before finalizing this graph.
            return LastEncounterEvent(encounter);
        }
    }

    private CombatToggleEventArgs LastEncounterEvent(EncounterData encounter) =>
        new(combatZones.Count - 1, combatZones[^1].Items.Count - 1, encounter);

    private void EnsureCombatStateInitialized()
    {
        if (!combatStateInitialized) throw new InvalidOperationException("Combat state is not initialized.");
    }
}
