using System.Text.Json;
using Aftercast.ActCompat;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    private ulong nextTimelineRevision;
    private (ulong Revision, byte[] Bytes)? timelineSnapshot;
    private sealed record TimelineOptions(int Grouping, int Window, bool Rolling, int Metrics);

    internal AvoidanceSnapshotInfo? CaptureTimeline(ulong id, string optionsJson)
    {
        var options = JsonSerializer.Deserialize<TimelineOptions>(optionsJson) ?? throw new InvalidDataException("Missing timeline options");
        if (options.Grouping is < 0 or > 3 || options.Window is < 1 or > 600 || options.Metrics is < 0 or > 15)
            throw new InvalidDataException("Invalid timeline options");
        lock (CombatGate)
        {
            var found = QueryEncounters().FirstOrDefault(item => Identity(item.Encounter).Id == id);
            if (found.Encounter is null) return null;
            var encounter = found.Encounter;
            if (encounter.Items.Count > 512 || encounter.LogLines.Count > EncounterData.MaximumRecordedLogLines)
                throw new InvalidDataException("Timeline exceeds actor or log limit");
            var seconds = encounter.Duration.TotalSeconds;
            if (!double.IsFinite(seconds) || seconds < 0 || seconds > 360000) throw new InvalidDataException("Timeline duration exceeds limit");
            var duration = (int)seconds;
            var points = options.Rolling ? duration : duration / options.Window;
            var allActors = encounter.Items.Values.ToList();
            var allies = encounter.GetAllies(true) ?? throw new InvalidDataException("Timeline ally cache is not available; retry after encounter details load");
            var groups = new Dictionary<string, List<CombatantData>>();
            void Add(string name, List<CombatantData> people) { ValidateAvoidanceText(name); groups.Add(name, people); }
            switch (options.Grouping)
            {
                case 0:
                    foreach (var actor in allActors) Add(actor.Name, [actor]);
                    break;
                case 1:
                    var types = new List<CombatantData>[] { [], [], [], [] };
                    string[] typeNames = ["Tanks", "Healers", "Melee DPS", "Other DPS"];
                    for (var i = 0; i < types.Length; i++) Add(typeNames[i], types[i]);
                    foreach (var actor in allActors)
                    {
                        if (!allies.Contains(actor)) { Add(actor.Name, [actor]); continue; }
                        var currentAllies = encounter.GetAllies();
                        if (!currentAllies.Contains(actor)) continue;
                        var damage = actor.Items[CombatantData.DamageTypeDataOutgoingDamage].Damage;
                        var healing = actor.Items[CombatantData.DamageTypeDataOutgoingHealing].Damage;
                        var nonSkill = actor.Items[CombatantData.DamageTypeDataNonSkillDamage].Damage;
                        var incomingHealing = actor.Items[CombatantData.DamageTypeDataIncomingHealing].Damage;
                        var type = incomingHealing > damage / 3 && damage > healing ? 1 :
                            healing > damage / 3 && healing > incomingHealing ? 2 : nonSkill > damage / 10 ? 3 : 4;
                        types[type - 1].Add(actor);
                    }
                    break;
                case 2:
                    Add("Us", allies); Add("Them", allActors.Where(actor => !allies.Contains(actor)).ToList());
                    break;
                case 3:
                    Add("Us", allies);
                    foreach (var actor in allActors.Where(actor => !allies.Contains(actor))) Add(actor.Name, [actor]);
                    break;
            }
            if (groups.Count > 516 || (long)groups.Count * (1 + System.Numerics.BitOperations.PopCount((uint)options.Metrics)) * points * 2 > 2_000_000)
                throw new InvalidDataException("Timeline exceeds group or cell budget");
            var timers = ActGlobals.oFormSpellTimers ?? throw new InvalidDataException("Timeline timer state is unavailable");
            var start = encounter.StartTime;
            long work = 0;
            var visited = 0;
            var rows = new List<object>();
            foreach (var (name, actors) in groups)
            {
                var markers = new long[checked(points * 2)];
                void Mark(MasterSwing action, long flag)
                {
                    var offset = (int)(action.Time - start).TotalSeconds;
                    var index = options.Rolling ? offset : offset / options.Window + 1;
                    if (index < 0 || index >= markers.Length) throw new InvalidDataException("Timeline marker outside ACT bucket bounds");
                    markers[index] |= flag;
                }
                foreach (var actor in actors)
                {
                    if (actor.AllOut.Count + actor.AllInc.Count > 16384) throw new InvalidDataException("Timeline exceeds attack group limit");
                    foreach (var (key, attack) in actor.AllOut)
                    {
                        if (key == "All") continue;
                        visited = checked(visited + attack.Items.Count);
                        if (visited > 65536) throw new InvalidDataException("Timeline marker row budget exceeded");
                        if (key == "Killing")
                        {
                            foreach (var action in attack.Items)
                                if (allies.Contains(actor) || !action.Victim.Contains(' ')) Mark(action, 1);
                        }
                        else if (timers.TimerLookups.ContainsKey(key))
                        {
                            var sorted = new List<MasterSwing>(attack.Items);
                            sorted.Sort(static (a, b) => { var v = a.TimeSorter.CompareTo(b.TimeSorter); return v != 0 ? v : a.Time.CompareTo(b.Time); });
                            if (sorted.Count == 0) throw new InvalidDataException("Timeline timer attack has no actions");
                            Mark(sorted[0], 4);
                            for (var i = 1; i < sorted.Count; i++)
                                if ((int)(sorted[i].Time - sorted[i - 1].Time).TotalSeconds > 12) Mark(sorted[i], 4);
                        }
                    }
                    if (actor.AllInc.TryGetValue("Killing", out var deaths))
                    {
                        visited = checked(visited + deaths.Items.Count);
                        if (visited > 65536) throw new InvalidDataException("Timeline marker row budget exceeded");
                        foreach (var action in deaths.Items) Mark(action, 2);
                    }
                }
                var series = new List<object>();
                string[] categories = [CombatantData.DamageTypeDataOutgoingDamage, CombatantData.DamageTypeDataIncomingDamage,
                    CombatantData.DamageTypeDataOutgoingHealing, CombatantData.DamageTypeDataIncomingHealing];
                for (var metric = 0; metric < 4; metric++)
                {
                    if ((options.Metrics & (1 << metric)) == 0) continue;
                    var samples = new List<TimelineNumeric.Sample>();
                    foreach (var actor in actors)
                    {
                        var category = actor.Items[categories[metric]];
                        if (!category.Items.TryGetValue("All", out var attack)) continue;
                        foreach (var action in attack.Items)
                        {
                            if (samples.Count == 16384) throw new InvalidDataException("Timeline exceeds 16384 metric samples");
                            var offset = (action.Time - start).TotalSeconds;
                            samples.Add(new(offset, action.Damage.Number));
                            if (action.Damage.Number > 0) work = checked(work + (options.Rolling ? options.Window : 1));
                            if (work > 4_000_000) throw new InvalidDataException("Timeline work budget exceeded");
                        }
                    }
                    var values = TimelineNumeric.Calculate(duration, options.Window, options.Rolling, samples);
                    series.Add(new { metric, axisMaximum = Math.Max(1, values.Raw.Take(points + 1).DefaultIfEmpty().Max()), raw = values.Raw.Skip(1).Take(points).ToArray(), rates = values.DisplayedRates });
                }
                rows.Add(new { name, markers = markers.Skip(1).Take(points).ToArray(), series });
            }
            ValidateAvoidanceText(encounter.ZoneName); ValidateAvoidanceText(encounter.Title); ValidateAvoidanceText(encounter.CharName);
            var sources = SourceIndices(encounter);
            var logs = encounter.LogLines.Select(line => {
                ValidateAvoidanceText(line.LogLine);
                return new { time = line.Time, seconds = (line.Time - start).TotalSeconds, line.GlobalTimeSorter,
                    line.Type, text = line.LogLine, sourceIndex = sources.GetValueOrDefault(line.GlobalTimeSorter, -1) };
            }).ToArray();
            var report = new { summary = Summarize(encounter, found.Import), options, duration, points,
                droppedLogs = encounter.DroppedLogLines, rows, logs };
            using var output = new AvoidanceBuffer();
            JsonSerializer.Serialize(output, report, AvoidanceJson);
            var bytes = output.ToArray();
            var revision = checked(++nextTimelineRevision);
            timelineSnapshot = (revision, bytes);
            return new(revision, bytes.Length);
        }
    }

    internal AvoidanceSnapshotChunk ReadTimeline(ulong revision, int offset, int limit)
    {
        if (offset < 0 || limit is < 1 or > 65536) throw new ArgumentOutOfRangeException(nameof(limit));
        lock (CombatGate)
        {
            if (timelineSnapshot is not { } snapshot || snapshot.Revision != revision)
                throw new InvalidOperationException("Timeline snapshot is stale");
            var start = Math.Min(offset, snapshot.Bytes.Length);
            var count = Math.Min(limit, snapshot.Bytes.Length - start);
            return new(snapshot.Bytes.Length, snapshot.Bytes.AsSpan(start, count).ToArray());
        }
    }
}
