namespace Advanced_Combat_Tracker;

public partial class CombatantData
{
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
}

public partial class EncounterData
{
    public int AlliedKills => AggregateActors().Aggregate(0, static (total, actor) => unchecked(total + actor.Kills));
    // Preserve ACT's literal-space exclusion rather than inventing a game-specific player/NPC classifier.
    public int AlliedDeaths => AggregateActors().Aggregate(0, static (total, actor) =>
        actor.Name.Contains(" ") ? total : unchecked(total + actor.Deaths));
    public double DPS => Damage / Duration.TotalSeconds;
}
