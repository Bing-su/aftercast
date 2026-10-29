using System.Runtime.CompilerServices;

namespace Aftercast.ActCompat
{
    public enum EncounterStatus { Active = 0, Ended = 1, Importing = 2, PartialImport = 3 }
    public sealed record EncounterSummaryData(ulong Id, bool IsImport, EncounterStatus Status,
        string Zone, string Title, string Character, DateTime Start, DateTime End,
        long DurationTicks, long Damage, long Healed, int Combatants);
    public sealed record ZoneSummaryData(ulong Id, string Name, DateTime Start, int Encounters);
    public sealed record EncounterDetailData(EncounterSummaryData Summary, CombatantSnapshotData[] Combatants);
    public sealed record AttackSummaryData(string Combatant, string Category, string Attack, bool Outgoing,
        long Damage, long DurationTicks, double DPS, double EncDPS, int Swings, int Hits, int CritHits, int Misses, int Blocked);
    public sealed record EncounterActionData(DateTime Time, int TimeSorter, string Attacker, string Victim,
        string Attack, string Damage, int SourceIndex, long Amount, int SwingType, bool Critical,
        string? Special, string? DamageType, string? RawLabel, string? RawSuffix);
    public sealed record ClassifiedActionData(EncounterActionData Action, bool RegisteredDamage, bool RegisteredHealing, bool IsDeath, string NumericTooltip);
    public sealed record ActionSnapshotData(ulong Revision, int Total);
    public sealed record EncounterLogData(DateTime Time, int TimeSorter, int Type, string Text, int SourceIndex);
    public sealed record EncounterLogPageData(long Dropped, CombatPage<EncounterLogData> Page);
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
        private readonly ConditionalWeakTable<ZoneData, EncounterIdentity> zoneIds = new();
        private ulong nextZoneId;
        private EncounterIdentity ZoneIdentity(ZoneData zone) => zoneIds.GetValue(zone, _ => new(checked(++nextZoneId)));
        private ulong nextEncounterId;
        private ulong nextActionSnapshotRevision;
        private (ulong Revision, ClassifiedActionData[] Items, EncounterLogPageData Logs)? actionSnapshot;
        private const int MaximumSnapshotActions = 16384;
        private EncounterIdentity Identity(EncounterData encounter) => encounterIds.GetValue(encounter,
            _ => new(checked(++nextEncounterId)));

        private IEnumerable<(EncounterData Encounter, bool Import)> QueryEncounters()
        {
            var seen = new HashSet<EncounterData>(ReferenceEqualityComparer.Instance);
            foreach (var zone in combatZones)
                for (var index = zone.PopulateAll ? 1 : 0; index < zone.Items.Count; index++)
                    if (seen.Add(zone.Items[index])) yield return (zone.Items[index], ReferenceEquals(zone, combatZones[0]));
        }

        private ZoneData? FindZone(ulong id) => combatZones.Take(4097).FirstOrDefault(zone => ZoneIdentity(zone).Id == id);
        private IEnumerable<(EncounterData Encounter, bool Import)> ZoneContents(ZoneData zone)
        {
            var imported = ReferenceEquals(zone, combatZones[0]);
            return zone.Items.Skip(zone.PopulateAll ? 1 : 0).Distinct<EncounterData>(ReferenceEqualityComparer.Instance).Select(item => (item, imported));
        }
        internal CombatPage<ZoneSummaryData> ListZones(int offset, int limit)
        {
            ValidateCombatPage(offset, limit);
            lock (CombatGate)
            {
                if (combatZones.Count > 4096) throw new InvalidDataException("Zone list exceeds 4096 entries");
                var items = combatZones.Skip(offset).Take(limit).Select(zone => new ZoneSummaryData(ZoneIdentity(zone).Id,
                    ColumnText(zone.ZoneName), zone.StartTime, ZoneContents(zone).Count())).ToArray();
                return new(combatZones.Count, items);
            }
        }
        private IEnumerable<(EncounterData Encounter, bool Import)>? QueryZoneId(ulong id)
        {
            if (combatZones.Count > 4096) throw new InvalidDataException("Zone list exceeds 4096 entries");
            var zone = FindZone(id);
            return zone is null ? null : ZoneContents(zone);
        }
        internal CombatPage<EncounterSummaryData>? ListZoneById(ulong zoneId, int offset, int limit)
        {
            ValidateCombatPage(offset, limit);
            lock (CombatGate)
            {
                var source = QueryZoneId(zoneId);
                if (source is null) return null;
                var total = 0; var items = new List<EncounterSummaryData>();
                foreach (var (encounter, import) in source)
                {
                    if (total >= offset && items.Count < limit) items.Add(Summarize(encounter, import));
                    total = checked(total + 1);
                }
                return new(total, items.ToArray());
            }
        }

        // An encounter ID anchors an actual ZoneList object, never a guessed name or timestamp.
        private IEnumerable<(EncounterData Encounter, bool Import)>? QueryZone(ulong anchorId)
        {
            var anchor = TableEncounter(anchorId);
            if (anchor is null) return null;
            var zone = combatZones.FirstOrDefault(item => item.Items.Skip(item.PopulateAll ? 1 : 0).Any(entry => ReferenceEquals(entry, anchor)));
            if (zone is null) return null;
            return ZoneContents(zone);
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

        internal CombatPage<EncounterSummaryData>? ListZoneEncounters(ulong anchorId, int offset, int limit)
        {
            ValidateCombatPage(offset, limit);
            lock (CombatGate)
            {
                var source = QueryZone(anchorId);
                if (source is null) return null;
                var total = 0;
                var items = new List<EncounterSummaryData>();
                foreach (var (encounter, import) in source)
                {
                    if (total >= offset && items.Count < limit) items.Add(Summarize(encounter, import));
                    total = checked(total + 1);
                }
                return new(total, items.ToArray());
            }
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

        internal EncounterDetailData? ReadCurrentEncounter()
        {
            lock (CombatGate)
                return !importingLogs && !startingCombat && !endingCombat && combatZone.ActiveEncounter is { } current
                    ? Detail(current, false) : null;
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
                    {
                        var attacks = category.Value.Items;
                        // Skip whole categories outside the page, e.g. a late page need not visit every earlier attack.
                        var first = Math.Clamp(offset - total, 0, attacks.Count);
                        var end = first + Math.Min(limit - items.Count, attacks.Count - first);
                        for (var index = first; index < end; index++)
                        {
                            var value = attacks.Values[index];
                            items.Add(new(actor.Name, category.Key, attacks.Keys[index], category.Value.Outgoing,
                                value.Damage, value.Duration.Ticks, value.DPS, value.EncDPS,
                                value.Swings, value.Hits, value.CritHits, value.Misses, value.Blocked));
                        }
                        total = checked(total + attacks.Count);
                    }
                return new(total, items.ToArray());
            }
        }

        internal CombatPage<EncounterActionData>? ReadEncounterActions(ulong id, int offset, int limit)
        {
            ValidateCombatPage(offset, limit);
            lock (CombatGate)
            {
                var encounter = QueryEncounters().Select(item => item.Encounter).FirstOrDefault(item => Identity(item).Id == id);
                if (encounter is null) return null;
                var actions = EncounterActions(encounter);
                var sources = SourceIndices(encounter);
                return new(actions.Count(), actions.Skip(offset).Take(limit).Select(action => ProjectAction(action, sources)).ToArray());
            }
        }

        private static IEnumerable<MasterSwing> EncounterActions(EncounterData encounter) =>
            encounter.Items.Values.SelectMany(actor => actor.AllOut.TryGetValue("All", out var all)
                ? (IEnumerable<MasterSwing>)all.Items : []);

        private static EncounterActionData ProjectAction(MasterSwing action, Dictionary<int, int> sources) => new(
            action.Time, action.TimeSorter, action.Attacker, action.Victim, action.AttackType, action.Damage.ToString(),
            sources.GetValueOrDefault(action.TimeSorter, -1), action.Damage.Number, action.SwingType, action.Critical,
            action.Special, action.DamageType, action.Damage.RawDamageString, action.Damage.DamageString2);

        private static readonly (long Threshold, string Suffix)[] DeathTooltipSuffixes = {
            (1_000_000_000_000_000, "Q"), (1_000_000_000_000, "T"),
            (1_000_000_000, "B"), (1_000_000, "M"), (1_000, "K") };

        // ACT CreateDamageString(value, true, true), used by DeathSecond tooltips.
        internal static string DeathNumericTooltip(long value)
        {
            if (value == long.MinValue) return float.NaN.ToString();
            if (value == long.MaxValue) return float.PositiveInfinity.ToString();
            if (value is < 0 and > -100) return new Dnum(value).ToString();
            foreach (var (threshold, suffix) in DeathTooltipSuffixes)
                if (value >= threshold) return ((double)value / threshold).ToString("0.00") + suffix;
            return value.ToString();
        }

        internal ActionSnapshotData? CaptureActionSnapshot(ulong id) => CaptureActions(id, null);

        // ACT's death report uses the selected combatant's incoming All list, which can
        // exist even if the plugin did not register an outgoing mapping for that swing.
        internal ActionSnapshotData? CaptureIncomingActionSnapshot(ulong id, string combatant) =>
            CaptureActions(id, combatant ?? throw new ArgumentNullException(nameof(combatant)));

        private sealed record LookupRequest(string Combatant, string Category, bool Outgoing);
        internal ActionSnapshotData? CaptureCategoryActions(ulong id, string json)
        {
            var request = System.Text.Json.JsonSerializer.Deserialize<LookupRequest>(json) ?? throw new InvalidDataException("Missing category selection");
            if (request.Combatant is null || request.Category is null) throw new InvalidDataException("Missing category selection");
            ValidateAvoidanceText(request.Combatant); ValidateAvoidanceText(request.Category);
            return CaptureActions(id, request.Combatant, request);
        }

        internal ActionSnapshotData? CaptureLogSnapshot(ulong id) => CaptureActions(id, null, logsOnly: true);

        private ActionSnapshotData? CaptureActions(ulong id, string? incomingCombatant, LookupRequest? lookup = null, bool logsOnly = false)
        {
            lock (CombatGate)
            {
                if (activeTableQuery) throw new InvalidOperationException("Snapshot capture is not permitted during table callbacks");
                ObjectDisposedException.ThrowIf(actionsClosed, this);
                var encounter = QueryEncounters().Select(item => item.Encounter).FirstOrDefault(item => Identity(item).Id == id);
                if (encounter is null) return null;
                IEnumerable<MasterSwing> source;
                if (logsOnly) source = [];
                else if (incomingCombatant is null) source = EncounterActions(encounter);
                else
                {
                    var combatant = encounter.GetCombatant(incomingCombatant);
                    if (combatant is null) return null;
                    if (lookup is not null)
                    {
                        if (!combatant.Items.TryGetValue(lookup.Category, out var category) || category.Outgoing != lookup.Outgoing) return null;
                        if (!ReferenceEquals(category.Parent, combatant)) throw new InvalidDataException("Category ownership changed");
                        source = category.Items.TryGetValue("All", out var all) ? all.Items : [];
                    }
                    else source = combatant.AllInc.TryGetValue("All", out var all) ? all.Items : [];
                }
                var actions = source.Take(MaximumSnapshotActions + 1).ToArray();
                if (actions.Length > MaximumSnapshotActions)
                    throw new InvalidDataException("Action snapshot exceeds 16384 rows");
                var damage = CombatantData.DamageSwingTypes.ToHashSet();
                var healing = CombatantData.HealingSwingTypes.ToHashSet();
                if (encounter.LogLines.Count > EncounterData.MaximumRecordedLogLines)
                    throw new InvalidDataException("Action snapshot exceeds 65536 log rows");
                var sources = SourceIndices(encounter);
                var logs = new EncounterLogPageData(encounter.DroppedLogLines, new(encounter.LogLines.Count,
                    encounter.LogLines.Select(line => new EncounterLogData(line.Time, line.GlobalTimeSorter, line.Type,
                        line.LogLine, sources.GetValueOrDefault(line.GlobalTimeSorter, -1))).ToArray()));
                var items = actions.Select(action => new ClassifiedActionData(ProjectAction(action, sources),
                    damage.Contains(action.SwingType), healing.Contains(action.SwingType), action.Damage == Dnum.Death, DeathNumericTooltip(action.Damage.Number))).ToArray();
                var revision = checked(++nextActionSnapshotRevision);
                if (lookup is not null) CaptureLookupGraph(revision, encounter, lookup, actions);
                else lookupGraph = null;
                actionSnapshot = (revision, items, logs);
                return new(revision, items.Length);
            }
        }

        internal CombatPage<ClassifiedActionData> ReadActionSnapshot(ulong revision, int offset, int limit)
        {
            ValidateCombatPage(offset, limit);
            lock (CombatGate)
            {
                if (actionSnapshot is not { } snapshot || snapshot.Revision != revision)
                    throw new InvalidOperationException("Action snapshot is stale");
                return new(snapshot.Items.Length, snapshot.Items.Skip(offset).Take(limit).ToArray());
            }
        }

        private sealed record LogSearchRequest(string Pattern, bool Regex, int? Type);
        internal int[] SearchSnapshotLogs(ulong revision, string json)
        {
            var request = System.Text.Json.JsonSerializer.Deserialize<LogSearchRequest>(json) ?? throw new InvalidDataException("Missing log search");
            if (request.Pattern is null || request.Pattern.Length > 4096) throw new ArgumentException("Invalid search pattern");
            lock (CombatGate)
            {
                ObjectDisposedException.ThrowIf(actionsClosed, this);
                if (actionSnapshot is not { } snapshot || snapshot.Revision != revision) throw new InvalidOperationException("Action snapshot is stale");
                var regex = request.Regex ? new System.Text.RegularExpressions.Regex(request.Pattern, System.Text.RegularExpressions.RegexOptions.IgnoreCase, TimeSpan.FromMilliseconds(25)) : null;
                var needle = request.Regex ? "" : request.Pattern.ToUpper();
                var clock = System.Diagnostics.Stopwatch.StartNew();
                long bytes = 0;
                var found = new List<int>();
                for (var index = 0; index < snapshot.Logs.Page.Items.Length; index++)
                {
                    if (clock.Elapsed > TimeSpan.FromSeconds(2)) throw new TimeoutException("Log search exceeded two seconds");
                    var line = snapshot.Logs.Page.Items[index];
                    bytes += System.Text.Encoding.UTF8.GetByteCount(line.Text);
                    if (bytes > 16 * 1024 * 1024) throw new InvalidDataException("Log search exceeds 16 MiB");
                    if (request.Type is not null && line.Type != request.Type) continue;
                    if (regex is null ? line.Text.ToUpper().IndexOf(needle, StringComparison.CurrentCulture) >= 0 : regex.IsMatch(line.Text)) found.Add(index);
                }
                return found.ToArray();
            }
        }

        internal EncounterLogPageData ReadActionSnapshotLogs(ulong revision, int offset, int limit)
        {
            ValidateCombatPage(offset, limit);
            lock (CombatGate)
            {
                if (actionSnapshot is not { } snapshot || snapshot.Revision != revision)
                    throw new InvalidOperationException("Action snapshot is stale");
                var logs = snapshot.Logs;
                return new(logs.Dropped, new(logs.Page.Total, logs.Page.Items.Skip(offset).Take(limit).ToArray()));
            }
        }

        internal EncounterLogPageData? ReadEncounterLogs(ulong id, int offset, int limit)
        {
            ValidateCombatPage(offset, limit);
            lock (CombatGate)
            {
                var encounter = QueryEncounters().Select(item => item.Encounter).FirstOrDefault(item => Identity(item).Id == id);
                if (encounter is null) return null;
                var sources = SourceIndices(encounter);
                return new(encounter.DroppedLogLines,
                    new(encounter.LogLines.Count, encounter.LogLines.Skip(offset).Take(limit).Select(line => new EncounterLogData(
                        line.Time, line.GlobalTimeSorter, line.Type, line.LogLine, sources.GetValueOrDefault(line.GlobalTimeSorter, -1))).ToArray()));
            }
        }

        private static Dictionary<int, int> SourceIndices(EncounterData encounter)
        {
            // Called under CombatGate so even duplicates outside the returned page invalidate source selection.
            var sources = new Dictionary<int, int>();
            if (!encounter.LogSourceIdentityKnown) return sources;
            for (var index = 0; index < encounter.LogLines.Count; index++)
            {
                var sorter = encounter.LogLines[index].GlobalTimeSorter;
                if (!sources.TryAdd(sorter, index)) sources[sorter] = -2;
            }
            return sources;
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
