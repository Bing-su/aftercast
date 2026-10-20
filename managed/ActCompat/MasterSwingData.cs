namespace Advanced_Combat_Tracker;

public partial class MasterSwing
{
    public MasterSwing(int swingType, bool critical, string special, Dnum damage, DateTime time,
        int timeSorter, string attackType, string attacker, string damageType, string victim)
    {
        SwingType = swingType;
        Critical = critical;
        Special = special;
        Damage = damage;
        Time = time;
        TimeSorter = timeSorter;
        AttackType = attackType;
        Attacker = attacker;
        DamageType = damageType;
        Victim = victim;
    }

    public string Special { get; } = null!;
    public EncounterData? ParentEncounter { get; set; }
    public DateTime Time { get; }
    public int TimeSorter { get; }
    public int SwingType { get; }
    public Dnum Damage { get; } = null!;
    public string Attacker { get; } = null!;
    public string Victim { get; } = null!;
    public string AttackType { get; } = null!;
    public string DamageType { get; } = null!;
    public bool Critical { get; set; }
    // Preserve the plugin's dictionary identity so later tag mutations remain visible to ACT callbacks.
    public Dictionary<string, object> Tags { get; set; } = new();
}
