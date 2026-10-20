namespace Advanced_Combat_Tracker;

internal static class PersonalDuration
{
    internal static TimeSpan Calculate(EncounterData encounter, List<MasterSwing> items, bool combatant)
    {
        var actions = new List<MasterSwing>(items);
        // ACT's CompareTime orders the sequence number first, even when timestamps arrive out of order.
        actions.Sort(static (left, right) =>
        {
            var order = left.TimeSorter.CompareTo(right.TimeSorter);
            return order != 0 ? order : left.Time.CompareTo(right.Time);
        });
        var starts = encounter.StartTimes;
        var ends = encounter.EndTimes;
        var personalStarts = new List<DateTime>();
        var personalEnds = new List<DateTime>();
        var cursor = 0;
        // CombatantData bounds its scan using a copied end list with one open end appended.
        var segments = combatant ? ends.Count + (starts.Count > ends.Count ? 1 : 0) : starts.Count;
        if (combatant && starts.Count > ends.Count) _ = encounter.EndTime;
        for (var segment = 0; segment < segments; segment++)
        {
            if (cursor < 0) cursor = 0;
            if (segment < ends.Count && ends[segment] < actions[cursor].Time) continue;
            for (var index = cursor; index < actions.Count; index++)
            {
                if (personalStarts.Count == personalEnds.Count)
                {
                    var time = actions[index].Time;
                    var openLast = segment == starts.Count - 1 && ends.Count + 1 == starts.Count;
                    if (time >= starts[segment] && time <= (openLast ? encounter.EndTime : ends[segment]))
                    {
                        personalStarts.Add(time);
                        cursor = index;
                    }
                }
                if (personalStarts.Count - 1 == personalEnds.Count)
                {
                    MasterSwing? last = null;
                    for (var end = index; end < actions.Count; end++)
                    {
                        last = actions[end];
                        if (end + 1 == actions.Count ||
                            starts.Count > segment + 1 && actions[end + 1].Time >= starts[segment + 1])
                        {
                            cursor = end - 1;
                            break;
                        }
                    }
                    // Deliberately use the last action before the next segment, not a clipped wall-clock end.
                    personalEnds.Add(last!.Time);
                    break;
                }
                if (segment < ends.Count && actions[index].Time > ends[segment]) break;
            }
        }
        if (personalStarts.Count - 1 == personalEnds.Count) personalEnds.Add(actions[^1].Time);
        if (personalStarts.Count != personalEnds.Count)
            throw new Exception(string.Format("Personal Duration failure.  StartTimes: {0}/{2} EndTimes: {1}/{3}",
                personalStarts.Count, personalEnds.Count, starts.Count, ends.Count));
        var duration = TimeSpan.Zero;
        for (var index = 0; index < personalStarts.Count; index++) duration += personalEnds[index] - personalStarts[index];
        return duration;
    }
}

public partial class AttackType
{
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
}

public partial class DamageTypeData
{
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
}

public partial class CombatantData
{
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
}
