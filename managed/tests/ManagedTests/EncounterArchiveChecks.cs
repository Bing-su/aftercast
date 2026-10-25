using System.Runtime.Serialization;
using System.Text;
using System.Xml;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Testing;

internal static class EncounterArchiveChecks
{
    private static void Check(bool condition, [System.Runtime.CompilerServices.CallerArgumentExpression(nameof(condition))] string? expression = null)
    { if (!condition) throw new Exception("Encounter archive check failed: " + expression); }
    private static void Throws<T>(Action action) where T : Exception
    { try { action(); } catch (T) { return; } throw new Exception("Expected " + typeof(T).Name); }

    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-history-");
        var index = Path.Combine(directory.FullName, "history", "index.xml");
        var output = Path.Combine(directory.FullName, "export.act");
        var epoch = DateTime.UnixEpoch.AddTicks(1234567);
        try
        {
            string saved;
            using (var host = new HostEnvironment(directory.FullName))
            {
                CombatFixture.ConfigureCategories();
                var root = ActGlobals.oFormActMain!;
                var records = Path.Combine(directory.FullName, "records.xml");
                root.EncDatabaseAdd(new(1, epoch, epoch.AddSeconds(2), "Metadata <한글>", "Source", "logs"));
                root.ZoneDatabaseAdd(new(0, epoch, epoch.AddSeconds(2), "Fixture", "Source", "logs"));
                root.SaveXmlRecords(records);
                root.EncDatabase.Clear(); root.ZoneDatabase.Clear(); root.LoadXmlRecords(records);
                Check(root.EncDatabase[epoch].Label == "Metadata <한글>" && root.EncDatabase[epoch].StartTime.Kind == epoch.Kind && root.ZoneDatabase.Count == 1);
                var badRecords = Path.Combine(directory.FullName, "bad-records.xml");
                File.WriteAllText(badRecords, "<Records><Record Type='99' StartTime='0' EndTime='0' Label='Bad'/></Records>");
                Throws<XmlException>(() => root.LoadXmlRecords(badRecords));
                Check(root.EncDatabase.Count == 1 && root.ZoneDatabase.Count == 1);
                root.EncDatabase.Clear(); root.ZoneDatabase.Clear();
                root.ChangeZone("Fixture Zone"); root.SetEncounter(epoch, "Source", "Target");
                var action = new MasterSwing(1, true, "Special", new Dnum(1234, "label"), epoch.AddSeconds(2), 1, "Strike", "Source", "Physical", "Target");
                action.Tags["unicode"] = "한글 & <test>";
                root.AddCombatAction(action); host.EndCombat();
                var encounter = root.ActiveZone.ActiveEncounter!;
                encounter.LogLines.Add(new(epoch, "raw log", 12, 17));
                var id = host.ListEncounters(0, 16).Items.Single().Id;
                Check(host.ArchiveOperation(2, output, id, true) == 1);
                var exported = File.ReadAllBytes(output);
                Throws<IOException>(() => host.ArchiveOperation(2, output, id, true));
                Check(File.ReadAllBytes(output).SequenceEqual(exported));
                for (var missing = 1; missing <= 8; missing++)
                    Throws<InvalidDataException>(() => root.MemDecompressGzip(exported[..^missing]));
                Check(host.ArchiveOperation(3, "", 0, false) == 1);
                Check(host.ArchiveOperation(3, "", 0, false) == 1);
                saved = File.ReadAllText(index);
                using (var checkpoint = new FileStream(Path.Combine(directory.FullName, "history", "checkpoint.lock"), FileMode.Open, FileAccess.ReadWrite, FileShare.None))
                    Throws<IOException>(() => host.ArchiveOperation(3, "", 0, false));
                Check(File.ReadAllText(index) == saved);
                // Unsupported plugin tag objects must fail the checkpoint and preserve its last complete index.
                action.Tags["unsupported"] = new FileInfo(output);
                Throws<InvalidDataContractException>(() => host.ArchiveOperation(3, "", 0, false));
                Check(File.ReadAllText(index) == saved);
                action.Tags.Remove("unsupported");
                var before = host.ListEncounters(0, 16).Total;
                var invalid = Path.Combine(directory.FullName, "bad.xml");
                var nilTags = new XmlDocument();
                nilTags.LoadXml(Encoding.UTF8.GetString(root.MemDecompressGzip(exported)));
                var tags = (XmlElement)nilTags.SelectSingleNode("//*[local-name()='ArrayOfKeyValueOfstringanyType']")!;
                tags.RemoveAll(); tags.SetAttribute("nil", "http://www.w3.org/2001/XMLSchema-instance", "true");
                File.WriteAllText(invalid, nilTags.OuterXml);
                Throws<XmlException>(() => host.ArchiveOperation(1, invalid, 0, false));
                File.WriteAllText(invalid, "<Encounters><Encounter Name='valid'/><Encounter Name='bad'><Swing/></Encounter></Encounters>");
                Throws<XmlException>(() => host.ArchiveOperation(1, invalid, 0, false));
                Check(host.ListEncounters(0, 16).Total == before);
                File.WriteAllText(invalid, "<!DOCTYPE Encounters [<!ENTITY x SYSTEM 'file:///etc/hostname'>]><Encounters/>");
                Throws<XmlException>(() => host.ArchiveOperation(1, invalid, 0, false));
                Check(host.ArchiveOperation(1, output, 0, false) == 1);
                Check(host.ListEncounters(0, 16).Total == before + 1);
            }
            using (var restarted = new HostEnvironment(directory.FullName))
            {
                CombatFixture.ConfigureCategories();
                Check(restarted.ListEncounters(0, 16).Total == 0);
                Check(restarted.ArchiveOperation(4, "no match", 0, false) == 0);
                Check(restarted.ArchiveOperation(4, "FIXTURE", 0, false) == 1);
                Check(restarted.ArchiveOperation(4, "", 0, false) == 0);
                var root = ActGlobals.oFormActMain!;
                var encounter = root.ZoneList[0].Items.Single();
                var action = encounter.GetCombatant("Source")!.AllOut["All"].Items.Single();
                Check(action.Damage.Number == 1234 && action.Damage.ToString() == "label" && action.Time == epoch.AddSeconds(2));
                Check(encounter.StartTimes.Single() == epoch.AddSeconds(2) && encounter.ZoneName == "Fixture Zone");
                Check((string)action.Tags["unicode"] == "한글 & <test>" && encounter.LogLines.Single().LogLine == "raw log");
                Check(restarted.ListEncounters(0, 16).Items.Single().Damage == 1234);
                Check(restarted.ArchiveOperation(3, "", 0, false) == 1);
                var corrupt = Directory.GetFiles(Path.Combine(directory.FullName, "history"), "*.act").First();
                File.WriteAllBytes(corrupt, [1, 2, 3]);
            }
            using (var broken = new HostEnvironment(directory.FullName))
            {
                CombatFixture.ConfigureCategories();
                Throws<InvalidDataException>(() => broken.ArchiveOperation(4, "", 0, false));
                Check(broken.ListEncounters(0, 16).Total == 0);
            }
            File.WriteAllText(index, saved.Replace("Archive=\"", "Archive=\"../"));
            Throws<XmlException>(() => { using var invalid = new HostEnvironment(directory.FullName); });
            Check(ActGlobals.oFormActMain is null);
            Console.WriteLine("ACT XML/gzip imports, atomic history checkpoints, filtered restart restoration and corruption boundaries passed.");
        }
        finally { directory.Delete(true); }
    }
}
