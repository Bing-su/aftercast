using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using System.Text.Json;

internal static class VcrSnapshotChecks
{
    internal static void Run() { HostLifetimeChecks.RunBounded(Snapshot); Console.WriteLine("VCR immutable graph, categories, provenance and no event replay passed."); }
    private static void Snapshot()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-vcr-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            Aftercast.Testing.CombatFixture.ConfigureCategories();
            CombatantData.OutgoingDamageTypeDataObjects["Custom"] = new("Custom", -1, System.Drawing.Color.Red);
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing[1].Add("Custom");
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing[1].Add("Custom");
            CombatantData.DamageTypeDataNonSkillDamage = "Damage";
            CombatantData.DamageSwingTypes.Add(1); CombatantData.HealingSwingTypes.Add(2);
            CombatantData.SwingTypeToDamageTypeDataLinksOutgoing[2] = new() { "Healing" };
            CombatantData.SwingTypeToDamageTypeDataLinksIncoming[2] = new() { "Received" };
            var root = ActGlobals.oFormActMain!; var eventCount = 0; var logCount = 0;
            root.AfterCombatAction += (_, _) => eventCount++;
            root.OnLogLineRead += (_, _) => logCount++;
            root.GetDateTimeFromLog = line => DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0]));
            root.BeforeLogLineRead += (_, info) => {
                var fields = info.logLine.Split('|'); root.SetEncounter(info.detectedTime, "Source", "Target");
                root.AddCombatAction(new MasterSwing(int.Parse(fields[1]), false, "", new Dnum(long.Parse(fields[2])), info.detectedTime,
                    root.GlobalTimeSorter, "Strike", "Source", "Physical", "Target"));
            };
            var path = Path.Combine(directory.FullName, "synthetic.log");
            File.WriteAllText(path, "0|1|10\n2|1|20\n4|2|5\n6|1|-10\n"); host.ImportLog(path);
            var id = host.ListEncounters(0, 1).Items.Single().Id; var beforeActions = eventCount; var beforeLogs = logCount;
            var snapshot = host.CaptureVcr(id)!; var bytes = host.ReadVcr(snapshot.Revision, 0, 65536).Bytes;
            using var report = JsonDocument.Parse(bytes);
            var actor = report.RootElement.GetProperty("actors").EnumerateArray().Single(row => row.GetProperty("Name").GetString() == "Target");
            if (actor.GetProperty("damageTaken").GetInt64() != 30 || actor.GetProperty("healsTaken").GetInt64() != 5 ||
                actor.GetProperty("incoming").GetArrayLength() != 4 || !actor.GetProperty("incoming")[3].GetProperty("isDeath").GetBoolean())
                throw new Exception("VCR captured actor totals or death");
            if (report.RootElement.GetProperty("logs").GetArrayLength() != 4 || actor.GetProperty("incoming")[1].GetProperty("action").GetProperty("SourceIndex").GetInt32() != 1)
                throw new Exception("VCR log identity");
            if (eventCount != beforeActions || logCount != beforeLogs) throw new Exception("VCR replayed events");
            var outgoing = host.CaptureCategoryActions(id, "{\"Combatant\":\"Source\",\"Category\":\"Damage\",\"Outgoing\":true}")!;
            var outgoingRows = host.ReadActionSnapshot(outgoing.Revision, 0, 256).Items;
            if (outgoingRows.Length != 3 || outgoingRows.Any(row => row.Action.Victim != "Target")) throw new Exception("Outgoing category scope");
            var log = host.ReadActionSnapshotLogs(outgoing.Revision, outgoingRows[0].Action.SourceIndex, 1).Page.Items.Single();
            if (log.TimeSorter != outgoingRows[0].Action.TimeSorter) throw new Exception("Category snapshot log provenance");
            var custom = host.CaptureCategoryActions(id, "{\"Combatant\":\"Source\",\"Category\":\"Custom\",\"Outgoing\":true}")!;
            var customRows = host.ReadActionSnapshot(custom.Revision, 0, 256).Items;
            var originalCustom = root.ZoneList[0].Items.Single().GetCombatant("Source")!.Items["Custom"].Items["All"].Items.Count;
            if (customRows.Length != originalCustom || originalCustom < outgoingRows.Length) throw new Exception($"Custom category entries lost: {customRows.Length} / {originalCustom}");
            var incoming = host.CaptureCategoryActions(id, "{\"Combatant\":\"Target\",\"Category\":\"Received\",\"Outgoing\":false}")!;
            var incomingRows = host.ReadActionSnapshot(incoming.Revision, 0, 256).Items;
            if (incomingRows.Length != 1 || incomingRows[0].Action.Attacker != "Source") throw new Exception("Incoming category scope");
            try { host.ReadActionSnapshot(outgoing.Revision, 0, 1); throw new Exception("Old category snapshot accepted"); } catch (InvalidOperationException) { }
            if (host.CaptureCategoryActions(id, "{\"Combatant\":\"Source\",\"Category\":\"Damage\",\"Outgoing\":false}") is not null) throw new Exception("Wrong direction accepted");
            root.ZoneList[0].Items.Single().GetCombatant("Target")!.AllInc["All"].Items.Clear();
            if (!bytes.SequenceEqual(host.ReadVcr(snapshot.Revision, 0, 65536).Bytes)) throw new Exception("VCR snapshot mutated");
            var replacement = host.CaptureVcr(id)!;
            try { host.ReadVcr(snapshot.Revision, 0, 1); throw new Exception("Old VCR revision accepted"); } catch (InvalidOperationException) { }
            if (replacement.Revision == snapshot.Revision || eventCount != beforeActions || logCount != beforeLogs) throw new Exception("VCR replacement emitted events");
        }
        finally { directory.Delete(true); }
    }
}
