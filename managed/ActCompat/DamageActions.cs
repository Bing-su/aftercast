namespace Advanced_Combat_Tracker;

public partial class AttackType
{
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
}

public partial class DamageTypeData
{
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
}
