using System.Drawing;
using System.Runtime.CompilerServices;
using Advanced_Combat_Tracker;

namespace Bxivact.LegacyReference;

// Probe registration metadata and callback delivery without creating or claiming aggregated combat data.
public static class RegistrationProbe
{
    private static void Check(bool condition)
    {
        if (!condition) throw new Exception("Original-reference registration probe failed");
    }

    public static string Run()
    {
        var damage = new CombatantData.DamageTypeDef("Incoming", -1, Color.Red);
        Check(damage.Label == "Incoming" && damage.AllyValue == -1 && damage.TypeColor == Color.Red);
        // An identity-only payload exercises delegates while deliberately avoiding unimplemented state constructors.
        var combatant = (CombatantData)RuntimeHelpers.GetUninitializedObject(typeof(CombatantData));
        var encounter = (EncounterData)RuntimeHelpers.GetUninitializedObject(typeof(EncounterData));
        var allies = new List<CombatantData> { combatant };
        var export = new CombatantData.TextExportFormatter("fixture", "Label", "Description",
            (data, extra) => ReferenceEquals(data, combatant) ? extra : throw new Exception("Wrong combatant"));
        Check(export.Name == "fixture" && export.Label == "Label" && export.Description == "Description");
        Check(export.GetExportString(combatant, "exported") == "exported");
        var encounterExport = new EncounterData.TextExportFormatter("encounter", "Encounter label", "Encounter description",
            (data, list, extra) => ReferenceEquals(data, encounter) && ReferenceEquals(list, allies) ? extra : throw new Exception("Wrong encounter"));
        Check(encounterExport.Name == "encounter" && encounterExport.Label == "Encounter label" && encounterExport.Description == "Encounter description");
        Check(encounterExport.GetExportString(encounter, allies, "encounter exported") == "encounter exported");

        var column = new CombatantData.ColumnDef("Cell", true, "VARCHAR(20)", "fixture_value",
            data => ReferenceEquals(data, combatant) ? "cell" : throw new Exception("Wrong cell payload"), _ => "sql", (_, _) => 7);
        Check(column.Label == "Cell" && column.DefaultVisible && column.SqlDataType == "VARCHAR(20)" && column.SqlDataName == "fixture_value");
        Check(column.GetCellData(combatant) == "cell" && column.GetSqlData(combatant) == "sql" && column.SortComparer(combatant, combatant) == 7);
        Check(column.GetCellForeColor(combatant) == Color.Transparent && column.GetCellBackColor(combatant) == Color.Transparent);
        column.GetCellData = _ => "changed callback";
        var masterColumn = new MasterSwing.ColumnDef("Master", false, "TEXT", "master", _ => "master", _ => "master sql", (_, _) => 1);
        var attackColumn = new AttackType.ColumnDef("Attack", false, "TEXT", "attack", _ => "attack", _ => "attack sql", (_, _) => 2);
        var damageColumn = new DamageTypeData.ColumnDef("Damage", true, "INT", "damage", _ => "damage", _ => "damage sql");
        var encounterColumn = new EncounterData.ColumnDef("Encounter", true, "INT", "encounter", _ => "encounter", _ => "encounter sql");
        Check(masterColumn.Label == "Master" && !masterColumn.DefaultVisible && masterColumn.SqlDataType == "TEXT" && masterColumn.SqlDataName == "master");
        Check(attackColumn.Label == "Attack" && !attackColumn.DefaultVisible && attackColumn.SqlDataType == "TEXT" && attackColumn.SqlDataName == "attack");
        Check(damageColumn.Label == "Damage" && damageColumn.DefaultVisible && damageColumn.SqlDataType == "INT" && damageColumn.SqlDataName == "damage");
        Check(encounterColumn.Label == "Encounter" && encounterColumn.DefaultVisible && encounterColumn.SqlDataType == "INT" && encounterColumn.SqlDataName == "encounter");

        const string key = "bxivact-fixture-registration";
        var columns = CombatantData.ColumnDefs;
        var definitions = CombatantData.IncomingDamageTypeDataObjects;
        var exports = EncounterData.ExportVariables;
        try
        {
            CombatantData.ColumnDefs = new() { [key] = column };
            CombatantData.IncomingDamageTypeDataObjects = new() { [key] = damage };
            EncounterData.ExportVariables = new() { [key] = encounterExport };
            Check(ReferenceEquals(CombatantData.IncomingDamageTypeDataObjects[key], damage));
            Check(EncounterData.ExportVariables[key].GetExportString(encounter, allies, "registered") == "registered");
            return combatant.GetColumnByName(key);
        }
        finally
        {
            CombatantData.ColumnDefs = columns;
            CombatantData.IncomingDamageTypeDataObjects = definitions;
            EncounterData.ExportVariables = exports;
        }
    }
}
