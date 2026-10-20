namespace Advanced_Combat_Tracker

open System

/// Initial binary-contract slice; formatting and aggregation operators remain unsupported.
[<AllowNullLiteral>]
type Dnum(number: int64, damageString: string) =
    let mutable damageString = damageString
    let mutable damageString2: string = null

    new(number: int64) = Dnum(number, String.Empty)

    member _.Number = number
    member this.DamageString
        with get () = if String.IsNullOrEmpty(damageString) then this.ToString() else damageString
        and set value = damageString <- value
    member _.DamageString2
        with get () = damageString2
        and set value = damageString2 <- value

    static member op_Implicit(value: Dnum) : int64 = value.Number
    // The original conversion clamps values below -10; direct constructors do not.
    static member op_Implicit(value: int64) : Dnum = Dnum(if value < -10L then -9L else value)
    static member NoDamage: Dnum = Dnum.op_Implicit(0L)
    static member Miss: Dnum = Dnum.op_Implicit(-1L)
    static member Unknown: Dnum = Dnum.op_Implicit(-9L)
    static member Death: Dnum = Dnum.op_Implicit(-10L)
    static member ThreatPosition: Dnum = Dnum.op_Implicit(-11L)

    // Fail visibly until ACT's localization/format/equality rules are implemented and compared.
    override _.ToString() = raise (NotSupportedException("ACT Dnum formatting is not implemented yet"))
    override _.Equals(_) = raise (NotSupportedException("ACT Dnum equality is not implemented yet"))
    override _.GetHashCode() = raise (NotSupportedException("ACT Dnum hashing is not implemented yet"))
