using Advanced_Combat_Tracker;

namespace Aftercast.ActCompat;

/// One ACT outcome for a single already-projected damage category.
public sealed record AvoidanceOutcomeData(string Label, long Amount, int[] Evidence);

public static class AvoidanceOutcomes
{
    // The caller holds CombatGate and owns the returned graph references until projection.
    // ACT deduplicates category names but applies every outgoing link to each incoming row.
    public static Dictionary<string, List<MasterSwing>> ProjectCategories(CombatantData combatant, string allTerm)
    {
        ArgumentNullException.ThrowIfNull(combatant);
        ArgumentNullException.ThrowIfNull(allTerm);
        var links = CombatantData.SwingTypeToDamageTypeDataLinksOutgoing;
        if (CombatantData.DamageSwingTypes.Count > 4096) throw new InvalidDataException("Avoidance projection exceeds 4096 registered swings");
        var registered = CombatantData.DamageSwingTypes.ToHashSet();
        var categories = new Dictionary<string, List<MasterSwing>>(StringComparer.Ordinal);
        var linkCount = 0;
        foreach (var type in CombatantData.DamageSwingTypes)
        {
            if (!links.TryGetValue(type, out var names)) throw new InvalidDataException($"Missing outgoing categories for damage swing {type}");
            foreach (var name in names)
            {
                if (++linkCount > 16384) throw new InvalidDataException("Avoidance projection exceeds 16384 category links");
                if (!categories.ContainsKey(name))
                {
                    if (categories.Count == 1024) throw new InvalidDataException("Avoidance projection exceeds 1024 categories");
                    categories.Add(name, new());
                }
            }
        }
        var count = 0;
        foreach (var attack in combatant.AllInc.Values)
        {
            if (attack.Type == allTerm) continue;
            foreach (var action in attack.Items)
            {
                if (!registered.Contains(action.SwingType) || action.Damage == Dnum.Death) continue;
                foreach (var name in links[action.SwingType])
                {
                    // The original inserts under All and then AttackType. If a plugin
                    // mutates the latter to the All key, the All list receives it twice.
                    var copies = action.AttackType == allTerm ? 2 : 1;
                    for (var copy = 0; copy < copies; copy++)
                    {
                        if (++count > 16384) throw new InvalidDataException("Avoidance category projection exceeds 16384 rows");
                        categories[name].Add(action);
                    }
                }
            }
        }
        return categories;
    }

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
