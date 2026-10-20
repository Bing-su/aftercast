namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    internal string[] IncomingCategories { get; private set; } = [];
    internal string[] OutgoingCategories { get; private set; } = [];
    internal Dictionary<Type, string[]> VisibleColumns { get; private set; } = new();

    // Reconcile actual registration data for a headless host instead of constructing ACT's native selector UI.
    public void ValidateLists()
    {
        foreach (var names in CombatantData.SwingTypeToDamageTypeDataLinksIncoming.Values)
            foreach (var name in names)
                if (!CombatantData.IncomingDamageTypeDataObjects.ContainsKey(name))
                    throw new InvalidOperationException($"Unknown incoming damage category: {name}");
        foreach (var names in CombatantData.SwingTypeToDamageTypeDataLinksOutgoing.Values)
            foreach (var name in names)
                if (!CombatantData.OutgoingDamageTypeDataObjects.ContainsKey(name))
                    throw new InvalidOperationException($"Unknown outgoing damage category: {name}");
        IncomingCategories = CombatantData.IncomingDamageTypeDataObjects.Keys.ToArray();
        OutgoingCategories = CombatantData.OutgoingDamageTypeDataObjects.Keys.ToArray();
    }

    public void ValidateTableSetup()
    {
        // Publish a complete replacement only after every definition has been read successfully.
        VisibleColumns = new()
        {
            [typeof(CombatantData)] = CombatantData.ColumnDefs.Where(item => item.Value.DefaultVisible).Select(item => item.Key).ToArray(),
            [typeof(EncounterData)] = EncounterData.ColumnDefs.Where(item => item.Value.DefaultVisible).Select(item => item.Key).ToArray(),
            [typeof(MasterSwing)] = MasterSwing.ColumnDefs.Where(item => item.Value.DefaultVisible).Select(item => item.Key).ToArray(),
            [typeof(DamageTypeData)] = DamageTypeData.ColumnDefs.Where(item => item.Value.DefaultVisible).Select(item => item.Key).ToArray(),
            [typeof(AttackType)] = AttackType.ColumnDefs.Where(item => item.Value.DefaultVisible).Select(item => item.Key).ToArray()
        };
    }
}
