# ACT special attack report contract

ACT 3.8.5.288 `FormAvoidanceReport.ShowSpecialsReport` reads the selected
combatant's `AllOut["All"]` actions and filters by the registered damage swing
types. It creates three sorted groups: skill plus special, skill with any
special, and special name. The original `GetAttackSpecials` sorts by TimeSorter,
then Time, clears its seen set on the localized None term, and counts a special
only once between None actions. `ANY` counts distinct special occurrences in
those segments, while `ONCE` counts segments with at least one special. Literal
special names `ANY` and `ONCE` collide with those reserved dictionary keys;
the synthetic fixture exercises the original behavior rather than renaming them.

For each skill/special ACT displays its special occurrence count, special swings,
all skill swings, occurrence/None count and special swings/None count, and both
encounter DPS values. The skill-any group uses ANY occurrences and ONCE/None for
the first ratio; special-only rows show swings and encounter DPS. The ratios are
float32 divisions and may be nonfinite. Swings use ACT's existing death-marker
rules; encounter DPS uses positive Dnum damage and encounter duration. The
original concatenates `skill + "(" + special + ")"` and then splits at the last
opening parenthesis, so labels containing parentheses may cause a lookup failure.
Do not silently replace this with a different report contract.

`GetAttackSpecials` is implemented and compared against the original method with
`managed/tests/fixtures/specials/counts.json`, including out-of-order rows,
repeated values, None resets, empty input and reserved-name collisions. The
implementation uses ACT's default English `None`; localization overrides need
ActGlobals.Trans, as elsewhere in aftercast.

The managed report captures all three groups, their evidence and retained logs
under one combat lock, then pages immutable bytes through the session-owned worker
API. A new capture retires old pages; the Rust report owns its decoded copy.
Limits are 16,384 source actions, 4,096 groups, 49,152 evidence rows, 65,536
logs, a conservative sum of per-skill squared row counts of 4,000,000, and a
64 MiB encoded report. These are aftercast safeguards, not ACT defaults. Failed
captures keep the previous snapshot. Missing/ambiguous source logs are explicit.
The existing Rust/UI displays all three groups, their raw ratios and rates, and
paged source evidence. Cancellation stops waiting; an admitted request may
finish. Numeric display uses fixed decimal formatting rather than ACT's locale.
Native layout, Windows execution and actual mouse/keyboard interaction remain
unverified. Independent review remains pending because of the existing quota
restriction; only self-review was performed.
