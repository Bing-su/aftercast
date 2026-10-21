using System.Drawing;
using Advanced_Combat_Tracker;

namespace Bxivact.Testing;

internal static class CombatFixture
{
    // Install categories inside a HostEnvironment lifetime; its teardown restores registry identity and callbacks.
    internal static void ConfigureCategories()
    {
        ActGlobals.charName = "Source";
        CombatantData.DamageTypeDataOutgoingDamage = "Damage";
        CombatantData.DamageTypeDataOutgoingHealing = "Healing";
        CombatantData.DamageTypeDataIncomingDamage = "Taken";
        CombatantData.DamageTypeDataIncomingHealing = "Received";
        CombatantData.OutgoingDamageTypeDataObjects = new()
        {
            ["Damage"] = new("Damage", -1, Color.Red),
            ["Healing"] = new("Healing", 1, Color.Green),
            ["All Out"] = new("All", 0, Color.Transparent)
        };
        CombatantData.IncomingDamageTypeDataObjects = new()
        {
            ["Taken"] = new("Taken", -1, Color.Red),
            ["Received"] = new("Received", 1, Color.Green),
            ["All In"] = new("All", 0, Color.Transparent)
        };
        CombatantData.SwingTypeToDamageTypeDataLinksOutgoing = new() { [1] = new() { "Damage" } };
        CombatantData.SwingTypeToDamageTypeDataLinksIncoming = new() { [1] = new() { "Taken" } };
    }
}
