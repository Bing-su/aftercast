namespace Advanced_Combat_Tracker;

// Preserve ACT's value and display semantics, including its throwing null/type comparisons.
public class Dnum : IComparable
{
    private readonly long number;
    private string? damageString;
    public Dnum(long number) : this(number, "") { }
    public Dnum(long number, string? damageString)
    {
        this.number = number;
        this.damageString = damageString;
    }
    public long Number => number;
    internal string? RawDamageString => damageString;
    public string DamageString
    {
        get => string.IsNullOrEmpty(damageString) ? ToString() : damageString;
        set => damageString = value;
    }
    public string? DamageString2 { get; set; }
    public static implicit operator long(Dnum value) => value.Number;
    // Original implicit conversion clamps below -10; direct construction does not.
    public static implicit operator Dnum(long value) => new(value < -10 ? -9 : value);
    public static Dnum NoDamage => (Dnum)0L;
    public static Dnum Miss => (Dnum)(-1L);
    public static Dnum Unknown => (Dnum)(-9L);
    public static Dnum Death => (Dnum)(-10L);
    public static Dnum ThreatPosition => (Dnum)(-11L);
    public static Dnum operator +(Dnum left, Dnum right)
    {
        if (left.Number >= 0 && right.Number >= 0) return new(unchecked(left.Number + right.Number));
        if (left.Number < 0 && right.Number >= 0) return new(right.Number);
        if (right.Number < 0 && left.Number >= 0) return new(left.Number);
        return new(0);
    }
    public static bool operator ==(Dnum left, Dnum right) => left.Equals(right);
    public static bool operator !=(Dnum left, Dnum right) => !left.Equals(right);

    private string NumericText() => Number.ToString(ActGlobals.mainTableShowCommas ? "#,0" : "0");

    // These are the pinned ACT AddPrebuild defaults, rather than guessed translations.
    // shortcut: external ACT localization overrides require implementing ActGlobals.Trans.
    private static string? ReservedText(long value) => value switch
    {
        0 => "No Damage",
        -1 => "Miss",
        -2 => "Resist",
        -3 => "Parry",
        -4 => "Riposte",
        -5 => "Block",
        -10 => "Death",
        _ => null
    };

    public override string ToString() => Number > 0 && string.IsNullOrEmpty(damageString)
        ? NumericText()
        : ReservedText(Number) ?? string.Concat(damageString, DamageString2);

    public string? ToString(bool compact)
    {
        if (Number > 0) return compact ? Number.ToString() : NumericText();
        if (Number is -1 or -10) return ReservedText(Number);
        if (Number == 0) return compact ? "0" : ReservedText(Number);
        if (!compact) return damageString;
        return damageString!.Length < 3 ? damageString : damageString[..3];
    }

    public int CompareTo(object? obj)
    {
        var other = (Dnum)obj!;
        return Number == -9 && other.Number == -9
            ? damageString!.CompareTo(other.damageString)
            : Number.CompareTo(other.Number);
    }

    public override bool Equals(object? obj)
    {
        var other = (Dnum)obj!;
        return other.Number == Number && DamageString.Equals(other.DamageString);
    }

    public override int GetHashCode() => ToString(false)!.GetHashCode();
}
