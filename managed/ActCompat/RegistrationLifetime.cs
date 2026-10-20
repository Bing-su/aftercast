namespace Advanced_Combat_Tracker;

internal static class RegistrationLifetime
{
    internal static void Reset()
    {
        // Release callbacks from the previous plugin session without mutating a consumer's retained dictionary.
        CombatantData.IncomingDamageTypeDataObjects = new();
        CombatantData.OutgoingDamageTypeDataObjects = new();
        CombatantData.SwingTypeToDamageTypeDataLinksIncoming = new();
        CombatantData.SwingTypeToDamageTypeDataLinksOutgoing = new();
        CombatantData.DamageSwingTypes = new();
        CombatantData.HealingSwingTypes = new();
        CombatantData.DamageTypeDataOutgoingDamage = "";
        CombatantData.DamageTypeDataIncomingDamage = "";
        CombatantData.DamageTypeDataOutgoingHealing = "";
        CombatantData.DamageTypeDataIncomingHealing = "";
        CombatantData.DamageTypeDataNonSkillDamage = "";
        CombatantData.DamageTypeDataOutgoingPowerReplenish = "";
        CombatantData.DamageTypeDataOutgoingPowerDamage = "";
        CombatantData.DamageTypeDataOutgoingCures = "";
        // Each host starts with ACT's built-in calculation defaults, not the previous plugin's options.
        ActGlobals.mainTableShowCommas = true;
        ActGlobals.blockIsHit = true;
        ActGlobals.disableIncrementalCaching = false;
        ActGlobals.restrictToAll = false;
        ActGlobals.longDuration = false;
        CombatantData.ExportVariables = new();
        CombatantData.ColumnDefs = new();
        EncounterData.ExportVariables = new();
        EncounterData.ColumnDefs = new();
        MasterSwing.ColumnDefs = new();
        AttackType.ColumnDefs = new();
        DamageTypeData.ColumnDefs = new();
    }
}
