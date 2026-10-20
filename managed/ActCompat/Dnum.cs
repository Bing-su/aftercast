namespace Advanced_Combat_Tracker;

// Keep the tested contract slice; unsupported semantics must fail visibly.
public class Dnum(long number, string? damageString)
{
    public Dnum(long number) : this(number, "") { }
    public long Number => number;
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
    public override string ToString() => throw new NotSupportedException("ACT Dnum formatting is not implemented yet");
    public override bool Equals(object? obj) => throw new NotSupportedException("ACT Dnum equality is not implemented yet");
    public override int GetHashCode() => throw new NotSupportedException("ACT Dnum hashing is not implemented yet");
}
