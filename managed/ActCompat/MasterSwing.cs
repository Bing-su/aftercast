using System.Drawing;

namespace Advanced_Combat_Tracker;

public class MasterSwing
{
    private MasterSwing() { }
    public static Dictionary<string, ColumnDef> ColumnDefs = new();
    public string GetColumnByName(string name) => ColumnDefs.TryGetValue(name, out var column) ? column.GetCellData(this) : string.Empty;
    public delegate string StringDataCallback(MasterSwing data);
    public delegate Color ColorDataCallback(MasterSwing data);
    public class ColumnDef(string label, bool defaultVisible, string sqlDataType, string sqlDataName,
        StringDataCallback cell, StringDataCallback sql, Comparison<MasterSwing> sort)
    {
        public string Label { get; } = label;
        public bool DefaultVisible { get; } = defaultVisible;
        public string SqlDataType { get; } = sqlDataType;
        public string SqlDataName { get; } = sqlDataName;
        public StringDataCallback GetCellData = cell;
        public StringDataCallback GetSqlData = sql;
        public Comparison<MasterSwing> SortComparer = sort;
        public ColorDataCallback GetCellForeColor = static _ => Color.Transparent;
        public ColorDataCallback GetCellBackColor = static _ => Color.Transparent;
    }

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

    public string Special { get; private set; } = null!;
    public EncounterData? ParentEncounter { get; set; }
    public DateTime Time { get; private set; }
    public int TimeSorter { get; private set; }
    public int SwingType { get; private set; }
    public Dnum Damage { get; private set; } = null!;
    public string Attacker { get; private set; } = null!;
    public string Victim { get; private set; } = null!;
    public string AttackType { get; private set; } = null!;
    public string DamageType { get; private set; } = null!;
    public bool Critical { get; set; }
    // Preserve the plugin's dictionary identity so later tag mutations remain visible to ACT callbacks.
    public Dictionary<string, object> Tags { get; set; } = new();

    internal void NormalizeForHost()
    {
        // ACT normalizes the original object before building the mutable BeforeCombatAction snapshot.
        if (string.IsNullOrWhiteSpace(Special) || Special is "hit" or "hits") Special = "None";
        Attacker = Attacker.Trim();
        Victim = Victim.Trim();
        AttackType = AttackType.Trim();
    }

    internal void Apply(CombatActionEventArgs info)
    {
        SwingType = info.swingType;
        Critical = info.critical;
        // Preserve ACT's commit order and string interning, including partial mutation when Intern rejects null.
        Special = string.Intern(info.special);
        Damage = info.damage;
        Time = info.time;
        TimeSorter = info.timeSorter;
        AttackType = string.Intern(info.theAttackType);
        Attacker = string.Intern(info.attacker);
        DamageType = string.Intern(info.theDamageType);
        Victim = string.Intern(info.victim);
        // Event tags alias Tags initially; replacing that field never replaces the original action dictionary.
    }
}
