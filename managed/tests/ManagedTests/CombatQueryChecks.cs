using System.Runtime.CompilerServices;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Testing;

internal static class CombatQueryChecks
{
    private static void Check(bool condition, [CallerArgumentExpression(nameof(condition))] string? expression = null)
    {
        if (!condition) throw new Exception("Combat query check failed: " + expression);
    }

    internal static void Run() => HostLifetimeChecks.RunBounded(() =>
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-combat-query-");
        try
        {
            using var host = new HostEnvironment(directory.FullName);
            var root = ActGlobals.oFormActMain!;
            CombatFixture.ConfigureCategories();
            Check(host.ReadEncounter(0, true) is null);
            Check(host.ListEncounters(0, 16).Total == 0);
            void Add(int second, long amount)
            {
                root.SetEncounter(DateTime.UnixEpoch.AddSeconds(second), "Source", "Target");
                root.AddCombatAction(new(1, false, "None", new Dnum(amount), DateTime.UnixEpoch.AddSeconds(second),
                    second, "Strike 한글", "Source", "Physical", "Target"));
            }
            Add(0, 100);
            Add(2, 300);
            EncounterDetailData live = null!;
            // Drain fixture actions before asserting values; production queries deliberately do not join producers.
            host.RunWithLogReadingSuspended(() =>
            {
                live = host.ReadEncounter(0, true)!;
                Check(live.Summary.Status == EncounterStatus.Active && live.Summary.Damage == 400 && live.Combatants.Length == 2);
                Check(live.Combatants.Single(actor => actor.Name == "Source").EncDPS == 200);
            });
            var id = live.Summary.Id;
            var page = host.ListEncounters(0, 16);
            Check(page.Total == 1 && page.Items.Single().Id == id);
            Check(host.ListEncounters(1, 1).Items.Length == 0);
            Check(host.ReadEncounter(ulong.MaxValue, false) is null);
            Check(host.ReadEncounterAttacks(ulong.MaxValue, 0, 16) is null);
            root.EndCombat(true);
            Check(host.ReadEncounter(0, true) is null);
            var ended = host.ReadEncounter(id, false)!;
            Check(ended.Summary.Status == EncounterStatus.Ended && ended.Summary.Id == id && ended.Summary.Damage == 400);
            var attacks = host.ReadEncounterAttacks(id, 0, 256)!;
            var strike = attacks.Items.Single(value => value.Combatant == "Source" && value.Category == "Damage" && value.Attack == "Strike 한글");
            Check(strike.Outgoing && strike.Damage == 400 && strike.Hits == 2 && strike.EncDPS == 200);
            Check(attacks.Items.Any(value => !value.Outgoing && value.Combatant == "Target"));
            Check(host.ReadEncounterAttacks(id, 1, 1)!.Items.Single() == attacks.Items[1]);
            Check(host.ReadEncounterAttacks(id, int.MaxValue, 1)!.Items.Length == 0);
            Add(4, 7);
            root.EndCombat(true);
            Check(host.ListEncounters(0, 16).Total == 2);
            Check(host.ReadEncounter(id, false)!.Summary.Damage == 400 && live.Summary.Status == EncounterStatus.Active);
            // Removing a public ACT graph entry makes its ID absent without renumbering another encounter.
            var zone = root.ZoneList.Single(value => value.Items.Any(item => item.Items.Count != 0));
            var newer = host.ListEncounters(1, 1).Items.Single().Id;
            zone.Items.RemoveAt(zone.PopulateAll ? 1 : 0);
            Check(host.ReadEncounter(id, false) is null && host.ReadEncounter(newer, false) is not null);
            Check(host.ReadEncounterAttacks(id, 0, 1) is null);
            Check(live.Summary.Damage == 400 && strike.Damage == 400);
            CheckImports(host, root, directory.FullName);
        }
        finally { directory.Delete(true); }
        Console.WriteLine("Live combat, stable encounter IDs, owned detail and attack page checks passed.");
    });

    private static void CheckImports(HostEnvironment host, FormActMain root, string directory)
    {
        var path = Path.Combine(directory, "query-import.log");
        root.GetDateTimeFromLog = _ => DateTime.UnixEpoch;
        root.OnLogLineRead += (import, _) =>
        {
            if (!import) return;
            root.SetEncounter(DateTime.UnixEpoch, "Source", "Target");
            root.AddCombatAction(new(1, false, "None", new Dnum(25), DateTime.UnixEpoch,
                99, "Import", "Source", "Physical", "Target"));
        };
        var observed = 0;
        root.OnCombatEnd += (import, _) =>
        {
            if (!import) return;
            var importing = host.ListEncounters(0, 256).Items.Where(item => item.Status == EncounterStatus.Importing).ToArray();
            Check(importing.Length == 1 && importing[0].IsImport);
            Check(host.ReadEncounter(importing[0].Id, false)!.Summary.Status == EncounterStatus.Importing);
            Check(host.ReadEncounter(0, true) is null);
            observed++;
        };
        File.WriteAllText(path, "valid\n");
        host.ImportLog(path);
        var first = host.ListEncounters(0, 256).Items.Single(item => item.IsImport);
        Check(first.Status == EncounterStatus.Ended);
        // A later failed import must mark only its new encounter; earlier successful history stays complete.
        File.WriteAllBytes(path, [(byte)'v', (byte)'\n', 255]);
        try { host.ImportLog(path); throw new Exception("Invalid UTF-8 import succeeded"); }
        catch (InvalidDataException) { }
        Check(observed == 2);
        var imported = host.ListEncounters(0, 256).Items.Where(item => item.IsImport).ToArray();
        Check(imported.Length == 2 && imported[0].Id == first.Id);
        Check(imported[0].Status == EncounterStatus.Ended && imported[1].Status == EncounterStatus.PartialImport);
        Check(host.ReadEncounter(imported[1].Id, false)!.Summary.Damage == 25);
    }
}
