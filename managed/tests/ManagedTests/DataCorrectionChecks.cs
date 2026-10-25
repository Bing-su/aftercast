using System.Xml;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;
using Aftercast.Testing;

internal static class DataCorrectionChecks
{
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-corrections-");
        var path = Path.Combine(directory.FullName, "DataCorrections.xml");
        const string configuration = "<DataCorrections><RenameFix><Rename Before='Pet' After='Owner'/><Rename Before='Target' After='Enemy'/></RenameFix><AbilityRedirectFix><Redirect Ability='Strike' Into='Player' Type='1'/></AbilityRedirectFix></DataCorrections>";
        try
        {
            File.WriteAllText(path, configuration);
            using (var environment = new HostEnvironment(directory.FullName))
            {
                CombatFixture.ConfigureCategories();
                CombatantData.DamageSwingTypes = [1];
                var root = ActGlobals.oFormActMain!;
                var before = false;
                var after = false;
                var importedHook = false;
                root.BeforeCombatAction += (isImport, info) =>
                {
                    if (info.attacker != "Player" || info.victim != "Enemy" || info.combatAction.Attacker != "Pet")
                        throw new Exception("Corrections must precede hooks without committing original action scalars");
                    before = true;
                    importedHook |= isImport;
                    info.attacker = "Final";
                };
                root.AfterCombatAction += (_, info) => after = info.attacker == "Final" && info.victim == "Enemy";
                var epoch = DateTime.UnixEpoch;
                root.SetEncounter(epoch, "Pet", "Target");
                root.AddCombatAction(new MasterSwing(1, false, "", new Dnum(100), epoch, 1, "Strike", " Pet ", "Physical", "Target"));
                environment.EndCombat();
                var encounter = root.ActiveZone.ActiveEncounter!;
                if (!before || !after || encounter.GetCombatant("Final")?.Damage != 100 || encounter.GetCombatant("Enemy")?.DamageTaken != 100 || environment.DiagnosticsSnapshot.Item1 != 0)
                    throw new Exception("Corrected actions did not reach aggregation cleanly");
                var log = Path.Combine(directory.FullName, "import.log");
                File.WriteAllText(log, "fixture\n");
                root.OnLogLineRead += (isImport, _) =>
                {
                    if (!isImport) return;
                    root.SetEncounter(epoch, "Pet", "Target");
                    root.AddCombatAction(new MasterSwing(1, false, "", new Dnum(100), epoch, 2, "Strike", "Pet", "Physical", "Target"));
                };
                environment.ImportLog(log);
                if (!importedHook || root.ZoneList[0].Items[^1].GetCombatant("Final")?.Damage != 100 || environment.DiagnosticsSnapshot.Item1 != 0)
                    throw new Exception("Imported actions bypassed data corrections");
            }
            if (File.ReadAllText(path) != configuration) throw new Exception("Read-only correction settings were overwritten");
            File.WriteAllText(path, "<DataCorrections><AbilityRedirectFix><Redirect Ability='Strike' Into='Player' Type='bad'/></AbilityRedirectFix></DataCorrections>");
            try { using var invalid = new HostEnvironment(directory.FullName); throw new Exception("Malformed correction accepted"); }
            catch (FormatException) { }
            if (ActGlobals.oFormActMain is not null) throw new Exception("Failed configuration leaked host globals");
            File.WriteAllText(path, "<!DOCTYPE DataCorrections [<!ENTITY x SYSTEM 'file:///etc/hostname'>]><DataCorrections/>");
            try { using var invalid = new HostEnvironment(directory.FullName); throw new Exception("Correction DTD accepted"); }
            catch (XmlException) { }
            Console.WriteLine("Data correction configuration, hook order, aggregation and invalid settings checks passed.");
        }
        finally { directory.Delete(true); }
    }
}
