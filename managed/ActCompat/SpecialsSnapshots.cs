using System.Text.Json;
using Aftercast.ActCompat;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    private ulong nextSpecialsRevision;
    private (ulong Revision, byte[] Bytes)? specialsSnapshot;

    internal AvoidanceSnapshotInfo? CaptureSpecials(ulong id, string combatant)
    {
        lock (CombatGate)
        {
            var found = QueryEncounters().FirstOrDefault(item => Identity(item.Encounter).Id == id);
            if (found.Encounter is null) return null;
            var encounter = found.Encounter;
            var actor = encounter.GetCombatant(combatant);
            if (actor is null) return null;
            if (encounter.LogLines.Count > EncounterData.MaximumRecordedLogLines)
                throw new InvalidDataException("Special report exceeds log limit");
            ValidateAvoidanceText(combatant); ValidateAvoidanceText(actor.Name); ValidateAvoidanceText(encounter.ZoneName);
            ValidateAvoidanceText(encounter.Title); ValidateAvoidanceText(encounter.CharName);
            var source = actor.AllOut["All"];
            if (source.Items.Count > 16384) throw new InvalidDataException("Special report exceeds 16384 actions");
            var byPair = new DamageTypeData(true, "", actor);
            var bySkill = new DamageTypeData(true, "", actor);
            var bySpecial = new DamageTypeData(true, "", actor);
            var allSkill = new DamageTypeData(true, "", actor);
            foreach (var action in source.Items)
            {
                if (!CombatantData.DamageSwingTypes.Contains(action.SwingType)) continue;
                ValidateAvoidanceText(action.AttackType); ValidateAvoidanceText(action.Special);
                if (action.Special != "None")
                {
                    byPair.AddCombatAction(action, $"{action.AttackType}({action.Special})");
                    bySkill.AddCombatAction(action, action.AttackType);
                    bySpecial.AddCombatAction(action, action.Special);
                }
                allSkill.AddCombatAction(action, action.AttackType);
            }
            if (byPair.Items.Count + bySkill.Items.Count + bySpecial.Items.Count > 4096)
                throw new InvalidDataException("Special report exceeds 4096 groups");
            long comparisonBudget = 0;
            foreach (var attack in allSkill.Items.Values)
            {
                comparisonBudget = checked(comparisonBudget + (long)attack.Items.Count * attack.Items.Count);
                if (comparisonBudget > 4_000_000) throw new InvalidDataException("Special report comparison budget exceeded");
            }
            var specialCounts = new SortedList<string, Dictionary<string, int>>();
            foreach (var (key, attack) in allSkill.Items) specialCounts.Add(key, attack.GetAttackSpecials());
            var sources = SourceIndices(encounter);
            var evidenceCount = 0;
            object[] Evidence(AttackType attack)
            {
                evidenceCount = checked(evidenceCount + attack.Items.Count);
                if (evidenceCount > 49152) throw new InvalidDataException("Special report exceeds evidence limit");
                return attack.Items.Select(action => (object)ValidateAvoidanceAction(ProjectAction(action, sources))).ToArray();
            }
            var rows = new List<object>();
            foreach (var (key, attack) in byPair.Items)
            {
                var index = key.LastIndexOf('(');
                var skill = key[..index];
                var special = key[(index + 1)..^1];
                var total = allSkill.Items[skill];
                var counts = specialCounts[skill];
                counts.TryGetValue("None", out var none);
                rows.Add(new { group = "pair", key, occurrences = (int?)counts[special], swings = attack.Swings,
                    totalSwings = (int?)total.Swings, occurrenceRatio = (float?)((float)counts[special] / none),
                    swingRatio = (float?)((float)attack.Swings / none), totalRate = (double?)total.EncDPS,
                    rate = attack.EncDPS, evidence = Evidence(attack) });
            }
            foreach (var (key, attack) in bySkill.Items)
            {
                var total = allSkill.Items[key];
                var counts = specialCounts[key];
                counts.TryGetValue("None", out var none);
                rows.Add(new { group = "skill", key, occurrences = (int?)counts["ANY"], swings = attack.Swings,
                    totalSwings = (int?)total.Swings, occurrenceRatio = (float?)((float)counts["ONCE"] / none),
                    swingRatio = (float?)((float)attack.Swings / none), totalRate = (double?)total.EncDPS,
                    rate = attack.EncDPS, evidence = Evidence(attack) });
            }
            foreach (var (key, attack) in bySpecial.Items)
                rows.Add(new { group = "special", key, occurrences = (int?)null, swings = attack.Swings,
                    totalSwings = (int?)null, occurrenceRatio = (float?)null, swingRatio = (float?)null,
                    totalRate = (double?)null, rate = attack.EncDPS, evidence = Evidence(attack) });
            var logs = encounter.LogLines.Select(line => {
                ValidateAvoidanceText(line.LogLine);
                return new EncounterLogData(line.Time, line.GlobalTimeSorter, line.Type, line.LogLine,
                    sources.GetValueOrDefault(line.GlobalTimeSorter, -1));
            }).ToArray();
            var report = new { summary = Summarize(encounter, found.Import), requestedCombatant = combatant, combatant = actor.Name,
                droppedLogs = encounter.DroppedLogLines, rows, logs };
            using var output = new AvoidanceBuffer();
            JsonSerializer.Serialize(output, report, AvoidanceJson);
            var bytes = output.ToArray();
            var revision = checked(++nextSpecialsRevision);
            specialsSnapshot = (revision, bytes);
            return new(revision, bytes.Length);
        }
    }

    internal AvoidanceSnapshotChunk ReadSpecials(ulong revision, int offset, int limit)
    {
        if (offset < 0 || limit is < 1 or > 65536) throw new ArgumentOutOfRangeException(nameof(limit));
        lock (CombatGate)
        {
            if (specialsSnapshot is not { } snapshot || snapshot.Revision != revision)
                throw new InvalidOperationException("Special report snapshot is stale");
            var start = Math.Min(offset, snapshot.Bytes.Length);
            var count = Math.Min(limit, snapshot.Bytes.Length - start);
            return new(snapshot.Bytes.Length, snapshot.Bytes.AsSpan(start, count).ToArray());
        }
    }
}
