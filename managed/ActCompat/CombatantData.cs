using System.Drawing;

namespace Advanced_Combat_Tracker;

// These registries retain plugin definitions and callbacks; they do not manufacture combat state.
public class CombatantData : IEquatable<CombatantData>
{
    private DamageTypeData? outAll;
    private DamageTypeData? incAll;

    public CombatantData(string name, EncounterData parent)
    {
        Name = name;
        // ACT keeps the last category object, so registration insertion order is observable.
        foreach (var key in OutgoingDamageTypeDataObjects.Keys)
        {
            outAll = new DamageTypeData(true, key, this);
            Items.Add(key, outAll);
        }
        foreach (var key in IncomingDamageTypeDataObjects.Keys)
        {
            incAll = new DamageTypeData(false, key, this);
            Items.Add(key, incAll);
        }
        Parent = parent;
    }

    public string Name { get; }
    public EncounterData Parent { get; }
    public Dictionary<string, DamageTypeData> Items { get; set; } = new();
    public SortedList<string, int> Allies { get; set; } = new();
    public Dictionary<string, object> Tags { get; set; } = new();
    public SortedList<string, AttackType> AllOut => outAll!.Items;
    public SortedList<string, AttackType> AllInc => incAll!.Items;

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

    public void InvalidateCachedValues() { startCached = endCached = false; InvalidateDurationCache(); deathsCached = false; InvalidateKillsCache(); }
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
        InvalidateDurationCache();
        if (!outgoing && action.Damage == Dnum.Death) deathsCached = false;
        if (outgoing && action.Damage == Dnum.Death) InvalidateKillsCache();
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

    private bool durationCached;
    private TimeSpan cachedDuration;
    internal void InvalidateDurationCache() => durationCached = false;

    public TimeSpan Duration
    {
        get
        {
            if (Parent.StartTimes.Count > 1)
            {
                if (durationCached) return cachedDuration;
                if (!AllOut.TryGetValue("All", out var attack)) return TimeSpan.Zero;
                cachedDuration = PersonalDuration.Calculate(Parent, attack.Items, combatant: true);
                durationCached = true;
                // The original additionally logs elapsed calculation time through its UI root; pure queries need no root here.
                return cachedDuration;
            }
            return EndTime > StartTime ? EndTime - StartTime : TimeSpan.Zero;
        }
    }

    // Preserve unguarded division: unlike AttackType, ACT exposes NaN/infinity at zero duration here.
    public double DPS => Damage / Duration.TotalSeconds;
    public double EncDPS => Damage / Parent.Duration.TotalSeconds;
    public double ExtDPS => EncDPS;
    public double EncHPS { get { var seconds = Parent.Duration.TotalSeconds; return Healed / seconds; } }
    public double ExtHPS => EncHPS;

    private bool deathsCached;
    private int cachedDeaths;

    public bool Equals(CombatantData? other) => Name.ToLower() == other!.Name.ToLower();

    public int Deaths
    {
        get
        {
            if (deathsCached) return cachedDeaths;
            if (AllInc.TryGetValue("Killing", out var killing)) cachedDeaths = killing.Items.Count;
            else if (AllInc.TryGetValue("All", out var all))
            {
                // Preserve ACT's existing counter when invalidating this fallback, including repeated death reads.
                foreach (var action in all.Items)
                    if (action.Damage == Dnum.Death) cachedDeaths = unchecked(cachedDeaths + 1);
            }
            else cachedDeaths = 0;
            deathsCached = true;
            return cachedDeaths;
        }
    }

    public void Trim()
    {
        foreach (var category in Items.Values) category.Trim();
    }

    private bool killsCached;
    private int cachedKills;
    internal void InvalidateKillsCache() => killsCached = false;

    public int Kills
    {
        get
        {
            if (killsCached) return cachedKills;
            // Prefer ACT's verified built-in Killing category; an existing empty category suppresses the All fallback.
            if (!AllOut.TryGetValue("Killing", out var killing)) AllOut.TryGetValue("All", out killing);
            if (killing is null)
            {
                cachedKills = 0;
                killsCached = true;
                return cachedKills;
            }
            var allied = false;
            if (Parent.GetAllies(true) is not null)
                allied = Parent is not null && Parent.GetAllies(true).Contains(this);
            cachedKills = 0;
            for (var index = 0; index < killing.Items.Count; index++)
            {
                var action = killing.Items[index];
                if (action.Damage == Dnum.Death && (allied || !action.Victim.Contains(" ")))
                    cachedKills = unchecked(cachedKills + 1);
            }
            killsCached = true;
            return cachedKills;
        }
    }

    public static Dictionary<string, DamageTypeDef> IncomingDamageTypeDataObjects = new();
    public static Dictionary<string, DamageTypeDef> OutgoingDamageTypeDataObjects = new();
    public static SortedDictionary<int, List<string>> SwingTypeToDamageTypeDataLinksIncoming = new();
    public static SortedDictionary<int, List<string>> SwingTypeToDamageTypeDataLinksOutgoing = new();
    public static List<int> DamageSwingTypes = new();
    public static List<int> HealingSwingTypes = new();
    public static Dictionary<string, TextExportFormatter> ExportVariables = new();
    public class DualComparison(string first, string second) : IComparer<CombatantData>
    {
        public int Compare(CombatantData? left, CombatantData? right)
        {
            var result = ColumnDefs.TryGetValue(first, out var primary) ? primary.SortComparer(left!, right!) : 0;
            if (result == 0 && ColumnDefs.TryGetValue(second, out var secondary)) result = secondary.SortComparer(left!, right!);
            return result != 0 ? result : left!.Damage.CompareTo(right!.Damage);
        }
    }
    public static Dictionary<string, ColumnDef> ColumnDefs = new();
    public string GetColumnByName(string name) => ColumnDefs.TryGetValue(name, out var column) ? column.GetCellData(this) : string.Empty;

    public class DamageTypeDef(string label, int allyValue, Color typeColor)
    {
        public string Label { get; } = label;
        public int AllyValue { get; } = allyValue;
        public Color TypeColor { get; } = typeColor;
    }

    public delegate string ExportStringDataCallback(CombatantData data, string extra);
    public class TextExportFormatter(string name, string label, string description, ExportStringDataCallback callback)
    {
        public string Name { get; } = name;
        public string Label { get; } = label;
        public string Description { get; } = description;
        public ExportStringDataCallback GetExportString = callback;
    }

    public delegate string StringDataCallback(CombatantData data);
    public delegate Color ColorDataCallback(CombatantData data);
    public class ColumnDef(string label, bool defaultVisible, string sqlDataType, string sqlDataName,
        StringDataCallback cell, StringDataCallback sql, Comparison<CombatantData> sort)
    {
        public string Label { get; } = label;
        public bool DefaultVisible { get; } = defaultVisible;
        public string SqlDataType { get; } = sqlDataType;
        public string SqlDataName { get; } = sqlDataName;
        public StringDataCallback GetCellData = cell;
        public StringDataCallback GetSqlData = sql;
        public Comparison<CombatantData> SortComparer = sort;
        public ColorDataCallback GetCellForeColor = static _ => Color.Transparent;
        public ColorDataCallback GetCellBackColor = static _ => Color.Transparent;
    }
}
