namespace Advanced_Combat_Tracker;

public delegate void CombatActionDelegate(bool isImport, CombatActionEventArgs actionInfo);
public delegate void CombatToggleEventDelegate(bool isImport, CombatToggleEventArgs encounterInfo);

public class CombatActionEventArgs : EventArgs
{
    public int swingType;
    public bool critical;
    public string attacker;
    public string theAttackType;
    public Dnum damage;
    public DateTime time;
    public int timeSorter;
    public string victim;
    public string theDamageType;
    public string special;
    public Dictionary<string, object> tags;
    public readonly MasterSwing combatAction;
    public bool cancelAction;

    public CombatActionEventArgs(MasterSwing action)
    {
        // Scalar edits remain a hook snapshot; ACT deliberately shares the original action and tag dictionary.
        combatAction = action;
        swingType = action.SwingType;
        critical = action.Critical;
        attacker = action.Attacker;
        theAttackType = action.AttackType;
        damage = action.Damage;
        time = action.Time;
        timeSorter = action.TimeSorter;
        victim = action.Victim;
        theDamageType = action.DamageType;
        special = action.Special;
        tags = action.Tags;
    }
}

public class CombatToggleEventArgs(int zoneDataIndex, int encounterDataIndex, EncounterData? encounter) : EventArgs
{
    public readonly int zoneDataIndex = zoneDataIndex;
    public readonly int encounterDataIndex = encounterDataIndex;
    public readonly EncounterData? encounter = encounter;
}
