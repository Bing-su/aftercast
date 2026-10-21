using System.Drawing;

namespace Advanced_Combat_Tracker;

// Keep category ownership, action insertion and dependent cached totals in one ACT object.
public class DamageTypeData
{
    private readonly string? tag;

    public DamageTypeData(bool outgoing, string? tag, CombatantData? parent)
    {
        Outgoing = outgoing;
        this.tag = tag;
        Parent = parent;
    }

    public CombatantData? Parent { get; }
    public bool Outgoing { get; set; }
    // The original getter dispatches through ToString, including overrides on derived category types.
    public string? Type => ToString();
    public SortedList<string, AttackType> Items { get; } = new();
    public Dictionary<string, object> Tags { get; set; } = new();
    public override string? ToString() => tag;

    public void AddCombatAction(MasterSwing action, string attackType)
    {
        InvalidateCachedValues();
        if (!Items.TryGetValue(attackType, out var attack))
        {
            attack = new AttackType(attackType, this);
            Items.Add(attackType, attack);
        }
        attack.AddCombatAction(action);
    }

    public void InvalidateCachedValues() => InvalidateDurationCache();

    public void InvalidateCachedValues(bool recursive)
    {
        InvalidateCachedValues();
        if (recursive)
            for (var index = 0; index < Items.Count; index++)
                Items.Values[index].InvalidateCachedValues(ActGlobals.disableIncrementalCaching);
    }

    // shortcut: All is ACT's verified built-in attackTypeTerm-all; custom localization requires ActGlobals.Trans.
    public long Damage => Items.TryGetValue("All", out var attack) ? attack.Damage : 0;
    public int Swings => Items.TryGetValue("All", out var attack) ? attack.Swings : 0;
    public int Hits => Items.TryGetValue("All", out var attack) ? attack.Hits : 0;
    // The original CritHits getter uses the literal All even when other getters resolve a localized key.
    public int CritHits => Items.TryGetValue("All", out var attack) ? attack.CritHits : 0;
    public int Misses => Items.TryGetValue("All", out var attack) ? attack.Misses : 0;
    public int Blocked => Items.TryGetValue("All", out var attack) ? attack.Blocked : 0;
    public DateTime StartTime => Items.TryGetValue("All", out var attack) ? attack.StartTime : DateTime.MaxValue;
    public DateTime EndTime => Items.TryGetValue("All", out var attack) ? attack.EndTime : DateTime.MinValue;

    private bool durationCached;
    private TimeSpan cachedDuration;
    internal void InvalidateDurationCache() => durationCached = false;

    public TimeSpan Duration
    {
        get
        {
            if (Parent is not null && Parent.Parent.StartTimes.Count > 1)
            {
                if (durationCached) return cachedDuration;
                if (!Items.TryGetValue("All", out var attack) || attack is null) return TimeSpan.Zero;
                cachedDuration = PersonalDuration.Calculate(Parent.Parent, attack.Items, combatant: false);
                durationCached = true;
                return cachedDuration;
            }
            return EndTime > StartTime ? EndTime - StartTime : TimeSpan.Zero;
        }
    }

    // Use the same verified built-in All key as the scalar category getters.
    public double DPS => Items.TryGetValue("All", out var attack) ? attack.DPS : 0;
    public double EncDPS => Items.TryGetValue("All", out var attack) ? attack.EncDPS : 0;
    public double ExtDPS => EncDPS;
    public double CharDPS => Items.TryGetValue("All", out var attack) ? attack.CharDPS : 0;

    public void Trim()
    {
        Items.TrimExcess();
        foreach (var attack in Items.Values) attack.Trim();
    }

    public static Dictionary<string, ColumnDef> ColumnDefs = new();
    public string GetColumnByName(string name) => ColumnDefs.TryGetValue(name, out var column) ? column.GetCellData(this) : string.Empty;
    public delegate string StringDataCallback(DamageTypeData data);
    public delegate Color ColorDataCallback(DamageTypeData data);
    public class ColumnDef(string label, bool defaultVisible, string sqlDataType, string sqlDataName,
        StringDataCallback cell, StringDataCallback sql)
    {
        public string Label { get; } = label;
        public bool DefaultVisible { get; } = defaultVisible;
        public string SqlDataType { get; } = sqlDataType;
        public string SqlDataName { get; } = sqlDataName;
        public StringDataCallback GetCellData = cell;
        public StringDataCallback GetSqlData = sql;
        public ColorDataCallback GetCellForeColor = static _ => Color.Transparent;
        public ColorDataCallback GetCellBackColor = static _ => Color.Transparent;
    }

    public float ToHit
    {
        get
        {
            // Preserve category-level failure containment as well as ACT's single-precision arithmetic.
            try { return (float)Hits / Swings * 100f; }
            catch (Exception) { return 0; }
        }
    }
}
