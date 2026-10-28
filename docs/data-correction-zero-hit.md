# Zero-damage hit counting

ACT 3.8.5.288 `Options_DataCorrectionMisc.InitializeComponent` enables
`cbBlockisHit` by default. Its checked-change handler assigns
`ActGlobals.blockIsHit`; the pinned `ChildActionProbe` exercises original ACT
objects with zero, miss, block and death values. With the option enabled, zero
counts toward hits and critical hits; when disabled it does not. Positive damage
and original action values are unchanged.

Aftercast persists this option with the ACT-enabled default and applies it before
plugin constructors on the next host session. Synthetic worker tests observe the
original input and the global in `BeforeCombatAction`, then compare hit counts
after import, reparse, and private-history restoration. Restoring history builds
fresh aggregates under the current session setting, not a stored historical
setting. Plugins can still change the public ACT global in their session. ACT's
incremental hit caches retain already calculated values when that global changes;
this implementation preserves that behavior rather than claiming immediate
recalculation. Native Windows timing and UI interaction remain unverified.
