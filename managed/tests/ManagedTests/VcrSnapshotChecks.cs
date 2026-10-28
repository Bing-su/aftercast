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
            var owner = Environment.CurrentManagedThreadId;
            DamageTypeData.ColumnDefs["Lookup"] = new("Lookup", true, "TEXT", "lookup", row => {
                if (Environment.CurrentManagedThreadId != owner || row.Parent?.Name != "Target") throw new Exception("Lookup parent/owner");
                return $"{row.Type}:{row.Damage}:{row.Swings}";
            }, _ => "");
            var categoryCatalog = host.ReadTableColumns(TableLevel.Category);
            var categoryQuery = new TableQuery(new(TableLevel.Category, id), categoryCatalog.Revision, ["Lookup"]);
            var categoryTable = host.ReadLookupTable(outgoing.Revision, categoryQuery);
            if (categoryTable.Total != 1 || categoryTable.Rows[0].Cells[0].Text != "Target:30:2") throw new Exception("Lookup aggregate metrics");
            var compares = 0;
            AttackType.ColumnDefs["Lookup"] = new("Lookup", true, "TEXT", "lookup", row => row.Type!, _ => "", (left, right) => { compares++; return string.CompareOrdinal(left.Type, right.Type); });
            var attackCatalog = host.ReadTableColumns(TableLevel.Attack);
            var attackQuery = new TableQuery(new(TableLevel.Attack, id), attackCatalog.Revision, ["Lookup"]);
            ActGlobals.mDSort = ActGlobals.mDSort2 = "Lookup";
            var attackTable = host.ReadLookupTable(outgoing.Revision, attackQuery, "Target");
            if (compares == 0 || attackTable.Total != 2 || attackTable.Rows[0].Cells[0].Text != "Strike") throw new Exception("Lookup custom comparison");
            MasterSwing.ColumnDefs["Lookup"] = new("Lookup", true, "TEXT", "lookup", row => {
                if (row.Special != "None" || row.Tags.Count != 0 || row.ParentEncounter is not null) throw new Exception("Lookup clone fields");
                try { host.ReadLookupTable(outgoing.Revision, categoryQuery); throw new Exception("Lookup reentrancy accepted"); } catch (InvalidOperationException) { }
                return row.Damage.ToString();
            }, _ => "", (left, right) => 0);
            var actionCatalog = host.ReadTableColumns(TableLevel.Action);
            var actionQuery = new TableQuery(new(TableLevel.Action, id), actionCatalog.Revision, ["Lookup"]);
            var actionTable = host.ReadLookupTable(outgoing.Revision, actionQuery, "Target", "Strike");
            if (actionTable.Rows.Any(row => row.Cells[0].Error is not null)) throw new Exception("Lookup clone callback failed");
            MasterSwing.ColumnDefs["ThrowLookup"] = new("ThrowLookup", true, "TEXT", "throw_lookup", _ => throw new InvalidOperationException("synthetic lookup cell"), _ => "", (_, _) => 0);
            actionCatalog = host.ReadTableColumns(TableLevel.Action);
            var failedCell = host.ReadLookupTable(outgoing.Revision, new(new(TableLevel.Action, id), actionCatalog.Revision, ["ThrowLookup"]), "Target", "Strike");
            if (failedCell.Rows.Any(row => row.Cells[0].Error is null)) throw new Exception("Lookup cell exception hidden");
            AttackType.ColumnDefs["ThrowLookup"] = new("ThrowLookup", true, "TEXT", "throw_lookup", row => row.Type!, _ => "", (_, _) => throw new InvalidOperationException("synthetic lookup comparison"));
            attackCatalog = host.ReadTableColumns(TableLevel.Attack);
            var failedSort = host.ReadLookupTable(outgoing.Revision, new(new(TableLevel.Attack, id), attackCatalog.Revision, ["Lookup"], Sort: new("ThrowLookup")), "Target");
            if (failedSort.SortError is null) throw new Exception("Lookup comparator exception hidden");
            var literalSearch = System.Text.Json.JsonSerializer.Serialize(new { Pattern = "|1|", Regex = false, Type = (int?)null });
            if (!host.SearchSnapshotLogs(outgoing.Revision, literalSearch).SequenceEqual(new[] { 0, 1, 3 })) throw new Exception("Snapshot literal search");
            var regexSearch = System.Text.Json.JsonSerializer.Serialize(new { Pattern = @"^[02]\|", Regex = true, Type = (int?)null });
            if (!host.SearchSnapshotLogs(outgoing.Revision, regexSearch).SequenceEqual(new[] { 0, 1 })) throw new Exception("Snapshot regex search");
            try { host.SearchSnapshotLogs(outgoing.Revision, System.Text.Json.JsonSerializer.Serialize(new { Pattern = "(", Regex = true, Type = (int?)null })); throw new Exception("Invalid regex accepted"); } catch (ArgumentException) { }
            var custom = host.CaptureCategoryActions(id, "{\"Combatant\":\"Source\",\"Category\":\"Custom\",\"Outgoing\":true}")!;
            var customRows = host.ReadActionSnapshot(custom.Revision, 0, 256).Items;
            var originalCustom = root.ZoneList[0].Items.Single().GetCombatant("Source")!.Items["Custom"].Items["All"].Items.Count;
            if (customRows.Length != originalCustom || originalCustom < outgoingRows.Length) throw new Exception($"Custom category entries lost: {customRows.Length} / {originalCustom}");
            try { host.ReadLookupTable(outgoing.Revision, categoryQuery); throw new Exception("Lookup stale revision accepted"); } catch (InvalidOperationException) { }
            var incoming = host.CaptureCategoryActions(id, "{\"Combatant\":\"Target\",\"Category\":\"Received\",\"Outgoing\":false}")!;
            var incomingRows = host.ReadActionSnapshot(incoming.Revision, 0, 256).Items;
            if (incomingRows.Length != 1 || incomingRows[0].Action.Attacker != "Source") throw new Exception("Incoming category scope");
            try { host.ReadActionSnapshot(outgoing.Revision, 0, 1); throw new Exception("Old category snapshot accepted"); } catch (InvalidOperationException) { }
            if (host.CaptureCategoryActions(id, "{\"Combatant\":\"Source\",\"Category\":\"Damage\",\"Outgoing\":false}") is not null) throw new Exception("Wrong direction accepted");
            var encounter = root.ZoneList[0].Items.Single();
            encounter.LogLines.Add(new LogLineEntry(DateTime.UnixEpoch, new string('a', 8192) + "!", 42, 999999));
            var logOnly = host.CaptureLogSnapshot(id)!;
            if (logOnly.Total != 0 || host.ReadActionSnapshotLogs(logOnly.Revision, 0, 256).Page.Total != 5) throw new Exception("Log-only capture");
            var typeSearch = System.Text.Json.JsonSerializer.Serialize(new { Pattern = "", Regex = false, Type = (int?)42 });
            if (!host.SearchSnapshotLogs(logOnly.Revision, typeSearch).SequenceEqual(new[] { 4 })) throw new Exception("Snapshot type filter");
            try { host.SearchSnapshotLogs(logOnly.Revision, System.Text.Json.JsonSerializer.Serialize(new { Pattern = "^(a+)+$", Regex = true, Type = (int?)42 })); throw new Exception("Regex timeout not enforced"); } catch (System.Text.RegularExpressions.RegexMatchTimeoutException) { }
            root.ZoneList[0].Items.Single().GetCombatant("Target")!.AllInc["All"].Items.Clear();
            if (!bytes.SequenceEqual(host.ReadVcr(snapshot.Revision, 0, 65536).Bytes)) throw new Exception("VCR snapshot mutated");
            var replacement = host.CaptureVcr(id)!;
            try { host.ReadVcr(snapshot.Revision, 0, 1); throw new Exception("Old VCR revision accepted"); } catch (InvalidOperationException) { }
            if (replacement.Revision == snapshot.Revision || eventCount != beforeActions || logCount != beforeLogs) throw new Exception("VCR replacement emitted events");
        }
        finally { directory.Delete(true); }
    }
}
