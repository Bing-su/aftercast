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
        var duration = TimeSpan.Zero;
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
                var time = actions[index].Time;
                var openLast = segment == starts.Count - 1 && ends.Count + 1 == starts.Count;
                if (time >= starts[segment] && time <= (openLast ? encounter.EndTime : ends[segment]))
                {
                    var last = actions[index];
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
                    duration += last.Time - time;
                    break;
                }
                if (segment < ends.Count && actions[index].Time > ends[segment]) break;
            }
        }
        return duration;
    }
}
