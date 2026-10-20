namespace Advanced_Combat_Tracker;

public partial class EncounterData
{
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
        var enemies = Items.Values.Where(actor => !allies.Contains(actor)).ToList();
        var ranked = new List<(string Name, double Score)>();
        foreach (var enemy in enemies)
        {
            // ACT divides integer damage before rounding the score through a single-precision float.
            var score = enemy.Deaths > 0 ? (float)(enemy.DamageTaken / enemy.Deaths) : (float)enemy.DamageTaken;
            ranked.Add((enemy.Name, score));
        }
        ranked.Sort(static (left, right) => left.Score.CompareTo(right.Score));
        ranked.Reverse();
        ActGlobals.oFormActMain?.WriteDebugLog(string.Format("GetStrongestEnemy: {0:0.00}", timer.Elapsed.TotalMilliseconds));
        return ranked.Count == 0 ? null : ranked[0].Name;
    }
}

public partial class CombatantData : IEquatable<CombatantData>
{
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
}

public partial class DamageTypeData
{
    public void Trim()
    {
        Items.TrimExcess();
        foreach (var attack in Items.Values) attack.Trim();
    }
}

public partial class AttackType
{
    public void Trim() => Items.TrimExcess();
}
