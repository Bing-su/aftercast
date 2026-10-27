using System.Runtime.Serialization;
using System.Security.Cryptography;
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
        // Match Rust's canonical Windows paths to cover history loading after a worker restart.
        if (OperatingSystem.IsWindows()) directory = new DirectoryInfo(@"\\?\" + directory.FullName);
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
                CheckGzip(root);
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
                var action = new MasterSwing(1, true, "Special", new Dnum(1234, "label"), epoch.AddSeconds(2), 42, "Strike", "Source", "Physical", "Target");
                action.Tags["unicode"] = "한글 & <test>";
                root.AddCombatAction(action); host.EndCombat();
                var encounter = root.ActiveZone.ActiveEncounter!;
                encounter.LogLines.Add(new(epoch.AddSeconds(2), "other log", 12, 17));
                encounter.LogLines.Add(new(epoch.AddSeconds(2), "raw log", 12, 42));
                var id = host.ListEncounters(0, 16).Items.Single().Id;
                Check(host.ArchiveOperation(2, output, id, true) == 1);
                var exported = File.ReadAllBytes(output);
                var xmlOutput = Path.Combine(directory.FullName, "export.xml");
                Check(host.ArchiveOperation(2, xmlOutput, id, false) == 1);
                Check(File.ReadAllBytes(xmlOutput).SequenceEqual(root.MemDecompressGzip(exported)));
                Check(!File.ReadAllText(xmlOutput).Contains("Aftercast", StringComparison.Ordinal));
                Throws<IOException>(() => host.ArchiveOperation(2, output, id, true));
                Check(File.ReadAllBytes(output).SequenceEqual(exported));
                for (var missing = 1; missing <= 8; missing++)
                    Throws<InvalidDataException>(() => root.MemDecompressGzip(exported[..^missing]));
                Check(host.ArchiveOperation(3, "", 0, false) == 1);
                Check(host.ArchiveOperation(3, "", 0, false) == 1);
                saved = File.ReadAllText(index);
                PrepareInvalidHistory(directory, saved, document => document.DocumentElement!.RemoveAttribute("AftercastHistoryVersion"));
                PrepareInvalidHistory(directory, saved, document => document.DocumentElement!.SetAttribute("AftercastHistoryVersion", "2"));
                PrepareInvalidHistory(directory, saved, document => ((XmlElement)document.SelectSingleNode("//LogLine")!).RemoveAttribute("AftercastTimeSorter"));
                PrepareInvalidHistory(directory, saved, document => ((XmlElement)document.SelectSingleNode("//LogLine")!).SetAttribute("AftercastTimeSorter", "2147483648"));
                PrepareInvalidHistory(directory, saved, document => ((XmlElement)document.SelectSingleNode("//Swing")!).RemoveAttribute("TimeSorter"));
                PrepareInvalidHistory(directory, saved, document => ((XmlElement)document.SelectSingleNode("//Swing")!).SetAttribute("TimeSorter", "invalid"));
                PrepareInvalidHistory(directory, saved, document => ((XmlElement)document.SelectSingleNode("//Encounter")!).SetAttribute("AftercastDroppedLogLines", "-1"));
                var manual = new EncounterData("Source", "Fixture Zone", root.ActiveZone);
                manual.Items["GHOST"] = new CombatantData("Ghost", manual);
                root.ActiveZone.Items.Add(manual);
                Throws<InvalidOperationException>(() => host.ArchiveOperation(3, "", 0, false));
                root.ActiveZone.Items.Remove(manual);
                Check(File.ReadAllText(index) == saved);
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
            CheckInvalidHistories(directory);
            using (var restarted = new HostEnvironment(directory.FullName))
            {
                CombatFixture.ConfigureCategories();
                Check(restarted.ListEncounters(0, 16).Total == 0);
                Check(restarted.ArchiveOperation(4, "no match", 0, false) == 0);
                ActGlobals.oFormActMain!.GlobalTimeSorter = 900;
                Check(restarted.ArchiveOperation(4, "FIXTURE", 0, false) == 1);
                Check(ActGlobals.oFormActMain!.GlobalTimeSorter == 900);
                Check(restarted.ArchiveOperation(4, "", 0, false) == 0);
                var root = ActGlobals.oFormActMain!;
                var encounter = root.ZoneList[0].Items.Single();
                var action = encounter.GetCombatant("Source")!.AllOut["All"].Items.Single();
                Check(action.Damage.Number == 1234 && action.Damage.ToString() == "label" && action.Time == epoch.AddSeconds(2));
                Check(encounter.StartTimes.Single() == epoch.AddSeconds(2) && encounter.ZoneName == "Fixture Zone");
                Check((string)action.Tags["unicode"] == "한글 & <test>" && encounter.LogLines.Count == 2);
                Check(encounter.LogLines.Select(line => line.GlobalTimeSorter).SequenceEqual(new[] { 17, 42 }));
                Check(encounter.LogLines.All(line => line.Time == action.Time));
                Check(encounter.LogLines.Single(line => line.GlobalTimeSorter == action.TimeSorter).LogLine == "raw log");
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
            var partialDirectory = Directory.CreateDirectory(Path.Combine(directory.FullName, "partial-state"));
            var partialLog = Path.Combine(directory.FullName, "partial.log");
            File.WriteAllText(partialLog, "fixture\n");
            using (var partial = new HostEnvironment(partialDirectory.FullName))
            {
                CombatFixture.ConfigureCategories();
                var root = ActGlobals.oFormActMain!;
                root.OnLogLineRead += (_, _) =>
                {
                    root.SetEncounter(epoch, "Source", "Target");
                    root.AddCombatAction(new MasterSwing(1, false, "", new Dnum(100), epoch, 1, "Strike", "Source", "Physical", "Target"));
                    throw new InvalidOperationException("Fixture import failure after accepting an action");
                };
                Throws<InvalidDataException>(() => partial.ImportLog(partialLog));
                Check(partial.ArchiveOperation(3, "", 0, false) == 1);
                Check(root.EncDatabase.Values.Single().FolderHint == directory.FullName);
            }
            using (var partial = new HostEnvironment(partialDirectory.FullName))
            {
                CombatFixture.ConfigureCategories();
                Check(partial.ArchiveOperation(4, "", 0, false) == 1);
                Check(partial.ListEncounters(0, 16).Items.Single().Status == EncounterStatus.PartialImport);
            }
            Console.WriteLine("ACT XML/gzip imports, atomic history checkpoints, filtered restart restoration and corruption boundaries passed.");
        }
        finally { directory.Delete(true); }
    }

    private static void PrepareInvalidHistory(DirectoryInfo source, string index, Action<XmlDocument> mutate)
    {
        // Give malformed private payloads valid hashes, e.g. format checks must run beyond integrity checks.
        var directory = Directory.CreateDirectory(Path.Combine(source.FullName, "invalid-" + Guid.NewGuid()));
        var history = Directory.CreateDirectory(Path.Combine(directory.FullName, "history"));
        var document = new XmlDocument();
        var root = ActGlobals.oFormActMain!;
        var original = Directory.GetFiles(Path.Combine(source.FullName, "history"), "*.act").Single();
        document.LoadXml(Encoding.UTF8.GetString(root.MemDecompressGzip(File.ReadAllBytes(original))));
        mutate(document);
        var payload = root.MemCompressGZip(Encoding.UTF8.GetBytes(document.OuterXml));
        var file = Convert.ToHexStringLower(SHA256.HashData(payload)) + ".act";
        File.WriteAllBytes(Path.Combine(history.FullName, file), payload);
        document.LoadXml(index);
        ((XmlElement)document.SelectSingleNode("/Records/Record")!).SetAttribute("Archive", file);
        document.Save(Path.Combine(history.FullName, "index.xml"));
    }

    private static void CheckInvalidHistories(DirectoryInfo source)
    {
        foreach (var directory in source.GetDirectories("invalid-*"))
        {
            using var host = new HostEnvironment(directory.FullName);
            CombatFixture.ConfigureCategories();
            var root = ActGlobals.oFormActMain!;
            root.GlobalTimeSorter = 900;
            try { host.ArchiveOperation(4, "", 0, false); throw new Exception("Invalid private history accepted"); }
            catch (Exception error) when (error is XmlException or FormatException or OverflowException) { }
            Check(host.ListEncounters(0, 16).Total == 0 && root.GlobalTimeSorter == 900);
        }
    }

    private static void CheckGzip(FormActMain root)
    {
        // Produced independently by gzip -n for "123456789"; its IEEE CRC-32 is CBF43926.
        var known = Convert.FromHexString("1F8B080000000000000333343236313533B7B004002639F4CB09000000");
        Check(root.MemDecompressGzip(known).AsSpan().SequenceEqual("123456789"u8));
        var bytes = new byte[8192 * 3 + 17];
        for (var index = 0; index < bytes.Length; index++) bytes[index] = (byte)index;
        Check(root.MemDecompressGzip(root.MemCompressGZip(bytes)).SequenceEqual(bytes));
        var empty = Convert.FromHexString("1F8B080000000000000303000000000000000000");
        Check(root.MemDecompressGzip(empty).Length == 0);
        var compressedEmpty = root.MemCompressGZip([]);
        Check(compressedEmpty.AsSpan().SequenceEqual(empty));
        Check(root.MemDecompressGzip(compressedEmpty).Length == 0);
        foreach (var field in new[] { known.Length - 8, known.Length - 4 })
        {
            var corrupt = (byte[])known.Clone();
            corrupt[field] ^= 1;
            Throws<InvalidDataException>(() => root.MemDecompressGzip(corrupt));
        }
        Throws<ArgumentNullException>(() => root.MemCompressGZip(null!));
    }
}
