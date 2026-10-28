using System.Text.Json;
using Aftercast.ActCompat;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    private ulong nextRecastRevision;
    private (ulong Revision, byte[] Bytes)? recastSnapshot;
    private sealed record RecastRequest(string Combatant, string Category, string Attack, bool Outgoing, bool Modifier);
    private static readonly string[] RecastModifierNames = ["Traumatic Swipe", "Traumatischer Greifer", "Coup traumatisant", "トラウマティック・スワイプ", "Травмирующий удар силача"];

    internal AvoidanceSnapshotInfo? CaptureRecast(ulong id, string requestJson)
    {
        var request = JsonSerializer.Deserialize<RecastRequest>(requestJson) ?? throw new InvalidDataException("Missing recast request");
        if (request.Combatant is null || request.Category is null || request.Attack is null) throw new InvalidDataException("Missing recast selection");
        ValidateAvoidanceText(request.Combatant); ValidateAvoidanceText(request.Category); ValidateAvoidanceText(request.Attack);
        lock (CombatGate)
        {
            var found = QueryEncounters().FirstOrDefault(item => Identity(item.Encounter).Id == id);
            if (found.Encounter is null) return null;
            var encounter = found.Encounter;
            var actor = encounter.GetCombatant(request.Combatant);
            if (actor is null || !actor.Items.TryGetValue(request.Category, out var category) || category.Outgoing != request.Outgoing ||
                !category.Items.TryGetValue(request.Attack, out var attack)) return null;
            if (!ReferenceEquals(category.Parent, actor) || !ReferenceEquals(attack.Parent, category))
                throw new InvalidDataException("Recast attack ownership changed");
            if (attack.Items.Count > 16384 || encounter.LogLines.Count > EncounterData.MaximumRecordedLogLines)
                throw new InvalidDataException("Recast snapshot exceeds action or log limit");
            ValidateAvoidanceText(actor.Name); ValidateAvoidanceText(encounter.ZoneName); ValidateAvoidanceText(encounter.Title); ValidateAvoidanceText(encounter.CharName);
            var firstAttacker = attack.Items.Count == 0 ? null : attack.Items[0].Attacker;
            ValidateAvoidanceText(firstAttacker); ValidateAvoidanceText(attack.Type);
            var actions = new List<MasterSwing>(attack.Items);
            actions.Sort(static (left, right) => { var order = left.TimeSorter.CompareTo(right.TimeSorter); return order != 0 ? order : left.Time.CompareTo(right.Time); });
            AttackType? modifier = null;
            if (request.Modifier && actor.AllInc.Count > 16384) throw new InvalidDataException("Recast exceeds incoming attack group limit");
            if (request.Modifier)
                foreach (var incoming in actor.AllInc.Values)
                    if (RecastModifierNames.Contains(incoming.Type)) modifier = incoming;
            if (modifier is not null && modifier.Items.Count > 16384) throw new InvalidDataException("Recast modifier exceeds action limit");
            var sources = SourceIndices(encounter);
            long work = 0;
            var rows = new List<object>();
            foreach (var action in actions)
            {
                bool? applied = modifier is null ? null : false;
                MasterSwing? evidence = null;
                if (modifier is not null)
                    foreach (var swipe in modifier.Items)
                    {
                        if (++work > 4_000_000) throw new InvalidDataException("Recast modifier work limit exceeded");
                        if (swipe.Damage.Number <= -1 || action.Time <= swipe.Time || action.Time >= swipe.Time.AddSeconds(31)) continue;
                        var active = true;
                        var attacker = encounter.GetCombatant(swipe.Attacker);
                        if (attacker is not null)
                        {
                            if (!attacker.AllInc.TryGetValue("Killing", out var deaths)) attacker.AllInc.TryGetValue("All", out deaths);
                            if (deaths is not null)
                                foreach (var death in deaths.Items)
                                {
                                    if (++work > 4_000_000) throw new InvalidDataException("Recast modifier work limit exceeded");
                                    if (death.Damage == Dnum.Death && death.Time >= swipe.Time && death.Time < action.Time) active = false;
                                }
                        }
                        if (!active) continue;
                        applied = true; evidence = swipe; break;
                    }
                rows.Add(new { ticks = action.Time.Ticks, action = ValidateAvoidanceAction(ProjectAction(action, sources)), modified = applied,
                    modifierEvidence = evidence is null ? null : ValidateAvoidanceAction(ProjectAction(evidence, sources)) });
            }
            var logs = encounter.LogLines.Select(line => { ValidateAvoidanceText(line.LogLine); return new EncounterLogData(line.Time,
                line.GlobalTimeSorter, line.Type, line.LogLine, sources.GetValueOrDefault(line.GlobalTimeSorter, -1)); }).ToArray();
            var report = new { summary = Summarize(encounter, found.Import), request, combatant = actor.Name, attackName = attack.Type, firstAttacker,
                droppedLogs = encounter.DroppedLogLines, rows, logs };
            using var output = new AvoidanceBuffer();
            JsonSerializer.Serialize(output, report, AvoidanceJson);
            var bytes = output.ToArray();
            var revision = checked(++nextRecastRevision);
            recastSnapshot = (revision, bytes);
            return new(revision, bytes.Length);
        }
    }
    internal AvoidanceSnapshotChunk ReadRecast(ulong revision, int offset, int limit)
    {
        if (offset < 0 || limit is < 1 or > 65536) throw new ArgumentOutOfRangeException(nameof(limit));
        lock (CombatGate)
        {
            if (recastSnapshot is not { } snapshot || snapshot.Revision != revision) throw new InvalidOperationException("Recast snapshot is stale");
            var start = Math.Min(offset, snapshot.Bytes.Length);
            return new(snapshot.Bytes.Length, snapshot.Bytes.AsSpan(start, Math.Min(limit, snapshot.Bytes.Length - start)).ToArray());
        }
    }
}
