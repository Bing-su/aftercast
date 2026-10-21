using Advanced_Combat_Tracker;

namespace Aftercast.LegacyReference;

public static class DefaultGlobalsProbe
{
    public static string Run()
    {
        // Distinguish bare static defaults from the main host's later category-key setup.
        var categories = new[]
        {
            CombatantData.DamageTypeDataNonSkillDamage, CombatantData.DamageTypeDataOutgoingDamage,
            CombatantData.DamageTypeDataOutgoingHealing, CombatantData.DamageTypeDataIncomingDamage,
            CombatantData.DamageTypeDataIncomingHealing, CombatantData.DamageTypeDataOutgoingPowerReplenish,
            CombatantData.DamageTypeDataOutgoingPowerDamage, CombatantData.DamageTypeDataOutgoingCures
        };
        if (ActGlobals.charName != "YOU" || categories.Any(value => value != string.Empty))
            throw new Exception("ACT default alias/category state did not match");
        return "Default globals";
    }
}
