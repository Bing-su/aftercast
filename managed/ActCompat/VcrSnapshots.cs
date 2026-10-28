using System.Text.Json;
using Aftercast.ActCompat;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    private ulong nextVcrRevision;
    private (ulong Revision, byte[] Bytes)? vcrSnapshot;

    internal AvoidanceSnapshotInfo? CaptureVcr(ulong id)
    {
        lock (CombatGate)
        {
            var found = QueryEncounters().FirstOrDefault(item => Identity(item.Encounter).Id == id);
            if (found.Encounter is null) return null;
            var encounter = found.Encounter;
            if (encounter.Items.Count > 512 || encounter.LogLines.Count > EncounterData.MaximumRecordedLogLines)
                throw new InvalidDataException("VCR exceeds actor or log limit");
            var summary = Summarize(encounter, found.Import);
            var seconds = encounter.Duration.TotalSeconds;
            if (!double.IsFinite(seconds) || seconds < 0 || seconds > 360000 || encounter.StartTime == DateTime.MaxValue)
                throw new InvalidDataException("VCR encounter time is unavailable or exceeds limit");
            ValidateAvoidanceText(encounter.ZoneName); ValidateAvoidanceText(encounter.Title); ValidateAvoidanceText(encounter.CharName);
            var allies = encounter.GetAllies();
            var categories = CombatantData.OutgoingDamageTypeDataObjects.Keys.ToArray();
            if (categories.Length > 1024) throw new InvalidDataException("VCR exceeds category limit");
            foreach (var name in categories) ValidateAvoidanceText(name);
            if (CombatantData.DamageSwingTypes.Count > 4096 || CombatantData.HealingSwingTypes.Count > 4096)
                throw new InvalidDataException("VCR exceeds registration limit");
            var damage = CombatantData.DamageSwingTypes.ToArray(); var healing = CombatantData.HealingSwingTypes.ToArray();
            var sources = SourceIndices(encounter);
            var copied = 0;
            object[] Copy(IEnumerable<MasterSwing> actions)
            {
                var rows = new List<object>();
                foreach (var action in actions)
                {
                    if (++copied > 65536) throw new InvalidDataException("VCR exceeds 65536 copied action rows");
                    rows.Add(new { ticks = action.Time.Ticks, action = ValidateAvoidanceAction(ProjectAction(action, sources)),
                        isDeath = action.Damage == Dnum.Death });
                }
                return rows.ToArray();
            }
            var actors = new List<object>();
            foreach (var actor in encounter.Items.Values)
            {
                ValidateAvoidanceText(actor.Name);
                var allied = allies.Contains(actor);
                var type = 0;
                if (allied)
                {
                    var outgoingDamage = actor.Items[CombatantData.DamageTypeDataOutgoingDamage].Damage;
                    var outgoingHealing = actor.Items[CombatantData.DamageTypeDataOutgoingHealing].Damage;
                    var nonSkill = actor.Items[CombatantData.DamageTypeDataNonSkillDamage].Damage;
                    var incomingHealing = actor.Items[CombatantData.DamageTypeDataIncomingHealing].Damage;
                    type = incomingHealing > outgoingDamage / 3 && outgoingDamage > outgoingHealing ? 1 :
                        outgoingHealing > outgoingDamage / 3 && outgoingHealing > incomingHealing ? 2 : nonSkill > outgoingDamage / 10 ? 3 : 4;
                }
                var incomingPresent = actor.AllInc.TryGetValue("All", out var inc);
                var incoming = incomingPresent ? Copy(inc!.Items) : [];
                var outgoing = actor.AllOut.TryGetValue("All", out var output) ? Copy(output.Items) : [];
                var grouped = new List<object>();
                foreach (var name in categories)
                    if (actor.Items.TryGetValue(name, out var category) && category.Items.TryGetValue("All", out var attack))
                        grouped.Add(new { name, actions = Copy(attack.Items) });
                actors.Add(new { actor.Name, allied, type, damageTaken = actor.DamageTaken, healsTaken = actor.HealsTaken, incomingPresent, incoming, outgoing, categories = grouped });
            }
            var logs = encounter.LogLines.Select(line => { ValidateAvoidanceText(line.LogLine); return new EncounterLogData(line.Time,
                line.GlobalTimeSorter, line.Type, line.LogLine, sources.GetValueOrDefault(line.GlobalTimeSorter, -1)); }).ToArray();
            var report = new { summary, startTicks = encounter.StartTime.Ticks, duration = (int)seconds, damageTypes = damage,
                healingTypes = healing, categories, droppedLogs = encounter.DroppedLogLines, actors, logs };
            using var buffer = new AvoidanceBuffer(); JsonSerializer.Serialize(buffer, report, AvoidanceJson);
            var bytes = buffer.ToArray(); var revision = checked(++nextVcrRevision); vcrSnapshot = (revision, bytes);
            return new(revision, bytes.Length);
        }
    }
    internal AvoidanceSnapshotChunk ReadVcr(ulong revision, int offset, int limit)
    {
        if (offset < 0 || limit is < 1 or > 65536) throw new ArgumentOutOfRangeException(nameof(limit));
        lock (CombatGate)
        {
            if (vcrSnapshot is not { } snapshot || snapshot.Revision != revision) throw new InvalidOperationException("VCR snapshot is stale");
            var start = Math.Min(offset, snapshot.Bytes.Length);
            return new(snapshot.Bytes.Length, snapshot.Bytes.AsSpan(start, Math.Min(limit, snapshot.Bytes.Length - start)).ToArray());
        }
    }
}
