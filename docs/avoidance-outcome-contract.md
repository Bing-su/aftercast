# ACT avoidance outcome grouping: first verified slice

This is not a complete avoidance report. It pins one category's outcome grouping
against ACT 3.8.5.288, SHA-256
`934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`.
The native report window was not constructed or displayed.

| Step            | Reference behavior                                                                                                                                                                                                                                                     |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Input           | The caller has already constructed a damage category from the selected combatant's incoming actions and the outgoing swing-type-to-category registration. Its order and duplicate rows matter.                                                                         |
| Eligibility     | In the original combatant report, include registered damage swings and exclude actions equal to `Dnum.Death`. A -10 with a different Dnum label can remain eligible.                                                                                                   |
| Unique outcomes | `GetUniqueAvoidances` considers all amounts `< 1`, including zero and custom negative values. It uses a culture-sensitive `SortedList` keyed by `Dnum.DamageString`; the first Dnum for an equivalent key becomes that group's representative.                         |
| Occurrences     | `GetDnumOccurances` rescans the category and includes actions whose `Dnum` equals the representative. Equality considers both numeric value and `DamageString`; a later different amount with the same label is not counted in the first representative's occurrences. |
| Source          | Evidence indexes returned by the managed slice refer only to its input category list, not to an encounter-wide action page or durable log IDs.                                                                                                                         |

`AvoidanceOutcomes.GroupCategory` implements only these rules, retains exact
Dnum equality and culture-sensitive ordering, and fails explicitly above 16,384
input actions or 4,000,000 comparisons. Its limits are aftercast safety limits,
not ACT behavior. Run on the owner thread: plugin-defined values can have
custom behavior. Category reconstruction, snapshot ownership, denominators,
estimated damage, encounter scope, Bridge/Rust, and GUI are not yet provided.

[Fixture](../managed/tests/fixtures/avoidance/outcomes.json) includes same-label
different-number collisions, duplicate equal outcomes, zero, positive values,
a custom-labeled -10, an actual death marker, an unregistered swing, and
canonically equivalent Unicode labels that compare equal in the selected culture
but are not ordinally equal in Dnum.Equals. The oracle matches evidence by
object reference: original MasterSwing.Equals compares rendered text, which can
otherwise falsely identify a different action as the same evidence. Both the
managed implementation and ACT's original `GetUniqueAvoidances` and
`GetDnumOccurances` execute this fixture. The original probe creates no native
form and does not perform OS clipboard operations.

```sh
DOTNET_ROOT=/usr/lib/dotnet dotnet run --project managed/tests/OriginalReference/OriginalReference.csproj -c Release -- binaries/act/'Advanced Combat Tracker.exe' crates/aftercast-winforms/managed avoidance-outcomes managed/tests/fixtures/avoidance/outcomes.json
```

Remaining exact reference questions: how category reconstruction handles linked
categories, duplicates, missing mappings and `All`; how `AttackType.Average`
selects successful samples (including zero, special and overflow), the melee
`DamageTypeDataNonSkillDamage` branch and victim-specific versus all-victim
fallback; how hit/block settings affect the two displayed denominators; and
how encounter ally-name/`YOU` filtering behaves with overlapping names and
custom labels. These need original IL and executable comparisons before a UI
can honestly claim ACT parity. The existing incoming snapshot does not expose
outgoing swing-to-category mapping or the full attacker sample graph.

Independent review remains pending because the review worker exhausted its
usage quota. Author tests are not a substitute for that review.
