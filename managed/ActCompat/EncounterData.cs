using System.Drawing;

namespace Advanced_Combat_Tracker;

public class EncounterData
{
    private string zoneName;
    private string title = "Encounter";
    private List<DateTime> startTimes = new();
    private List<DateTime> endTimes = new();
    // The selective overload applies ACT's attacker/victim list policy, e.g. selected attacker actions retain both sides.
    internal bool SelectiveParsing { get; private set; }
    internal bool IgnoreEnemies { get; private set; }
    // Null means the source did not preserve this policy; it is not evidence of full recording.
    internal bool? SelectiveRecordingEvidence { get; private set; } = false;

    // Private history replay rebuilds already accepted actions before restoring this policy.
    internal void RestoreSelectionPolicy(bool? selective, bool ignoreEnemies)
    {
        if (selective != true && ignoreEnemies) throw new System.Xml.XmlException("IgnoreEnemies requires selective parsing");
        SelectiveRecordingEvidence = selective;
        SelectiveParsing = selective == true;
        IgnoreEnemies = ignoreEnemies;
    }

    public EncounterData(string charName, string zoneName, bool ignoreEnemies, ZoneData parent)
        : this(charName, zoneName, parent)
    {
        SelectiveParsing = true;
        SelectiveRecordingEvidence = true;
        IgnoreEnemies = ignoreEnemies;
    }

    public EncounterData(string charName, string zoneName, ZoneData parent)
    {
        CharName = charName;
        this.zoneName = zoneName;
        Parent = parent;
    }

    private string? cachedEncId;
    // ACT's cached object hash is process-local, never a durable archive/source identity.
    public string EncId => cachedEncId ??= GetHashCode().ToString("x8");
    public string DurationS => Duration.Hours == 0 ? $"{Duration.Minutes:00}:{Duration.Seconds:00}" : $"{Duration.Hours:00}:{Duration.Minutes:00}:{Duration.Seconds:00}";
    public override string ToString()
    {
        if (StartTime == DateTime.MaxValue) return $"{Title} - [{DurationS}]";
        return (DateTime.Now - StartTime).TotalHours > 12
            ? $"{Title} - [{DurationS}] ({StartTime.ToShortDateString()}) {StartTime.ToLongTimeString()}"
            : $"{Title} - [{DurationS}] {StartTime.ToLongTimeString()}";
    }

    public ZoneData Parent { get; set; }
    public string CharName { get; set; }
    // shortcut: these verified built-in English terms need ActGlobals.Trans for custom localization.
    public string ZoneName { get => zoneName == "All" && Parent is not null ? Parent.ZoneName : zoneName; set => zoneName = value; }
    public string Title { get => zoneName == "All" ? "All" : title; set => title = value; }
    public bool Active { get; set; }
    public bool DuplicateDetection { get; set; }
    public HistoryRecord? HistoryRecord { get; set; }
    public SortedList<string, CombatantData> Items { get; set; } = new();
    public List<LogLineEntry> LogLines { get; set; } = new();
    internal bool LogSourceIdentityKnown { get; set; } = true;
    internal bool ClipboardImport { get; set; }
    public long DroppedLogLines { get; internal set; }
    internal const int MaximumRecordedLogLines = 65536;
    internal const int MaximumRecordedLogCharacters = 4 * 1024 * 1024;
    private int recordedLogCharacters;

    internal void RetainLog(LogLineEntry line)
    {
        // Keep the earliest source context and report every omission; never block parsing on a full log store.
        if (LogLines.Count >= MaximumRecordedLogLines || line.LogLine.Length > MaximumRecordedLogCharacters - recordedLogCharacters)
        { DroppedLogLines = checked(DroppedLogLines + 1); return; }
        LogLines.Add(line);
        recordedLogCharacters += line.LogLine.Length;
    }
    public Dictionary<string, object> Tags { get; set; } = new();
    public int NumCombatants => Items.Count;
    public List<DateTime> StartTimes
    {
        // ACT removes unset sentinels on read while returning the same caller-mutable list.
        get { while (startTimes.Remove(DateTime.MaxValue)) { } return startTimes; }
        set => startTimes = value;
    }
    public List<DateTime> EndTimes
    {
        get { while (endTimes.Remove(DateTime.MinValue)) { } return endTimes; }
        set => endTimes = value;
    }

    private readonly HashSet<int> includedTimeSorters = new();
    private List<CombatantData>? selectedAllies;
    private bool alliesManual;
    private bool alliesCached;
    private DateTime alliesLastCall = DateTime.Now;

    internal void EnsureActionSupported()
    {
        if (SelectiveParsing && ActGlobals.oFormActMain is null)
            throw new NotSupportedException("Selective combat parsing requires the host's actual selection policy.");
    }

    public void AddCombatAction(MasterSwing action)
    {
        EnsureActionSupported();
        if (DuplicateDetection && !includedTimeSorters.Add(action.TimeSorter)) return;
        action.ParentEncounter = this;
        if (SelectiveParsing && !ActGlobals.oFormActMain!.SelectiveListGetSelected(action.Attacker)
            && (IgnoreEnemies || !ActGlobals.oFormActMain.SelectiveListGetSelected(action.Victim))) return;
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
        if (alliesCached || allowLimited && DateTime.Now.Second == alliesLastCall.Second ||
            selectedAllies is not null && Active && Title == "All") return selectedAllies!;
        if (IgnoreEnemies) return new(Items.Values);
        var timer = System.Diagnostics.Stopwatch.StartNew();
        var seed = GetCombatant(CharName);
        if (seed is null) return new();
        var graph = new SortedList<string, AllyNode> { [seed.Name.ToUpper()] = new(seed) };
        bool added;
        do
        {
            added = false;
            // Preserve ACT's index order: adding an earlier key can change the current node mid-pass.
            for (var index = 0; index < graph.Count; index++)
                for (var relation = 0; relation < graph.Values[index].Actor.Allies.Count; relation++)
                {
                    var name = graph.Values[index].Actor.Allies.Keys[relation];
                    var modifier = graph.Values[index].Actor.Allies.Values[relation];
                    if (!graph.ContainsKey(name))
                    {
                        var actor = GetCombatant(name);
                        if (actor is null) continue;
                        graph.Add(name, new(actor));
                        added = true;
                    }
                    graph[name].Weight = graph.Values[index].Weight > 0
                        ? unchecked(graph[name].Weight + modifier)
                        : unchecked(graph[name].Weight - modifier);
                }
        } while (added);
        var negative = graph[seed.Name.ToUpper()].Weight < 0;
        selectedAllies = graph.Values.Where(node => negative ? node.Weight < 0 : node.Weight > 0)
            .Select(node => node.Actor).ToList();
        alliesCached = true;
        alliesLastCall = DateTime.Now;
        timer.Stop();
        // Keep real diagnostics when hosted, while standalone graph calculations need no UI root.
        ActGlobals.oFormActMain?.WriteDebugLog("GetAllies: " + timer.ElapsedMilliseconds.ToString("F"));
        return selectedAllies;
    }

    private sealed class AllyNode(CombatantData actor)
    {
        internal CombatantData Actor { get; } = actor;
        internal int Weight;
    }

    private ICollection<CombatantData> AggregateActors() => IgnoreEnemies ? Items.Values : GetAllies();
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
            if (actors.Count == 0) actors = Items.Values;
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
        lock (ActGlobals.actionDataLock)
        {
            Active = false;
            var start = StartTimes[EndTimes.Count];
            var end = EndTime;
            EndTimes.Add(start < end ? end : start);
            if (finalize)
            {
                Trim();
                Title = GetStrongestEnemy(ActGlobals.charName!)!;
            }
        }
    }

    public bool GetIsSelective() => SelectiveParsing;

    public void Trim()
    {
        Items.TrimExcess();
        foreach (var combatant in Items.Values) combatant.Trim();
    }

    public string? GetStrongestEnemy(string charName)
    {
        var timer = System.Diagnostics.Stopwatch.StartNew();
        // The original signature accepts a name, but selection uses the encounter's configured ally graph.
        if (SelectiveParsing && IgnoreEnemies) return "Encounter";
        var allies = GetAllies();
        if (allies.Count == 0) return "Encounter";
        var ranked = new List<(string Name, double Score)>();
        foreach (var enemy in Items.Values)
        {
            if (allies.Contains(enemy)) continue;
            // ACT divides integer damage before rounding the score through a single-precision float.
            var score = enemy.Deaths > 0 ? (float)(enemy.DamageTaken / enemy.Deaths) : (float)enemy.DamageTaken;
            ranked.Add((enemy.Name, score));
        }
        ranked.Sort(static (left, right) => left.Score.CompareTo(right.Score));
        ranked.Reverse();
        ActGlobals.oFormActMain?.WriteDebugLog(string.Format("GetStrongestEnemy: {0:0.00}", timer.Elapsed.TotalMilliseconds));
        return ranked.Count == 0 ? null : ranked[0].Name;
    }

    public int AlliedKills => AggregateActors().Aggregate(0, static (total, actor) => unchecked(total + actor.Kills));
    // Preserve ACT's literal-space exclusion rather than inventing a game-specific player/NPC classifier.
    public int AlliedDeaths => AggregateActors().Aggregate(0, static (total, actor) =>
        actor.Name.Contains(" ") ? total : unchecked(total + actor.Deaths));
    public double DPS => Damage / Duration.TotalSeconds;

    public static Dictionary<string, TextExportFormatter> ExportVariables = new();
    public static Dictionary<string, ColumnDef> ColumnDefs = new();
    public string GetColumnByName(string name) => ColumnDefs.TryGetValue(name, out var column) ? column.GetCellData(this) : string.Empty;
    public delegate string ExportStringDataCallback(EncounterData data, List<CombatantData> allies, string extra);
    public class TextExportFormatter(string name, string label, string description, ExportStringDataCallback callback)
    {
        public string Name { get; } = name;
        public string Label { get; } = label;
        public string Description { get; } = description;
        public ExportStringDataCallback GetExportString = callback;
    }

    public delegate string StringDataCallback(EncounterData data);
    public delegate Color ColorDataCallback(EncounterData data);
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
}
