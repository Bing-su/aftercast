using System.Drawing;
using System.Runtime.CompilerServices;
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
                CheckEmpty();
                Check(!ReferenceEquals(retained, CombatantData.ColumnDefs) && retained.ContainsKey("old"));
                // A new session must not invoke a callback that belonged to a disposed plugin session.
                var combatant = (CombatantData)RuntimeHelpers.GetUninitializedObject(typeof(CombatantData));
                Check(combatant.GetColumnByName("old") == "" && calls == 0);
            }
        }
        finally { directory.Delete(true); }
    }

    private static void CheckEmpty()
    {
        Check(CombatantData.IncomingDamageTypeDataObjects.Count == 0);
        Check(CombatantData.OutgoingDamageTypeDataObjects.Count == 0);
        Check(CombatantData.SwingTypeToDamageTypeDataLinksIncoming.Count == 0);
        Check(CombatantData.SwingTypeToDamageTypeDataLinksOutgoing.Count == 0);
        Check(CombatantData.DamageSwingTypes.Count == 0 && CombatantData.HealingSwingTypes.Count == 0);
        Check(CombatantData.ExportVariables.Count == 0 && EncounterData.ExportVariables.Count == 0);
        Check(CombatantData.ColumnDefs.Count == 0 && EncounterData.ColumnDefs.Count == 0);
        Check(MasterSwing.ColumnDefs.Count == 0 && AttackType.ColumnDefs.Count == 0 && DamageTypeData.ColumnDefs.Count == 0);
    }
}
