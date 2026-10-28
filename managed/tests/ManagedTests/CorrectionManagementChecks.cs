using System.Xml;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Testing;

internal static class CorrectionManagementChecks
{
    private static void Check(bool value, string message) { if (!value) throw new Exception(message); }
    private static void Reject(Action action)
    {
        try { action(); }
        catch (Exception error) when (error is ArgumentException or InvalidOperationException or IOException or UnauthorizedAccessException or XmlException) { return; }
        throw new Exception("Expected correction rejection");
    }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-correction-editor-");
        var path = Path.Combine(directory.FullName, "DataCorrections.xml");
        try
        {
            using (var host = new HostEnvironment(directory.FullName))
            {
                CombatFixture.ConfigureCategories(); CombatantData.DamageSwingTypes = [1];
                var root = ActGlobals.oFormActMain!;
                var initial = host.ReadCorrections();
                Check(initial.Editable && initial.Renames.Length == 0 && initial.Redirects.Length == 0, "empty state");
                var saved = host.SaveCorrections(initial.Revision, [new("Pet", "Owner", true)], [new("Strike", "Final", 1, true)]);
                var bytes = File.ReadAllBytes(path);
                Reject(() => host.SaveCorrections(initial.Revision, [], []));
                Reject(() => host.SaveCorrections(saved.Revision, [new(" ", "X", true)], []));
                Reject(() => host.SaveCorrections(saved.Revision, [new("A -> B", "C", true), new("A", "B -> C", true)], []));
                Reject(() => host.SaveCorrections(saved.Revision, [], [new("A", "B", 1, true), new("A", "B", 2, true)]));
                Check(File.ReadAllBytes(path).SequenceEqual(bytes) && host.ReadCorrections().Revision == saved.Revision, "failed edit mutated settings");
                File.WriteAllText(path, "<DataCorrections/>");
                Reject(() => host.SaveCorrections(saved.Revision, [], []));
                Check(host.ReadCorrections().Renames.Single().After == "Owner", "external change mutated runtime");
                File.WriteAllBytes(path, bytes);
                File.Move(path, path + ".saved"); Directory.CreateDirectory(path);
                try { Reject(() => host.SaveCorrections(saved.Revision, [], [])); Check(host.ReadCorrections().Revision == saved.Revision, "write failure mutated runtime"); }
                finally { Directory.Delete(path); File.Move(path + ".saved", path); }
                var seen = new List<(string, string)>();
                root.BeforeCombatAction += (_, info) => seen.Add((info.attacker, info.combatAction.Attacker));
                root.TimeStampLen = 0;
                root.OnLogLineRead += (import, _) => {
                    if (!import) return;
                    var time = DateTime.UnixEpoch;
                    if (root.SetEncounter(time, "Pet", "Target"))
                        root.AddCombatAction(new MasterSwing(1, false, "None", new Dnum(100), time, root.GlobalTimeSorter, "Strike", "Pet", "Physical", "Target"));
                };
                var log = Path.Combine(directory.FullName, "synthetic.log"); File.WriteAllText(log, "one\n");
                host.ImportLog(log);
                Check(seen.Single() == ("Final", "Pet") && root.ZoneList[0].Items.Single().GetCombatant("Final")!.Damage == 100, "pre-callback correction");
                var second = host.SaveCorrections(saved.Revision, [new("Pet", "Other", true)], []);
                host.ImportLog(log);
                Check(seen.Last() == ("Other", "Pet") && root.ZoneList[0].Items[0].GetCombatant("Final")!.Damage == 100 && root.ZoneList[0].Items[1].GetCombatant("Other")!.Damage == 100, "edit changed old graph or did not affect reparse");
                var cycle = host.SaveCorrections(second.Revision, [new("A", "B", true), new("B", "A", true)], []);
                Check(cycle.Renames.Length == 2, "finite ACT one-pass cycle rejected");
                Check(host.ArchiveOperation(3, "", 0, false) == 2, "checkpoint");
                root.SetEncounter(DateTime.UnixEpoch, "A", "Target");
                root.AddCombatAction(new MasterSwing(1, false, "None", new Dnum(1), DateTime.UnixEpoch, 99, "Strike", "A", "Physical", "Target"));
                host.EndCombat();
                Check(seen.Last() == ("A", "A"), "sorted cycle must run once and terminate");
                host.SaveCorrections(cycle.Revision, [], []);
            }
            using (var host = new HostEnvironment(directory.FullName))
            {
                CombatFixture.ConfigureCategories();
                Check(host.ReadCorrections().Renames.Length == 0 && host.ReadCorrections().Redirects.Length == 0, "deletion did not survive restart");
                Check(host.ArchiveOperation(4, "", 0, false) == 2, "history restore");
                var names = ActGlobals.oFormActMain!.ZoneList[0].Items.SelectMany(item => item.Items.Values).Select(actor => actor.Name).ToArray();
                Check(names.Contains("Final") && names.Contains("Other"), "history re-applied current corrections");
            }
            File.WriteAllText(path, "<DataCorrections><Unknown data='preserve'/></DataCorrections>");
            using (var host = new HostEnvironment(directory.FullName))
            {
                var snapshot = host.ReadCorrections(); Check(!snapshot.Editable, "unsupported XML not signalled");
                Reject(() => host.SaveCorrections(snapshot.Revision, [], []));
                Check(File.ReadAllText(path).Contains("Unknown", StringComparison.Ordinal), "unsupported XML overwritten");
            }
            Console.WriteLine("Correction edits, sorted rule identity, failure atomicity, callbacks, reparse and history passed.");
        }
        finally { directory.Delete(true); }
    }
}
