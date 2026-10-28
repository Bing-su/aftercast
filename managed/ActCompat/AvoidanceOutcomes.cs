using Advanced_Combat_Tracker;

namespace Aftercast.ActCompat;

/// One ACT outcome for a single already-projected damage category.
public sealed record AvoidanceOutcomeData(string Label, long Amount, int[] Evidence);

public static class AvoidanceOutcomes
{
    // Call on the owner thread: Dnum equality and its labels can be plugin-defined.
    // Category routing and sample estimates are intentionally separate.
    public static AvoidanceOutcomeData[] GroupCategory(IReadOnlyList<MasterSwing> actions, IReadOnlyCollection<int> damageSwingTypes)
    {
        ArgumentNullException.ThrowIfNull(actions);
        ArgumentNullException.ThrowIfNull(damageSwingTypes);
        if (actions.Count > 16384) throw new InvalidDataException("Avoidance category exceeds 16384 actions");
        var unique = new SortedList<string, Dnum>();
        var accepted = new List<(int Index, MasterSwing Action)>();
        for (var i = 0; i < actions.Count; i++)
        {
            var action = actions[i];
            if (!damageSwingTypes.Contains(action.SwingType) || action.Damage == Dnum.Death) continue;
            accepted.Add((i, action));
            if (action.Damage.Number < 1 && !unique.ContainsKey(action.Damage.DamageString))
                unique.Add(action.Damage.DamageString, action.Damage);
        }
        if ((long)unique.Count * accepted.Count > 4_000_000)
            throw new InvalidDataException("Avoidance category exceeds 4000000 comparisons");
        return unique.Select(pair => new AvoidanceOutcomeData(pair.Key, pair.Value.Number,
            accepted.Where(item => item.Action.Damage == pair.Value).Select(item => item.Index).ToArray())).ToArray();
    }
}
