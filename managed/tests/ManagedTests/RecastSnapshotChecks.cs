using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using System.Text.Json;

internal static class RecastSnapshotChecks
{
    internal static void Run() { HostLifetimeChecks.RunBounded(Snapshot); HostLifetimeChecks.RunBounded(ModifierWindow); HostLifetimeChecks.RunBounded(LastNamedModifier); Console.WriteLine("Recast owned attack, modifier evidence, death boundary and snapshot checks passed."); }
    private static void Snapshot()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-recast-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            Aftercast.Testing.CombatFixture.ConfigureCategories();
            CombatantData.DamageSwingTypes.Add(1);
            var root = ActGlobals.oFormActMain!;
            root.GetDateTimeFromLog = line => DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0]));
            root.BeforeLogLineRead += (_, info) => {
                var fields = info.logLine.Split('|');
                root.SetEncounter(info.detectedTime, fields[3], fields[4]);
                root.AddCombatAction(new MasterSwing(1, false, "", new Dnum(long.Parse(fields[1])), info.detectedTime,
                    root.GlobalTimeSorter, fields[2], fields[3], "Physical", fields[4]));
            };
            var path = Path.Combine(directory.FullName, "input.log");
            File.WriteAllText(path, "0|100|Strike|Source|Target\n5|0|Traumatic Swipe|Enemy|Source\n5|-1|Strike|Source|Target\n10|100|Strike|Source|Target\n12|-10|Killing|Source|Enemy\n15|-1|Strike|Source|Target\n20|100|Strike|Source|Target\n");
            host.ImportLog(path);
            var id = host.ListEncounters(0, 1).Items.Single().Id;
            const string request = "{\"Combatant\":\"Source\",\"Category\":\"Damage\",\"Attack\":\"Strike\",\"Outgoing\":true,\"Modifier\":true}";
            var snapshot = host.CaptureRecast(id, request)!;
            var bytes = host.ReadRecast(snapshot.Revision, 0, 65536).Bytes;
            using var report = JsonDocument.Parse(bytes);
            var rows = report.RootElement.GetProperty("rows").EnumerateArray().ToArray();
            if (rows.Length != 5 || rows[0].GetProperty("modified").GetBoolean() || rows[1].GetProperty("modified").GetBoolean() ||
                !rows[2].GetProperty("modified").GetBoolean() || rows[3].GetProperty("modified").GetBoolean())
                throw new Exception("Modifier strict boundary or attacker death");
            if (rows[2].GetProperty("modifierEvidence").GetProperty("SourceIndex").GetInt32() != 1 ||
                rows[2].GetProperty("action").GetProperty("SourceIndex").GetInt32() != 3 || report.RootElement.GetProperty("logs").GetArrayLength() != 7)
                throw new Exception("Recast source identity");
            if (report.RootElement.GetProperty("firstAttacker").GetString() != "Source") throw new Exception("Timer attacker source");
            var disabled = host.CaptureRecast(id, request.Replace("\"Modifier\":true", "\"Modifier\":false", StringComparison.Ordinal))!;
            using var noModifier = JsonDocument.Parse(host.ReadRecast(disabled.Revision, 0, 65536).Bytes);
            if (noModifier.RootElement.GetProperty("rows")[2].GetProperty("modified").ValueKind != JsonValueKind.Null) throw new Exception("Disabled modifier");
            try { host.ReadRecast(snapshot.Revision, 0, 1); throw new Exception("Old recast accepted"); }
            catch (InvalidOperationException) { }
            try { host.CaptureRecast(id, "{}"); throw new Exception("Missing selection accepted"); }
            catch (InvalidDataException) { }
            if (host.ReadRecast(disabled.Revision, 0, 1).Bytes.Length != 1) throw new Exception("Failed capture replaced snapshot");
            if (host.CaptureRecast(id, request.Replace("\"Outgoing\":true", "\"Outgoing\":false", StringComparison.Ordinal)) is not null)
                throw new Exception("Wrong direction accepted");
        }
        finally { directory.Delete(true); }
    }
    private static void ModifierWindow()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-recast-window-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            Aftercast.Testing.CombatFixture.ConfigureCategories();
            var root = ActGlobals.oFormActMain!;
            root.GetDateTimeFromLog = line => DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0]));
            root.BeforeLogLineRead += (_, info) => {
                var fields = info.logLine.Split('|'); root.SetEncounter(info.detectedTime, fields[3], fields[4]);
                root.AddCombatAction(new MasterSwing(1, false, "", new Dnum(long.Parse(fields[1])), info.detectedTime,
                    root.GlobalTimeSorter, fields[2], fields[3], "Physical", fields[4]));
            };
            var path = Path.Combine(directory.FullName, "window.log");
            File.WriteAllText(path, "0|0|Traumatic Swipe|Enemy|Source\n5|100|Filler|Source|Target\n10|100|Filler|Source|Target\n15|100|Filler|Source|Target\n20|100|Filler|Source|Target\n25|100|Filler|Source|Target\n30|100|Strike|Source|Target\n31|100|Strike|Source|Target\n");
            host.ImportLog(path);
            var id = host.ListEncounters(0, 1).Items.Single().Id;
            var capture = host.CaptureRecast(id, "{\"Combatant\":\"Source\",\"Category\":\"Damage\",\"Attack\":\"Strike\",\"Outgoing\":true,\"Modifier\":true}")!;
            using var report = JsonDocument.Parse(host.ReadRecast(capture.Revision, 0, 65536).Bytes);
            var rows = report.RootElement.GetProperty("rows");
            if (!rows[0].GetProperty("modified").GetBoolean() || rows[1].GetProperty("modified").GetBoolean())
                throw new Exception("Modifier 30/31-second boundary");
        }
        finally { directory.Delete(true); }
    }

    private static void LastNamedModifier()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-recast-localized-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            Aftercast.Testing.CombatFixture.ConfigureCategories();
            var root = ActGlobals.oFormActMain!;
            root.GetDateTimeFromLog = line => DateTime.UnixEpoch.AddSeconds(int.Parse(line.Split('|')[0]));
            root.BeforeLogLineRead += (_, info) => {
                var fields = info.logLine.Split('|'); root.SetEncounter(info.detectedTime, fields[3], fields[4]);
                root.AddCombatAction(new MasterSwing(1, false, "", new Dnum(long.Parse(fields[1])), info.detectedTime,
                    root.GlobalTimeSorter, fields[2], fields[3], "Physical", fields[4]));
            };
            var path = Path.Combine(directory.FullName, "localized.log");
            File.WriteAllText(path, "0|0|Traumatischer Greifer|Enemy2|Source\n5|0|Traumatic Swipe|Enemy|Source\n10|100|Strike|Source|Target\n");
            host.ImportLog(path);
            var id = host.ListEncounters(0, 1).Items.Single().Id;
            var capture = host.CaptureRecast(id, "{\"Combatant\":\"Source\",\"Category\":\"Damage\",\"Attack\":\"Strike\",\"Outgoing\":true,\"Modifier\":true}")!;
            using var report = JsonDocument.Parse(host.ReadRecast(capture.Revision, 0, 65536).Bytes);
            var evidence = report.RootElement.GetProperty("rows")[0].GetProperty("modifierEvidence");
            if (evidence.GetProperty("Attacker").GetString() != "Enemy2" || evidence.GetProperty("SourceIndex").GetInt32() != 0)
                throw new Exception("Last named modifier list");
        }
        finally { directory.Delete(true); }
    }

}
