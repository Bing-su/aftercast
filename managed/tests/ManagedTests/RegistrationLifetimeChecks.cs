using System.Drawing;
using Advanced_Combat_Tracker;
using Bxivact.ActCompat;

internal static class RegistrationLifetimeChecks
{
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Registration lifetime check failed");
    }

    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("bxivact-registration-");
        try
        {
            var calls = 0;
            Dictionary<string, CombatantData.ColumnDef> retained;
            using (var first = new HostEnvironment(directory.FullName))
            {
                CombatantData.IncomingDamageTypeDataObjects["old"] = new("Old", 1, Color.Red);
                CombatantData.OutgoingDamageTypeDataObjects["old"] = new("Old", 1, Color.Red);
                CombatantData.SwingTypeToDamageTypeDataLinksIncoming[1] = ["old"];
                CombatantData.SwingTypeToDamageTypeDataLinksOutgoing[1] = ["old"];
                CombatantData.DamageSwingTypes.Add(1);
                CombatantData.HealingSwingTypes.Add(2);
                CombatantData.DamageTypeDataOutgoingDamage = "old outgoing damage";
                CombatantData.DamageTypeDataIncomingDamage = "old incoming damage";
                CombatantData.DamageTypeDataOutgoingHealing = "old outgoing healing";
                CombatantData.DamageTypeDataIncomingHealing = "old incoming healing";
                CombatantData.DamageTypeDataNonSkillDamage = "old non-skill damage";
                CombatantData.DamageTypeDataOutgoingPowerReplenish = "old power replenish";
                CombatantData.DamageTypeDataOutgoingPowerDamage = "old power damage";
                CombatantData.DamageTypeDataOutgoingCures = "old cures";
                ActGlobals.mainTableShowCommas = false;
                ActGlobals.blockIsHit = false;
                ActGlobals.disableIncrementalCaching = true;
                ActGlobals.restrictToAll = true;
                ActGlobals.longDuration = true;
                CombatantData.ExportVariables["old"] = new("old", "Old", "Old session", (_, _) => "old");
                EncounterData.ExportVariables["old"] = new("old", "Old", "Old session", (_, _, _) => "old");
                CombatantData.ColumnDefs["old"] = new("Old", true, "TEXT", "old", _ => { calls++; return "old"; }, _ => "old", (_, _) => 0);
                EncounterData.ColumnDefs["old"] = new("Old", true, "TEXT", "old", _ => "old", _ => "old");
                MasterSwing.ColumnDefs["old"] = new("Old", true, "TEXT", "old", _ => "old", _ => "old", (_, _) => 0);
                AttackType.ColumnDefs["old"] = new("Old", true, "TEXT", "old", _ => "old", _ => "old", (_, _) => 0);
                DamageTypeData.ColumnDefs["old"] = new("Old", true, "TEXT", "old", _ => "old", _ => "old");
                retained = CombatantData.ColumnDefs;
                try
                {
                    using var rejected = new HostEnvironment(directory.FullName);
                    throw new Exception("Duplicate host unexpectedly acquired ACT globals");
                }
                catch (InvalidOperationException)
                {
                    Check(ReferenceEquals(retained, CombatantData.ColumnDefs) && retained.ContainsKey("old"));
                }
            }

            CheckEmpty();
            using (var second = new HostEnvironment(directory.FullName))
            {
                CheckEmpty(hostInitialized: true);
                Check(!ReferenceEquals(retained, CombatantData.ColumnDefs) && retained.ContainsKey("old"));
                // A new session must not invoke a callback that belonged to a disposed plugin session.
                var zone = new ZoneData(DateTime.UnixEpoch, "Fixture", false, false, false);
                var combatant = new CombatantData("Fixture", new EncounterData("Fixture", "Fixture", zone));
                Check(combatant.GetColumnByName("old") == "" && calls == 0);
            }
        }
        finally { directory.Delete(true); }
    }

    private static void CheckEmpty(bool hostInitialized = false)
    {
        Check(CombatantData.IncomingDamageTypeDataObjects.Count == 0);
        Check(CombatantData.OutgoingDamageTypeDataObjects.Count == 0);
        Check(CombatantData.SwingTypeToDamageTypeDataLinksIncoming.Count == 0);
        Check(CombatantData.SwingTypeToDamageTypeDataLinksOutgoing.Count == 0);
        Check(CombatantData.DamageSwingTypes.Count == 0 && CombatantData.HealingSwingTypes.Count == 0);
        Check(CombatantData.DamageTypeDataOutgoingDamage == (hostInitialized ? "Outgoing Damage" : ""));
        Check(CombatantData.DamageTypeDataIncomingDamage == (hostInitialized ? "Incoming Damage" : ""));
        Check(CombatantData.DamageTypeDataOutgoingHealing == (hostInitialized ? "Healed (Out)" : ""));
        Check(CombatantData.DamageTypeDataIncomingHealing == (hostInitialized ? "Healed (Inc)" : ""));
        Check(CombatantData.DamageTypeDataNonSkillDamage == (hostInitialized ? "Auto-Attack (Out)" : ""));
        Check(CombatantData.DamageTypeDataOutgoingCures == (hostInitialized ? "Cure/Dispel (Out)" : ""));
        Check(CombatantData.DamageTypeDataOutgoingPowerReplenish == (hostInitialized ? "Power Replenish (Out)" : ""));
        Check(CombatantData.DamageTypeDataOutgoingPowerDamage == (hostInitialized ? "Power Drain (Out)" : ""));
        Check(ActGlobals.mainTableShowCommas && ActGlobals.blockIsHit && !ActGlobals.disableIncrementalCaching);
        Check(!ActGlobals.restrictToAll && !ActGlobals.longDuration);
        Check(CombatantData.ExportVariables.Count == 0 && EncounterData.ExportVariables.Count == 0);
        Check(CombatantData.ColumnDefs.Count == 0 && EncounterData.ColumnDefs.Count == 0);
        Check(MasterSwing.ColumnDefs.Count == 0 && AttackType.ColumnDefs.Count == 0);
        Check(DamageTypeData.ColumnDefs.Count == (hostInitialized ? 1 : 0));
        if (hostInitialized) Check(DamageTypeData.ColumnDefs.ContainsKey("ToHit"));
    }
}
