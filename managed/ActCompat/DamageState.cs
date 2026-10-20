namespace Advanced_Combat_Tracker;

// Reconstruct only the constructor-owned graph; action insertion and aggregate calculations are separate contracts.
public partial class DamageTypeData
{
    private readonly string? tag;

    public DamageTypeData(bool outgoing, string? tag, CombatantData? parent)
    {
        Outgoing = outgoing;
        this.tag = tag;
        Parent = parent;
    }

    public CombatantData? Parent { get; }
    public bool Outgoing { get; set; }
    // The original getter dispatches through ToString, including overrides on derived category types.
    public string? Type => ToString();
    public SortedList<string, AttackType> Items { get; } = new();
    public Dictionary<string, object> Tags { get; set; } = new();
    public override string? ToString() => tag;
}

public partial class AttackType
{
    public AttackType(string? attackType, DamageTypeData? parent)
    {
        Type = attackType;
        Parent = parent;
    }

    public DamageTypeData? Parent { get; }
    public string? Type { get; }
    public List<MasterSwing> Items { get; set; } = new();
    public Dictionary<string, object> Tags { get; set; } = new();
    public override string? ToString() => Type;
}
