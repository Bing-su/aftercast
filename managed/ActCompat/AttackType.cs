using System.Drawing;

namespace Advanced_Combat_Tracker;

public class AttackType
{
    public AttackType(string? attackType, DamageTypeData? parent)
    {
        Type = attackType;
        Parent = parent;
    }

    public DamageTypeData? Parent { get; }
    public string? Type { get; }
    public List<MasterSwing> Items { get; set; } = new();
    public Dictionary<string, object> Tags { get; set; } = new();
    public override string? ToString() => Type;

    private (int Count, long Value) damage;
    private (int Count, int Value) hits, critHits, swings, misses, blocked;
    private (int Count, DateTime Value) startTime = (0, DateTime.MaxValue);
    private (int Count, DateTime Value) endTime = (0, DateTime.MinValue);

    public void AddCombatAction(MasterSwing action)
    {
        InvalidateCachedValues(ActGlobals.disableIncrementalCaching);
        Items.Add(action);
    }

    public void InvalidateCachedValues() => InvalidateDurationCache();

    public void InvalidateCachedValues(bool full)
    {
        if (full)
        {
            damage = default;
            hits = critHits = swings = misses = blocked = default;
            startTime = (0, DateTime.MaxValue);
            endTime = (0, DateTime.MinValue);
        }
        InvalidateCachedValues();
    }

    // ACT keeps independent append cursors; same-count edits stay cached until explicitly reset.
    private T ReadCache<T>(ref (int Count, T Value) cache, Func<T, MasterSwing, AttackType, T> accumulate)
    {
        var count = Items.Count;
        var value = cache.Value;
        if (cache.Count == count) return value;
        for (var index = cache.Count; index < count; index++)
            value = accumulate(value, Items[index], this);
        cache = (count, value);
        return value;
    }

    public long Damage => ReadCache(ref damage, static (value, action, _) => action.Damage.Number > 0
        // Preserve Dnum's implicit clamp after a previous sum overflowed below -10.
        ? (long)((Dnum)value + action.Damage) : value);
    // ACT returns NaN when there are no hits and otherwise uses the existing
    // damage and hits caches (including prior blockIsHit reads).
    public double Average => Hits > 0 ? (double)Damage / Hits : double.NaN;
    public int Hits => ReadCache(ref hits, static (value, action, _) =>
        (ActGlobals.blockIsHit ? action.Damage.Number >= 0 : action.Damage.Number > 0) ? unchecked(value + 1) : value);
    public int CritHits => ReadCache(ref critHits, static (value, action, _) => action.Critical &&
        (ActGlobals.blockIsHit ? action.Damage.Number >= 0 : action.Damage.Number > 0) ? unchecked(value + 1) : value);
    // shortcut: the verified built-in Killing term needs ActGlobals.Trans for external localization overrides.
    public int Swings => ReadCache(ref swings, static (value, action, owner) =>
        action.Damage != Dnum.Death || owner.Type == "Killing" ? unchecked(value + 1) : value);
    public int Misses => ReadCache(ref misses, static (value, action, _) => action.Damage == Dnum.Miss ? unchecked(value + 1) : value);
    public int Blocked => ReadCache(ref blocked, static (value, action, _) =>
        action.Damage.Number < -1 && action.Damage != Dnum.Death ? unchecked(value + 1) : value);
    public DateTime StartTime => ReadCache(ref startTime, static (value, action, _) => action.Time < value ? action.Time : value);
    public DateTime EndTime => ReadCache(ref endTime, static (value, action, _) => action.Time > value ? action.Time : value);

    private bool durationCached;
    private TimeSpan cachedDuration;
    internal void InvalidateDurationCache() => durationCached = false;

    public TimeSpan Duration
    {
        get
        {
            var actor = Parent!.Parent;
            if (actor is not null && actor.Parent.StartTimes.Count > 1)
            {
                if (!durationCached)
                {
                    cachedDuration = PersonalDuration.Calculate(actor.Parent, Items, combatant: false);
                    durationCached = true;
                }
                return cachedDuration;
            }
            return EndTime > StartTime ? EndTime - StartTime : TimeSpan.Zero;
        }
    }

    public double DPS { get { var seconds = Duration.TotalSeconds; return seconds > 0 ? Damage / seconds : 0; } }
    public double EncDPS
    {
        get
        {
            var actor = Parent!.Parent;
            if (actor is null) return double.NaN;
            var seconds = actor.Parent.Duration.TotalSeconds;
            return seconds > 0 ? Damage / seconds : 0;
        }
    }
    [Obsolete]
    public double ExtDPS => EncDPS;
    public double CharDPS
    {
        get
        {
            var actor = Parent!.Parent;
            if (actor is null) return double.NaN;
            var seconds = actor.Duration.TotalSeconds;
            return seconds > 0 ? Damage / seconds : 0;
        }
    }

    public Dictionary<string, int> GetAttackSpecials()
    {
        var ordered = new List<MasterSwing>(Items);
        ordered.Sort(static (left, right) =>
        {
            var order = left.TimeSorter.CompareTo(right.TimeSorter);
            return order != 0 ? order : left.Time.CompareTo(right.Time);
        });
        var seen = new List<string>();
        var counts = new Dictionary<string, int>();
        foreach (var action in ordered)
        {
            // shortcut: ACT localization overrides need ActGlobals.Trans; use its built-in English term.
            if (action.Special == "None") seen.Clear();
            if (!seen.Contains(action.Special))
            {
                Increment(action.Special);
                if (action.Special != "None")
                {
                    Increment("ANY");
                    if (!seen.Contains("ONCE")) { Increment("ONCE"); seen.Add("ONCE"); }
                }
            }
            seen.Add(action.Special);
        }
        return counts;

        void Increment(string key)
        {
            counts.TryGetValue(key, out var count);
            counts[key] = unchecked(count + 1);
        }
    }

    public void Trim() => Items.TrimExcess();

    public static Dictionary<string, ColumnDef> ColumnDefs = new();
    public string GetColumnByName(string name) => ColumnDefs.TryGetValue(name, out var column) ? column.GetCellData(this) : string.Empty;
    public delegate string StringDataCallback(AttackType data);
    public delegate Color ColorDataCallback(AttackType data);
    public class ColumnDef(string label, bool defaultVisible, string sqlDataType, string sqlDataName,
        StringDataCallback cell, StringDataCallback sql, Comparison<AttackType> sort)
    {
        public string Label { get; } = label;
        public bool DefaultVisible { get; } = defaultVisible;
        public string SqlDataType { get; } = sqlDataType;
        public string SqlDataName { get; } = sqlDataName;
        public StringDataCallback GetCellData = cell;
        public StringDataCallback GetSqlData = sql;
        public Comparison<AttackType> SortComparer = sort;
        public ColorDataCallback GetCellForeColor = static _ => Color.Transparent;
        public ColorDataCallback GetCellBackColor = static _ => Color.Transparent;
    }

    public float ToHit
    {
        get
        {
            // ACT catches malformed caller-owned data here; ordinary empty data still yields floating-point NaN.
            try { return (float)Hits / Swings * 100f; }
            catch (Exception) { return 0; }
        }
    }
}
