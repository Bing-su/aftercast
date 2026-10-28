using System.Text.Json;
using System.Text.Json.Serialization;
using Aftercast.ActCompat;

namespace Aftercast.ActCompat
{
    public sealed record AvoidanceSnapshotInfo(ulong Revision, int ByteLength);
    public sealed record AvoidanceSnapshotChunk(int Total, byte[] Bytes);
}

namespace Advanced_Combat_Tracker
{
    public partial class FormActMain
    {
        private ulong nextAvoidanceRevision;
        private (ulong Revision, byte[] Bytes)? avoidanceSnapshot;
        private const int AvoidanceSnapshotByteLimit = 64 * 1024 * 1024;
        private static readonly System.Text.UTF8Encoding AvoidanceUtf8 = new(false, true);
        private static void ValidateAvoidanceText(string? value) { if (value is not null) _ = AvoidanceUtf8.GetByteCount(value); }
        private static EncounterActionData ValidateAvoidanceAction(EncounterActionData action)
        {
            ValidateAvoidanceText(action.Attacker); ValidateAvoidanceText(action.Victim); ValidateAvoidanceText(action.Attack);
            ValidateAvoidanceText(action.Damage); ValidateAvoidanceText(action.Special); ValidateAvoidanceText(action.DamageType);
            ValidateAvoidanceText(action.RawLabel); ValidateAvoidanceText(action.RawSuffix);
            return action;
        }
        private static readonly JsonSerializerOptions AvoidanceJson = new() { NumberHandling = JsonNumberHandling.AllowNamedFloatingPointLiterals };

        private sealed class AvoidanceBuffer : Stream
        {
            private readonly MemoryStream inner = new();
            public byte[] ToArray() => inner.ToArray();
            public override bool CanRead => false;
            public override bool CanSeek => false;
            public override bool CanWrite => true;
            public override long Length => inner.Length;
            public override long Position { get => inner.Position; set => throw new NotSupportedException(); }
            public override void Flush() => inner.Flush();
            public override void Write(byte[] buffer, int offset, int count) => Write(buffer.AsSpan(offset, count));
            public override void Write(ReadOnlySpan<byte> buffer)
            {
                if (buffer.Length > AvoidanceSnapshotByteLimit - inner.Length)
                    throw new InvalidDataException("Avoidance snapshot exceeds 64 MiB");
                inner.Write(buffer);
            }
            public override int Read(byte[] buffer, int offset, int count) => throw new NotSupportedException();
            public override long Seek(long offset, SeekOrigin origin) => throw new NotSupportedException();
            public override void SetLength(long value) => throw new NotSupportedException();
            protected override void Dispose(bool disposing) { if (disposing) inner.Dispose(); base.Dispose(disposing); }
        }

        internal AvoidanceSnapshotInfo? CaptureCombatantAvoidance(ulong id, string combatant) =>
            CaptureAvoidance(id, combatant ?? throw new ArgumentNullException(nameof(combatant)));

        internal AvoidanceSnapshotInfo? CaptureEncounterAvoidance(ulong id) => CaptureAvoidance(id, null);

        private AvoidanceSnapshotInfo? CaptureAvoidance(ulong id, string? combatant)
        {
            lock (CombatGate)
            {
                var found = QueryEncounters().FirstOrDefault(item => Identity(item.Encounter).Id == id);
                if (found.Encounter is null) return null;
                var encounter = found.Encounter;
                var actor = combatant is null ? null : encounter.GetCombatant(combatant);
                if (combatant is not null && actor is null) return null;
                if (encounter.LogLines.Count > EncounterData.MaximumRecordedLogLines)
                    throw new InvalidDataException("Avoidance snapshot exceeds 65536 log rows");
                var originalBlock = ActGlobals.blockIsHit;
                ActGlobals.blockIsHit = false;
                try
                {
                    ValidateAvoidanceText(encounter.ZoneName); ValidateAvoidanceText(encounter.Title); ValidateAvoidanceText(encounter.CharName);
                    if (actor is not null) ValidateAvoidanceText(actor.Name);
                    var sources = SourceIndices(encounter);
                    var encounterScope = actor is null;
                    var projected = new List<(string Name, List<MasterSwing> Actions, AvoidanceOutcomeData[] Groups)>();
                    if (actor is not null)
                    {
                        foreach (var category in AvoidanceOutcomes.ProjectCategories(actor, "All"))
                            projected.Add((category.Key, category.Value, AvoidanceOutcomes.GroupCategory(category.Value, CombatantData.DamageSwingTypes)));
                    }
                    else
                    {
                        if (encounter.Items.Count > 16384) throw new InvalidDataException("Avoidance encounter exceeds 16384 combatants");
                        var actions = new List<MasterSwing>();
                        var visited = 0;
                        foreach (var person in encounter.Items.Values)
                        {
                            if (person.AllInc.Count > 16384) throw new InvalidDataException("Avoidance encounter exceeds 16384 incoming attack groups");
                            foreach (var attack in person.AllInc.Values)
                            {
                                if (attack.Type == "All") continue;
                                foreach (var action in attack.Items)
                                {
                                    if (++visited > 65536) throw new InvalidDataException("Avoidance encounter exceeds 65536 incoming rows");
                                    if (!CombatantData.DamageSwingTypes.Contains(action.SwingType) || action.Damage == Dnum.Death) continue;
                                    if (actions.Count == 16384) throw new InvalidDataException("Avoidance encounter exceeds 16384 actions");
                                    actions.Add(action);
                                }
                            }
                        }
                        var groups = AvoidanceOutcomes.GroupCategory(actions, CombatantData.DamageSwingTypes);
                        var allies = encounter.GetAllies(true) ?? throw new InvalidDataException("Avoidance ally cache is not available yet; retry after encounter details load");
                        var aliases = allies.Select(person => person.Name).Append("YOU").ToArray();
                        if (aliases.Length > 16385) throw new InvalidDataException("Avoidance encounter exceeds alias limit");
                        if ((long)groups.Length * aliases.Length > 4_000_000) throw new InvalidDataException("Avoidance alias matching budget exceeded");
                        var active = new List<string>();
                        foreach (var group in groups)
                            foreach (var alias in aliases)
                                if (group.Label.Contains(alias, StringComparison.Ordinal))
                                {
                                    if (!active.Contains(alias)) active.Add(alias);
                                    break;
                                }
                        if ((long)groups.Length * active.Count > 4_000_000) throw new InvalidDataException("Avoidance alias grouping budget exceeded");
                        foreach (var alias in active)
                            projected.Add((alias, actions, groups.Where(group => group.Label.Contains(alias, StringComparison.Ordinal)).ToArray()));
                    }
                    var rows = new List<(long Damage, object Row)>();
                    var sampleCount = 0;
                    var evidenceCount = 0;
                    long comparisonWork = 0;
                    foreach (var category in projected)
                    {
                        ValidateAvoidanceText(category.Name);
                        var total = new AttackType("All", null);
                        if (!encounterScope) foreach (var action in category.Actions) total.AddCombatAction(action);
                        var swings = total.Swings;
                        var hits = total.Hits;
                        var outcomeRows = new List<object>();
                        var totalOccurrences = 0;
                        double totalEstimate = 0;
                        foreach (var group in category.Groups)
                        {
                            ValidateAvoidanceText(group.Label);
                            var evidence = new List<object>();
                            double sum = 0;
                            foreach (var index in group.Evidence)
                            {
                                var action = category.Actions[index];
                                if (++evidenceCount > 16384) throw new InvalidDataException("Avoidance snapshot evidence budget exceeded");
                                var estimate = AvoidanceEstimates.Calculate(encounter, action, "All");
                                comparisonWork = checked(comparisonWork + estimate.PoolCount);
                                sampleCount = checked(sampleCount + estimate.SampleRows.Length);
                                if (comparisonWork > 4_000_000 || sampleCount > 16384)
                                    throw new InvalidDataException("Avoidance snapshot sample budget exceeded");
                                sum += estimate.Estimate;
                                ValidateAvoidanceText(estimate.SamplePool);
                                evidence.Add(new { action = ValidateAvoidanceAction(ProjectAction(action, sources)), estimate.Estimate, estimate.UsedFallback,
                                    estimate.HasSampleAverage, estimate.SamplePool, estimate.PoolCount,
                                    samples = estimate.SampleRows.Select(sample => ValidateAvoidanceAction(ProjectAction(sample, sources))).ToArray() });
                            }
                            totalOccurrences = checked(totalOccurrences + group.Evidence.Length);
                            totalEstimate += sum;
                            outcomeRows.Add(new { group.Label, group.Amount, count = group.Evidence.Length,
                                percentNonHits = encounterScope ? (float?)null : (float)group.Evidence.Length / (swings - hits) * 100f,
                                percentSwings = encounterScope ? (float?)null : (float)group.Evidence.Length / swings * 100f,
                                estimatedDamage = sum, estimatedRate = sum / encounter.Duration.TotalSeconds, evidence });
                        }
                        var damage = total.Damage;
                        rows.Add((damage, new { name = category.Name, swings = encounterScope ? (int?)null : swings, hits = encounterScope ? (int?)null : hits, damage, totalOccurrences,
                            percentSwings = encounterScope ? (float?)null : (float)totalOccurrences / swings * 100f, estimatedDamage = totalEstimate,
                            estimatedRate = totalEstimate / encounter.Duration.TotalSeconds, outcomes = outcomeRows }));
                    }
                    // ACT sorts DamageTypeData by damage ascending; equal values have no stable order.
                    if (!encounterScope) rows.Sort((left, right) => left.Damage.CompareTo(right.Damage));
                    var summary = Summarize(encounter, found.Import);
                    var logs = encounter.LogLines.Select(line => {
                        ValidateAvoidanceText(line.LogLine);
                        return new EncounterLogData(line.Time, line.GlobalTimeSorter, line.Type,
                            line.LogLine, sources.GetValueOrDefault(line.GlobalTimeSorter, -1));
                    }).ToArray();
                    var report = new { summary, scope = encounterScope ? "encounter" : "combatant", combatant = actor?.Name, droppedLogs = encounter.DroppedLogLines, categories = rows.Select(row => row.Row).ToArray(), logs };
                    using var output = new AvoidanceBuffer();
                    JsonSerializer.Serialize(output, report, AvoidanceJson);
                    var bytes = output.ToArray();
                    var revision = checked(++nextAvoidanceRevision);
                    avoidanceSnapshot = (revision, bytes);
                    return new(revision, bytes.Length);
                }
                finally { ActGlobals.blockIsHit = originalBlock; }
            }
        }

        internal AvoidanceSnapshotChunk ReadCombatantAvoidance(ulong revision, int offset, int limit)
        {
            if (offset < 0 || limit is < 1 or > 65536) throw new ArgumentOutOfRangeException(nameof(limit));
            lock (CombatGate)
            {
                if (avoidanceSnapshot is not { } snapshot || snapshot.Revision != revision)
                    throw new InvalidOperationException("Avoidance snapshot is stale");
                var start = Math.Min(offset, snapshot.Bytes.Length);
                var count = Math.Min(limit, snapshot.Bytes.Length - start);
                return new(snapshot.Bytes.Length, snapshot.Bytes.AsSpan(start, count).ToArray());
            }
        }
    }
}
