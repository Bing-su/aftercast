namespace Bxivact.LegacyReference

open Advanced_Combat_Tracker

/// These member references are emitted against original ACT metadata, not our replacement.
[<AbstractClass; Sealed>]
type ContractProbe private () =
    static member Run() =
        let check condition = if not condition then failwith "Original-reference Dnum contract probe failed"
        let damage = Dnum(123L)
        let labelled = Dnum(456L, "explicit")
        check (damage.Number = 123L)
        check (labelled.DamageString = "explicit")
        labelled.DamageString <- "changed"
        check (labelled.DamageString = "changed")
        check (isNull labelled.DamageString2)
        labelled.DamageString2 <- "suffix"
        check (labelled.DamageString2 = "suffix")
        let number: int64 = Dnum.op_Implicit(damage)
        let unknown: Dnum = Dnum.op_Implicit(-11L)
        check (number = 123L && unknown.Number = -9L)
        let maximum: Dnum = Dnum.op_Implicit(System.Int64.MaxValue)
        let minimum: Dnum = Dnum.op_Implicit(System.Int64.MinValue)
        check (maximum.Number = System.Int64.MaxValue && minimum.Number = -9L)
        check (Dnum(System.Int64.MinValue).Number = System.Int64.MinValue)
        check (Dnum.NoDamage.Number = 0L)
        check (Dnum.Miss.Number = -1L)
        check (Dnum.Unknown.Number = -9L)
        check (Dnum.Death.Number = -10L)
        check (Dnum.ThreatPosition.Number = -9L)
        number
