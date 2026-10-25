namespace Advanced_Combat_Tracker;

internal static class RegistrationLifetime
{
    internal static void InitializeHostCategoryNames()
    {
        // InitACT assigns these literal keys before plugins replace category definitions; bare ACT statics stay empty.
        CombatantData.DamageTypeDataNonSkillDamage = "Auto-Attack (Out)";
        CombatantData.DamageTypeDataOutgoingDamage = "Outgoing Damage";
        CombatantData.DamageTypeDataOutgoingHealing = "Healed (Out)";
        CombatantData.DamageTypeDataIncomingDamage = "Incoming Damage";
        CombatantData.DamageTypeDataIncomingHealing = "Healed (Inc)";
        CombatantData.DamageTypeDataOutgoingPowerReplenish = "Power Replenish (Out)";
        CombatantData.DamageTypeDataOutgoingPowerDamage = "Power Drain (Out)";
        CombatantData.DamageTypeDataOutgoingCures = "Cure/Dispel (Out)";
    }

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
        ActGlobals.selectiveList = new();
        CombatantData.ExportVariables = new();
        CombatantData.ColumnDefs = new();
        EncounterData.ExportVariables = new();
        EncounterData.ColumnDefs = new();
        MasterSwing.ColumnDefs = new();
        AttackType.ColumnDefs = new();
        DamageTypeData.ColumnDefs = new();
    }
}
