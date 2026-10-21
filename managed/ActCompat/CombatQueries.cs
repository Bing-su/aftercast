using System.Runtime.CompilerServices;

namespace Aftercast.ActCompat
{
    public enum EncounterStatus { Active = 0, Ended = 1, Importing = 2, PartialImport = 3 }
    public sealed record EncounterSummaryData(ulong Id, bool IsImport, EncounterStatus Status,
        string Zone, string Title, string Character, DateTime Start, DateTime End,
        long DurationTicks, long Damage, long Healed, int Combatants);
    public sealed record EncounterDetailData(EncounterSummaryData Summary, CombatantSnapshotData[] Combatants);
    public sealed record AttackSummaryData(string Combatant, string Category, string Attack, bool Outgoing,
        long Damage, long DurationTicks, double DPS, double EncDPS, int Swings, int Hits, int CritHits, int Misses, int Blocked);
    public sealed record CombatPage<T>(int Total, T[] Items);
}

namespace Advanced_Combat_Tracker
{
    using Aftercast.ActCompat;

    public partial class FormActMain
    {
        private sealed class EncounterIdentity(ulong id)
        {
            internal readonly ulong Id = id;
            internal CombatSnapshotStatus Status = CombatSnapshotStatus.Ended;
        }
        // Weak keys keep query IDs stable without retaining graphs removed by plugins, e.g. cleared history.
        private readonly ConditionalWeakTable<EncounterData, EncounterIdentity> encounterIds = new();
        private ulong nextEncounterId;
        private EncounterIdentity Identity(EncounterData encounter) => encounterIds.GetValue(encounter,
            _ => new(checked(++nextEncounterId)));

        private IEnumerable<(EncounterData Encounter, bool Import)> QueryEncounters()
        {
            var seen = new HashSet<EncounterData>(ReferenceEqualityComparer.Instance);
            foreach (var zone in combatZones)
                for (var index = zone.PopulateAll ? 1 : 0; index < zone.Items.Count; index++)
                    if (seen.Add(zone.Items[index])) yield return (zone.Items[index], ReferenceEquals(zone, combatZones[0]));
        }

        private EncounterSummaryData Summarize(EncounterData encounter, bool import)
        {
            var identity = Identity(encounter);
            var status = identity.Status switch
            {
                CombatSnapshotStatus.Importing => EncounterStatus.Importing,
                CombatSnapshotStatus.PartialImport => EncounterStatus.PartialImport,
                _ => encounter.Active ? EncounterStatus.Active : EncounterStatus.Ended
            };
            return new(identity.Id, import, status, encounter.ZoneName, encounter.Title,
                encounter.CharName, encounter.StartTime, encounter.EndTime, encounter.Duration.Ticks,
                encounter.Damage, encounter.Healed, encounter.Items.Count);
        }

        internal CombatPage<EncounterSummaryData> ListEncounters(int offset, int limit)
        {
            ValidateCombatPage(offset, limit);
            lock (CombatGate)
            {
                var total = 0;
                var items = new List<EncounterSummaryData>();
                foreach (var (encounter, import) in QueryEncounters())
                {
                    if (total >= offset && items.Count < limit) items.Add(Summarize(encounter, import));
                    total = checked(total + 1);
                }
                return new(total, items.ToArray());
            }
        }

        internal EncounterDetailData? ReadEncounter(ulong id, bool live)
        {
            lock (CombatGate)
            {
                if (live)
                    return inCombat && !importingLogs && combatZone.ActiveEncounter is { } current
                        ? Detail(current, false) : null;
                foreach (var (encounter, import) in QueryEncounters())
                    if (Identity(encounter).Id == id) return Detail(encounter, import);
                return null;
            }
        }

        private EncounterDetailData Detail(EncounterData encounter, bool import) =>
            new(Summarize(encounter, import), CaptureCombatants(encounter));

        internal CombatPage<AttackSummaryData>? ReadEncounterAttacks(ulong id, int offset, int limit)
        {
            ValidateCombatPage(offset, limit);
            lock (CombatGate)
            {
                var encounter = QueryEncounters().Select(item => item.Encounter).FirstOrDefault(item => Identity(item).Id == id);
                if (encounter is null) return null;
                var total = 0;
                var items = new List<AttackSummaryData>();
                foreach (var actor in encounter.Items.Values)
                    foreach (var category in actor.Items)
                        foreach (var attack in category.Value.Items)
                        {
                            if (total >= offset && items.Count < limit)
                            {
                                var value = attack.Value;
                                items.Add(new(actor.Name, category.Key, attack.Key, category.Value.Outgoing,
                                    value.Damage, value.Duration.Ticks, value.DPS, value.EncDPS,
                                    value.Swings, value.Hits, value.CritHits, value.Misses, value.Blocked));
                            }
                            total = checked(total + 1);
                        }
                return new(total, items.ToArray());
            }
        }

        private void CompleteImportQueries(ZoneData zone, int first, bool success)
        {
            lock (CombatGate)
                foreach (var encounter in zone.Items.Skip(first))
                    Identity(encounter).Status = success ? CombatSnapshotStatus.Ended : CombatSnapshotStatus.PartialImport;
        }

        private static void ValidateCombatPage(int offset, int limit)
        {
            if (offset < 0 || limit is < 1 or > 256) throw new ArgumentOutOfRangeException(nameof(limit), "Expected offset >= 0 and limit 1..256");
        }
    }
}
