using System.Buffers.Binary;
using System.Diagnostics;
using System.Globalization;
using System.Security.Cryptography;
using Advanced_Combat_Tracker;
using Bxivact.ActCompat;
using Bxivact.Bridge;

internal static class OriginalCorpusChecks
{
    private const long Records = 104744;
    private const string Hash = "a5992e53bef80424ba29d0db4db00d0359c727e00532de0873ce49c67732f240";
    private sealed record ActionValue(EncounterData Encounter, string Attacker, string Victim, long Amount,
        int OutgoingDamage, int OutgoingHealing, int IncomingDamage, int IncomingHealing);
    private sealed class ActorTotals
    {
        internal long Damage, Healed, DamageTaken, HealsTaken;
    }
    private sealed record EncounterTotals(EncounterData Encounter, Dictionary<CombatantData, ActorTotals> Actors,
        HashSet<CombatantData> Allies, long Damage, long Healed);

    private static void Check(bool condition, [System.Runtime.CompilerServices.CallerArgumentExpression(nameof(condition))] string? expression = null)
    {
        if (!condition) throw new Exception("Original corpus integration check failed: " + expression);
    }

    internal static void Run(PluginSession session, string path)
    {
        path = Path.GetFullPath(path);
        using (var input = File.OpenRead(path))
        {
            Check(input.Length == 14971097);
            Check(Convert.ToHexStringLower(SHA256.HashData(input)) == Hash);
        }
        var root = ActGlobals.oFormActMain!;
        var live = (root.ActiveZone, root.CurrentZone, root.LogFilePath, root.LastKnownTime,
            root.ReadThreadLock, ActGlobals.charName);
        Check(!root.InCombat);
        var values = new List<ActionValue>();
        CombatActionDelegate observe = (import, info) =>
        {
            // After event fields are mutable snapshots; capture the actual committed action and routing registrations instead.
            var action = info.combatAction;
            var encounter = action.ParentEncounter;
            Check(import && encounter is not null && !encounter.DuplicateDetection);
            var restricted = ActGlobals.restrictToAll;
            var outgoing = CaptureRoutes(CombatantData.SwingTypeToDamageTypeDataLinksOutgoing,
                CombatantData.OutgoingDamageTypeDataObjects.Keys.LastOrDefault(),
                CombatantData.DamageTypeDataOutgoingDamage, CombatantData.DamageTypeDataOutgoingHealing,
                action.SwingType, action.AttackType, restricted);
            var incoming = CaptureRoutes(CombatantData.SwingTypeToDamageTypeDataLinksIncoming,
                CombatantData.IncomingDamageTypeDataObjects.Keys.LastOrDefault(),
                CombatantData.DamageTypeDataIncomingDamage, CombatantData.DamageTypeDataIncomingHealing,
                action.SwingType, action.AttackType, restricted);
            values.Add(new(encounter!, action.Attacker, action.Victim, action.Damage.Number,
                outgoing.Damage, outgoing.Healing, incoming.Damage, incoming.Healing));
        };
        var subscription = session.SubscribeLogs(2, 8 * 1024 * 1024);
        root.AfterCombatAction += observe;
        try
        {
            ImportResult? baseline = null;
            int? baselineActions = null;
            ulong revision = 0;
            for (var iteration = 0; iteration < 3; iteration++)
            {
                values.Clear();
                var firstEncounter = root.ZoneList[0].Items.Count;
                var elapsed = Stopwatch.StartNew();
                var report = session.ImportLog(path);
                elapsed.Stop();
                Check(report.LinesRead == Records && report.RecordsDispatched == Records && report.Encounters == 3);
                Check(values.Count > 0);
                var imported = root.ZoneList[0].Items.Skip(firstEncounter).ToArray();
                var expected = DeriveTotals(values, imported);
                Check(expected.Length == report.Encounters);
                Check(report.Combatants == expected.Sum(encounter => encounter.Actors.Count));
                Check(report.Damage == expected.Sum(encounter => encounter.Damage));
                Check(report.Healed == expected.Sum(encounter => encounter.Healed));
                Check(report.Damage > 0 && report.Healed > 0);
                var snapshot = session.ReadCombatSnapshot(revision) ?? throw new Exception("Original corpus produced no ended snapshot");
                Check(snapshot.Revision > revision && snapshot.IsImport);
                Check(snapshot.Status == CombatSnapshotStatus.Ended);
                Check(snapshot.DurationTicks == 5808330000);
                CheckSnapshot(snapshot, expected[^1]);
                revision = snapshot.Revision;
                Check(session.ReadCombatSnapshot(revision) is null);
                Check(!root.InCombat && ReferenceEquals(root.ActiveZone, live.ActiveZone));
                Check(root.CurrentZone == live.CurrentZone && root.LogFilePath == live.LogFilePath);
                Check(root.LastKnownTime == live.LastKnownTime && root.ReadThreadLock == live.ReadThreadLock);
                Check(ActGlobals.charName == live.charName && session.DiagnosticsSnapshot.Item1 == 0);
                CheckDroppedPrefix(session, subscription, iteration);
                baseline ??= report;
                baselineActions ??= values.Count;
                // The original parser performs stochastic DoT rounding and can retain import state; validate each run exactly.
                Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(new
                {
                    scenario = "recorded_original_parser_corpus",
                    pass = iteration + 1,
                    report,
                    observedActions = values.Count,
                    actionDerivedDamage = expected.Sum(encounter => encounter.Damage),
                    actionDerivedHealed = expected.Sum(encounter => encounter.Healed),
                    snapshotActors = snapshot.Combatants.Length,
                    observedEncounters = expected.Select(encounter => new
                    {
                        actors = encounter.Actors.Count,
                        allies = encounter.Allies.Count,
                        encounter.Damage,
                        encounter.Healed
                    }),
                    variationFromFirst = new
                    {
                        combatants = report.Combatants != baseline.Combatants,
                        actions = values.Count != baselineActions,
                        damage = report.Damage != baseline.Damage,
                        healed = report.Healed != baseline.Healed
                    },
                    dropped = Records - 2,
                    elapsedMs = elapsed.ElapsedMilliseconds
                }));
            }
        }
        finally
        {
            root.AfterCombatAction -= observe;
            session.UnsubscribeLogs(subscription);
        }
    }

    private static (int Damage, int Healing) CaptureRoutes(SortedDictionary<int, List<string>> registrations,
        string? allCategory, string damageCategory, string healingCategory, int swing, string attack, bool restricted)
    {
        if (!registrations.TryGetValue(swing, out var registered)) return (0, 0);
        // Clone actual link arrays at observation time; later registration edits cannot reclassify already-captured scalars.
        var links = registered.ToArray();
        int Count(string category)
        {
            // Preserve repeated links and ACT's observable All-category/All-attack aliases instead of assuming unique routing.
            var linked = links.Count(link => link == category) * (attack == "All" && !restricted ? 2 : 1);
            return linked + (allCategory == category ? attack == "All" ? 2 : 1 : 0);
        }
        return (Count(damageCategory), Count(healingCategory));
    }

    private static EncounterTotals[] DeriveTotals(List<ActionValue> actions, EncounterData[] imported)
    {
        var byEncounter = new Dictionary<EncounterData, Dictionary<CombatantData, ActorTotals>>(ReferenceEqualityComparer.Instance);
        foreach (var action in actions)
        {
            Check(imported.Any(encounter => ReferenceEquals(encounter, action.Encounter)));
            if (!byEncounter.TryGetValue(action.Encounter, out var actors))
            {
                actors = new(ReferenceEqualityComparer.Instance);
                byEncounter.Add(action.Encounter, actors);
            }
            ActorTotals Find(string name)
            {
                var actor = action.Encounter.GetCombatant(name) ?? throw new Exception("An observed action has no committed actor: " + name);
                if (!actors.TryGetValue(actor, out var totals)) actors.Add(actor, totals = new());
                return totals;
            }
            var outgoing = Find(action.Attacker);
            var incoming = Find(action.Victim);
            // Negative Dnum sentinels and zero contribute no numeric damage, while their actors still belong to the graph.
            if (action.Amount <= 0) continue;
            checked
            {
                outgoing.Damage += action.Amount * action.OutgoingDamage;
                outgoing.Healed += action.Amount * action.OutgoingHealing;
                incoming.DamageTaken += action.Amount * action.IncomingDamage;
                incoming.HealsTaken += action.Amount * action.IncomingHealing;
            }
        }
        Check(byEncounter.Count == imported.Length);
        var result = new List<EncounterTotals>();
        foreach (var encounter in imported)
        {
            if (!byEncounter.TryGetValue(encounter, out var actors)) throw new Exception("Imported encounter has no observed actions");
            Check(actors.Count == encounter.NumCombatants && encounter.Items.Values.All(actors.ContainsKey));
            foreach (var (actor, totals) in actors)
            {
                Check(actor.Damage == totals.Damage && actor.Healed == totals.Healed);
                Check(actor.DamageTaken == totals.DamageTaken && actor.HealsTaken == totals.HealsTaken);
            }
            var allies = new HashSet<CombatantData>(encounter.GetAllies(), ReferenceEqualityComparer.Instance);
            Check(allies.All(actors.ContainsKey));
            var damage = allies.Sum(actor => actors[actor].Damage);
            var healed = allies.Sum(actor => actors[actor].Healed);
            Check(encounter.Damage == damage && encounter.Healed == healed);
            result.Add(new(encounter, actors, allies, damage, healed));
        }
        return result.ToArray();
    }

    private static void CheckSnapshot(CombatSnapshotData snapshot, EncounterTotals expected)
    {
        var encounter = expected.Encounter;
        Check(snapshot.Damage == expected.Damage && snapshot.Healed == expected.Healed);
        Check(snapshot.Combatants.Length == expected.Actors.Count);
        Check(snapshot.Zone == encounter.ZoneName && snapshot.Title == encounter.Title && snapshot.Character == encounter.CharName);
        Check(snapshot.Start == encounter.StartTime && snapshot.End == encounter.EndTime && snapshot.DurationTicks == encounter.Duration.Ticks);
        var seen = new HashSet<CombatantData>(ReferenceEqualityComparer.Instance);
        foreach (var actor in snapshot.Combatants)
        {
            var committed = encounter.GetCombatant(actor.Name) ?? throw new Exception("Snapshot actor is absent from the observed action graph");
            Check(seen.Add(committed) && actor.Name == committed.Name);
            var totals = expected.Actors[committed];
            Check(actor.IsAlly == expected.Allies.Contains(committed));
            Check(actor.Damage == totals.Damage && actor.Healed == totals.Healed);
            Check(actor.DamageTaken == totals.DamageTaken && actor.HealsTaken == totals.HealsTaken);
        }
    }

    internal static void CheckMessages(string[] messages)
    {
        var informationPrefixes = new[]
        {
            "FFXIV_ACT_Plugin Version:", "Selected Language ID:", "Selected Process ID:", "Detected Process ID:"
        };
        const string warningPrefix = "SimulatedDamage Timeout with no ticks/GroundAoE: Removing status ";
        Check(messages.Length >= 3);
        var notices = 0;
        var warnings = 0;
        for (var index = 0; index < messages.Length; index++)
        {
            var message = messages[index];
            var delimiter = message.IndexOf("] ", StringComparison.Ordinal);
            Check(message.StartsWith('[') && delimiter > 1);
            Check(TimeOnly.TryParseExact(message.AsSpan(1, delimiter - 1), "HH:mm:ss.fff", CultureInfo.InvariantCulture,
                DateTimeStyles.None, out _));
            var body = message[(delimiter + 2)..];
            if (index < 3) Check(body.StartsWith(informationPrefixes[index], StringComparison.Ordinal));
            var information = informationPrefixes.FirstOrDefault(prefix => body.StartsWith(prefix, StringComparison.Ordinal));
            if (information is not null)
            {
                Check(!body.AsSpan(information.Length).Trim().IsEmpty);
                notices++;
            }
            else
            {
                // This corpus-specific warning is reported explicitly; arbitrary exceptions or unknown messages still fail.
                Check(body.StartsWith(warningPrefix, StringComparison.Ordinal));
                Check(!body.AsSpan(warningPrefix.Length).Trim().IsEmpty);
                warnings++;
            }
        }
        Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(new
        {
            scenario = "recorded_original_parser_messages",
            notices,
            warnings,
            warningCategory = "simulated_damage_timeout_without_ticks_or_ground_aoe"
        }));
    }

    private static void CheckDroppedPrefix(PluginSession session, ulong subscription, int iteration)
    {
        // A capacity query must leave both source records and the loss interval available to the next successful drain.
        var query = session.DrainLogs(subscription, 0);
        Check(query.Records == 0 && query.RequiredPayloadCapacity > 0 && query.Dropped == 0);
        var batch = session.DrainLogs(subscription, 8 * 1024 * 1024);
        var first = (ulong)(iteration * Records + 1);
        Check(batch.RequiredPayloadCapacity == 0 && batch.Records == 2 && batch.Dropped == (ulong)(Records - 2));
        Check(batch.FirstDropped == first && batch.LastDropped == first + (ulong)Records - 3);
        var payload = batch.Payload.AsSpan();
        for (var index = 0; index < 2; index++)
        {
            Check(payload.Length >= 56);
            var length = checked((int)BinaryPrimitives.ReadUInt32LittleEndian(payload));
            Check(length >= 56 && length <= payload.Length);
            Check((BinaryPrimitives.ReadUInt32LittleEndian(payload[4..]) & 1) == 1);
            Check(BinaryPrimitives.ReadUInt64LittleEndian(payload[8..]) == first + (ulong)Records - 2 + (ulong)index);
            payload = payload[length..];
        }
        Check(payload.IsEmpty);
        var empty = session.DrainLogs(subscription, 0);
        Check(empty.Records == 0 && empty.Dropped == 0 && empty.RequiredPayloadCapacity == 0);
    }
}
