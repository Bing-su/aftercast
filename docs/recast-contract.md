# ACT recast analysis contract

ACT 3.8.5.288 `FormSpellRecastCalc.ShowMedian` reads one selected AttackType,
including successful and unsuccessful actions, then uses `MasterSwing.CompareTime`
(TimeSorter, then Time). The first row is checked. Each later default checkbox
compares its integer-truncated time difference to the preceding raw row: greater
than 11 seconds when the default restriction is on, greater than 1 when off.
Recalculation compares consecutive checked rows instead, so deselecting a row can
merge intervals. Exact comparator ties have no original stable-order guarantee.

For every checked pair ACT records an integer-second delay. The proposed timer is
the minimum delay strictly greater than one and less than the sentinel 12345;
if no value qualifies its timer handoff uses zero. The histogram separately
includes delays greater than one, excluding 121 or more only when the restriction
is on. The minimum is not a median and is not subject to the histogram's 121s
cutoff. The result UI explicitly identifies insufficient evidence.

The optional game-specific adjustment scans the selected attack owner's incoming
attacks and takes the last matching ACT Traumatic Swipe name in its five built-in
languages. A qualifying swipe has damage number at least zero and applies only
when the interval's starting action is strictly after the swipe and strictly
before swipe time plus 31 seconds. The swipe does not apply if its attacker has
an ACT death marker at or after the swipe and before the interval start; ACT
prefers that attacker's `Killing` incoming list, otherwise `All`. If no attacker
is found, ACT keeps the modifier. The adjusted integer delay is the float32 delay
divided by 1.5 and truncated; when a matching attack list exists but no swipe
applies, ACT still converts through float32 and divides by 1. If no matching
attack exists, it skips that conversion. This is a named game-specific rule, not
a generic cooldown adjustment.

The managed capture freezes the selected actor/category/direction/attack,
original-order first attacker for timer handoff, sorted rows, optional modifier
and source evidence, and retained encounter logs under the same combat lock.
Missing or ambiguous log identities remain explicit. The owner and category must
still match the selected attack object. A competing capture invalidates older
worker pages, while a decoded Rust snapshot owns its data. Limits are 16,384
attack rows and incoming groups, 16,384 modifier rows, 65,536 retained logs,
4,000,000 modifier/death visits and 64 MiB encoded output. These are aftercast
safeguards, not ACT defaults. Timestamps and intervals outside the supported
integer range fail explicitly rather than being guessed.

The Rust/UI view shows checked actions, intervals and frequency bars, their
source evidence, and a timer draft. A draft uses ACT's attack object name,
first original-row attacker, modifier setting and minimum/zero fallback; it is
not saved until the user acts in the existing timer editor. An unsaved or busy
editor is not overwritten. Options are captured at admission; changing draft
options requires a new calculation. Local cancellation stops waiting, while an
already admitted worker request can still finish. Output is a simplified static
chart rather than ACT's native drawing. ACT's default English `All` and `Killing`
terms are used; external localization overrides are not supported.

The `recast-policy` probe pins the original report IL and runs transformed
selection and arithmetic fixtures. It does not execute the native report form.
Synthetic managed and isolated-worker fixtures cover same-time actions, negative
outcomes, strict modifier timing, attacker death and frozen log identity. Physical
GUI interactions, Windows execution and independent review remain unverified;
the latter is pending under the previously reported quota restriction.
