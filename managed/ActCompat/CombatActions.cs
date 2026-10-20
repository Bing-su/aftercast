namespace Advanced_Combat_Tracker;

public static partial class ActGlobals
{
    // These original defaults control category detail and encounter end-time selection.
    public static bool restrictToAll;
    public static bool longDuration;
}

public partial class ZoneData
{
    public void AddCombatAction(MasterSwing action)
    {
        // Reject unsupported selection before changing segment state or action ownership.
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

public partial class EncounterData
{
    private readonly HashSet<int> includedTimeSorters = new();
    private List<CombatantData>? selectedAllies;
    private bool alliesManual;
    private bool alliesCached;

    internal void EnsureActionSupported()
    {
        if (SelectiveParsing)
            throw new NotSupportedException("Selective combat parsing requires the host's actual selection policy.");
    }

    public void AddCombatAction(MasterSwing action)
    {
        EnsureActionSupported();
        if (DuplicateDetection && !includedTimeSorters.Add(action.TimeSorter)) return;
        action.ParentEncounter = this;
        var attackerKey = action.Attacker.ToUpper();
        if (!Items.TryGetValue(attackerKey, out var attacker))
        {
            attacker = new CombatantData(action.Attacker, this);
            Items.Add(attackerKey, attacker);
        }
        attacker.AddCombatAction(action);
        AddReverseCombatAction(action);
    }

    public void AddReverseCombatAction(MasterSwing action)
    {
        EnsureActionSupported();
        var victimKey = action.Victim.ToUpper();
        if (!Items.TryGetValue(victimKey, out var victim))
        {
            victim = new CombatantData(action.Victim, this);
            Items.Add(victimKey, victim);
        }
        victim.AddReverseCombatAction(action);
    }

    public CombatantData? GetCombatant(string? name) => name is not null && Items.TryGetValue(name.ToUpper(), out var combatant) ? combatant : null;
    public bool GetIgnoreEnemies() => IgnoreEnemies;
    public void SetAllies(List<CombatantData>? allies)
    {
        if (allies is null || allies.Count == 0)
        {
            alliesCached = alliesManual = false;
            return;
        }
        selectedAllies = allies;
        alliesCached = alliesManual = true;
    }

    public void SetAlliesUncached()
    {
        if (!alliesManual) alliesCached = false;
    }

    public List<CombatantData> GetAllies() => GetAllies(false);
    public List<CombatantData> GetAllies(bool allowLimited)
    {
        if (alliesCached) return selectedAllies!;
        if (IgnoreEnemies) return new(Items.Values);
        throw new NotSupportedException("Automatic ally resolution requires the ACT relationship-graph policy; call SetAllies with an explicit nonempty list.");
    }

    private List<CombatantData> AggregateActors() => IgnoreEnemies ? new(Items.Values) : GetAllies();
    public long Damage => AggregateActors().Aggregate(0L, static (total, actor) => unchecked(total + actor.Damage));
    public long Healed => AggregateActors().Aggregate(0L, static (total, actor) => unchecked(total + actor.Healed));
    public int NumAllies => GetAllies().Count;
    public int NumEnemies => NumCombatants - NumAllies;
    public DateTime StartTime => Items.Values.Aggregate(DateTime.MaxValue, static (earliest, actor) => actor.StartTime < earliest ? actor.StartTime : earliest);
    public DateTime ShortEndTime
    {
        get
        {
            var actors = AggregateActors();
            if (actors.Count == 0) actors = new(Items.Values);
            return actors.Aggregate(DateTime.MinValue, static (latest, actor) => actor.ShortEndTime > latest ? actor.ShortEndTime : latest);
        }
    }
    public DateTime EndTime => ActGlobals.longDuration
        ? Items.Values.Aggregate(DateTime.MinValue, static (latest, actor) => actor.EndTime > latest ? actor.EndTime : latest)
        : ShortEndTime;
    public TimeSpan Duration
    {
        get
        {
            var starts = StartTimes;
            if (starts.Count <= 1) return EndTime > StartTime ? EndTime - StartTime : TimeSpan.Zero;
            try
            {
                var result = TimeSpan.Zero;
                for (var index = 0; index < starts.Count; index++)
                    result += (EndTimes.Count == index ? EndTime : EndTimes[index]) - starts[index];
                return result;
            }
            catch (NotSupportedException) { throw; } // Preserve explicit host-policy failures instead of reporting a fabricated duration.
            catch (Exception) { return TimeSpan.Zero; } // ACT treats malformed caller-edited segment lists as zero duration.
        }
    }

    public void EndCombat(bool finalize)
    {
        if (finalize) throw new NotSupportedException("Encounter finalization requires ACT title selection and trimming policies.");
        var end = EndTime;
        var start = StartTimes[EndTimes.Count];
        Active = false;
        EndTimes.Add(start < end ? end : start);
    }
}

public partial class CombatantData
{
    // Category names are configured by parser plugins; empty names match the original static defaults.
    public static string DamageTypeDataNonSkillDamage = string.Empty;
    public static string DamageTypeDataOutgoingDamage = string.Empty;
    public static string DamageTypeDataOutgoingHealing = string.Empty;
    public static string DamageTypeDataIncomingDamage = string.Empty;
    public static string DamageTypeDataIncomingHealing = string.Empty;
    public static string DamageTypeDataOutgoingPowerReplenish = string.Empty;
    public static string DamageTypeDataOutgoingPowerDamage = string.Empty;
    public static string DamageTypeDataOutgoingCures = string.Empty;
    private bool startCached, endCached;
    private DateTime cachedStart, cachedEnd;

    public void InvalidateCachedValues() { startCached = endCached = false; }
    public void InvalidateCachedValues(bool recursive)
    {
        InvalidateCachedValues();
        if (recursive) foreach (var category in Items.Values) category.InvalidateCachedValues(true);
    }

    public void AddCombatAction(MasterSwing action) => Route(action, true);
    public void AddReverseCombatAction(MasterSwing action) => Route(action, false);
    private void Route(MasterSwing action, bool outgoing)
    {
        startCached = endCached = false;
        var links = outgoing ? SwingTypeToDamageTypeDataLinksOutgoing : SwingTypeToDamageTypeDataLinksIncoming;
        if (!links.TryGetValue(action.SwingType, out var categories)) return;
        var definitions = outgoing ? OutgoingDamageTypeDataObjects : IncomingDamageTypeDataObjects;
        var other = (outgoing ? action.Victim : action.Attacker).ToUpper();
        foreach (var key in categories)
        {
            var category = Items[key];
            ModAlly(other, definitions[category.Type!].AllyValue);
            category.AddCombatAction(action, "All");
            if (!ActGlobals.restrictToAll) category.AddCombatAction(action, action.AttackType);
        }
        var all = outgoing ? outAll! : incAll!;
        all.AddCombatAction(action, "All");
        all.AddCombatAction(action, action.AttackType);
    }

    public void ModAlly(string name, int modifier)
    {
        if (Name == "Unknown" || name == "UNKNOWN") return;
        if (!Allies.ContainsKey(name)) Allies.Add(name, 0);
        if (modifier != 0)
        {
            Allies[name] = unchecked(Allies[name] + modifier);
            Parent.SetAlliesUncached();
        }
    }

    public DateTime StartTime
    {
        get { if (!startCached) { cachedStart = outAll!.StartTime; startCached = true; } return cachedStart; }
    }
    public DateTime EndTime
    {
        get { if (!endCached) { cachedEnd = outAll!.EndTime; endCached = true; } return cachedEnd; }
    }
    public DateTime ShortEndTime => Items[DamageTypeDataOutgoingDamage].EndTime;
    public DateTime EncStartTime => Parent.StartTime;
    public DateTime EncEndTime => Parent.EndTime;
    public long Damage => Items[DamageTypeDataOutgoingDamage].Damage;
    public long Healed => Items[DamageTypeDataOutgoingHealing].Damage;
    public long DamageTaken => Items[DamageTypeDataIncomingDamage].Damage;
    public long HealsTaken => Items[DamageTypeDataIncomingHealing].Damage;
    public int Swings => Items[DamageTypeDataOutgoingDamage].Swings;
    public int Hits => Items[DamageTypeDataOutgoingDamage].Hits;
    public int CritHits => Items[DamageTypeDataOutgoingDamage].CritHits;
    public int Heals => Items[DamageTypeDataOutgoingHealing].Swings;
    public int Misses => Items[DamageTypeDataOutgoingDamage].Misses;
    public int Blocked => Items[DamageTypeDataOutgoingDamage].Blocked;
}
