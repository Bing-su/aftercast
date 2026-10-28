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

        internal AvoidanceSnapshotInfo? CaptureCombatantAvoidance(ulong id, string combatant)
        {
            ArgumentNullException.ThrowIfNull(combatant);
            lock (CombatGate)
            {
                var found = QueryEncounters().FirstOrDefault(item => Identity(item.Encounter).Id == id);
                if (found.Encounter is null) return null;
                var encounter = found.Encounter;
                var actor = encounter.GetCombatant(combatant);
                if (actor is null) return null;
                if (encounter.LogLines.Count > EncounterData.MaximumRecordedLogLines)
                    throw new InvalidDataException("Avoidance snapshot exceeds 65536 log rows");
                var originalBlock = ActGlobals.blockIsHit;
                ActGlobals.blockIsHit = false;
                try
                {
                    var sources = SourceIndices(encounter);
                    var categories = AvoidanceOutcomes.ProjectCategories(actor, "All");
                    var rows = new List<(long Damage, object Row)>();
                    var sampleCount = 0;
                    long comparisonWork = 0;
                    foreach (var category in categories)
                    {
                        var total = new AttackType("All", null);
                        foreach (var action in category.Value) total.AddCombatAction(action);
                        var swings = total.Swings;
                        var hits = total.Hits;
                        var outcomeRows = new List<object>();
                        var totalOccurrences = 0;
                        double totalEstimate = 0;
                        foreach (var group in AvoidanceOutcomes.GroupCategory(category.Value, CombatantData.DamageSwingTypes))
                        {
                            var evidence = new List<object>();
                            double sum = 0;
                            foreach (var index in group.Evidence)
                            {
                                var action = category.Value[index];
                                var estimate = AvoidanceEstimates.Calculate(encounter, action, "All");
                                comparisonWork = checked(comparisonWork + estimate.PoolCount);
                                sampleCount = checked(sampleCount + estimate.SampleRows.Length);
                                if (comparisonWork > 4_000_000 || sampleCount > 16384)
                                    throw new InvalidDataException("Avoidance snapshot sample budget exceeded");
                                sum += estimate.Estimate;
                                evidence.Add(new { action = ProjectAction(action, sources), estimate.Estimate, estimate.UsedFallback,
                                    estimate.HasSampleAverage, estimate.SamplePool, estimate.PoolCount,
                                    samples = estimate.SampleRows.Select(sample => ProjectAction(sample, sources)).ToArray() });
                            }
                            totalOccurrences = checked(totalOccurrences + group.Evidence.Length);
                            totalEstimate += sum;
                            outcomeRows.Add(new { group.Label, group.Amount, count = group.Evidence.Length,
                                percentNonHits = (float)group.Evidence.Length / (swings - hits) * 100f,
                                percentSwings = (float)group.Evidence.Length / swings * 100f,
                                estimatedDamage = sum, estimatedRate = sum / encounter.Duration.TotalSeconds, evidence });
                        }
                        var damage = total.Damage;
                        rows.Add((damage, new { name = category.Key, swings, hits, damage, totalOccurrences,
                            percentSwings = (float)totalOccurrences / swings * 100f, estimatedDamage = totalEstimate,
                            estimatedRate = totalEstimate / encounter.Duration.TotalSeconds, outcomes = outcomeRows }));
                    }
                    // ACT sorts DamageTypeData by damage ascending; equal values have no stable order.
                    rows.Sort((left, right) => left.Damage.CompareTo(right.Damage));
                    var summary = Summarize(encounter, found.Import);
                    var logs = encounter.LogLines.Select(line => new EncounterLogData(line.Time, line.GlobalTimeSorter, line.Type,
                        line.LogLine, sources.GetValueOrDefault(line.GlobalTimeSorter, -1))).ToArray();
                    var report = new { summary, combatant = actor.Name, droppedLogs = encounter.DroppedLogLines, categories = rows.Select(row => row.Row).ToArray(), logs };
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
