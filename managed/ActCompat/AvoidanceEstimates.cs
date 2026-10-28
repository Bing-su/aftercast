using Advanced_Combat_Tracker;

namespace Aftercast.ActCompat;

/// Estimate and exact sample positions in the selected attacker pool.
public sealed record AvoidanceEstimateData(double Estimate, bool UsedFallback, bool HasSampleAverage,
    string SamplePool, int[] MatchingSamples, int PoolCount, MasterSwing[] SampleRows);

public static class AvoidanceEstimates
{
    // Caller owns CombatGate and the plugin owner thread. Retains original pool
    // Hits caches; only the victim-filtered temporary AttackType starts fresh.
    public static AvoidanceEstimateData Calculate(EncounterData encounter, MasterSwing failed, string allTerm)
    {
        ArgumentNullException.ThrowIfNull(encounter);
        ArgumentNullException.ThrowIfNull(failed);
        ArgumentNullException.ThrowIfNull(allTerm);
        var originalBlock = ActGlobals.blockIsHit;
        ActGlobals.blockIsHit = false;
        try
        {
            var attacker = encounter.GetCombatant(failed.Attacker) ?? throw new InvalidDataException("Avoidance attacker not found");
            if (!CombatantData.SwingTypeToDamageTypeDataLinksOutgoing.TryGetValue(failed.SwingType, out var links))
                throw new InvalidDataException("Avoidance swing category mapping not found");
            AttackType? pool;
            string name;
            if (links.Contains(CombatantData.DamageTypeDataNonSkillDamage))
            {
                name = CombatantData.DamageTypeDataNonSkillDamage;
                if (!attacker.Items.TryGetValue(name, out var category) || !category.Items.TryGetValue(allTerm, out pool))
                    throw new InvalidDataException("Avoidance non-skill sample pool not found");
            }
            else
            {
                name = failed.AttackType;
                if (!attacker.AllOut.TryGetValue(name, out pool))
                    throw new InvalidDataException("Avoidance attack sample pool not found");
            }
            if (pool.Items.Count > 16384) throw new InvalidDataException("Avoidance sample pool exceeds 16384 rows");
            var local = new AttackType(pool.Type, pool.Parent);
            var matches = new List<int>();
            for (var index = 0; index < pool.Items.Count; index++)
            {
                var sample = pool.Items[index];
                // ACT checks the failed swing's membership, not the sample swing.
                if (sample.Victim == failed.Victim && CombatantData.DamageSwingTypes.Contains(failed.SwingType))
                {
                    local.AddCombatAction(sample);
                    matches.Add(index);
                }
            }
            var estimate = local.Average;
            var fallback = double.IsNaN(estimate);
            if (fallback) estimate = pool.Average;
            var available = !double.IsNaN(estimate);
            if (!available) estimate = 0;
            var sampleRows = fallback ? pool.Items.ToArray() : matches.Select(index => pool.Items[index]).ToArray();
            return new(estimate, fallback, available, name, matches.ToArray(), pool.Items.Count, sampleRows);
        }
        finally { ActGlobals.blockIsHit = originalBlock; }
    }
}
