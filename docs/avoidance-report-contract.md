# Remaining ACT avoidance report contracts

Pinned ACT: 3.8.5.288, SHA-256
`934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`.
This continues [outcome grouping](avoidance-outcome-contract.md); it does not
change the production report or construct the native report window.

## Category construction

ShowAvoidanceReport IL 0040..00B9 collects distinct category names by traversing
DamageSwingTypes and indexing SwingTypeToDamageTypeDataLinksOutgoing. Each name
gets a fresh DamageTypeData(false, name, null). Missing registered swing mappings
throw even if no action ultimately uses that swing. Do not silently omit them.

Then 00BF..0214 traverses the selected combatant's AllInc **attack entries** and
skips entries whose Type equals the localized All term. It traverses individual
skill lists, not just the combined incoming All list. Each registered-damage,
non-Dnum.Death action is added to every outgoing category link twice: once under
All and once under its AttackType. Duplicate links are not deduplicated here;
duplicate input skill entries remain duplicate. The category-name dictionary is
deduplicated, but the routed evidence is not. An ability whose Type is All can
therefore be skipped by this loop. Preserve actual graph/list semantics.

## Estimate and sample selection

For every occurrence of the outcome representative:

1. Resolve the attacker from the failed action's parent encounter.
2. If that failed swing's outgoing links contain DamageTypeDataNonSkillDamage,
   use attacker's Items[nonSkillCategory].Items[localized All]. Otherwise use
   attacker's AllOut[failedAction.AttackType]. These are different pools: the
   melee pool includes multiple ability names. Missing attacker/category/skill
   is an error, not an instruction to guess another pool.
3. Create a fresh AttackType with the chosen pool's Type and Parent. Copy all
   sample rows whose Victim equals failedAction.Victim (ordinal case-sensitive).
   **The damage-membership check inside this loop reads the failed action's
   SwingType, not the candidate sample's SwingType.** A matching victim's healing
   or other swing can enter if it is already present in the chosen skill pool.
4. Use this fresh target-specific Average unless it is NaN; then use the
   original chosen pool's Average unless NaN; otherwise use zero. No positive,
   infinity, or other validity check accompanies IsNaN. Source is target-specific
   or overall; retain it for sample evidence.
5. Sum estimates as doubles per outcome and group. Divide by the parent encounter
   Duration.TotalSeconds without a zero guard. Keep nonfinite results explicit.

IL actor 04D2..0606 and encounter 03B0..04DF implement the same estimator.
The target-specific loop's failed-action operand is guarded by the new IL probe.
Evidence activation groups counts by outcome label, attacker, skill/melee label,
selected victim/all label and estimate. It is separate from activating a source
log row. Retain both kinds of evidence rather than conflating them.

## Denominators, cached averages and zero cases

Both report methods temporarily set blockIsHit=false. The newly built category's
Hits therefore counts positive amounts only; its Swings excludes true Dnum.Death
(unless an attack is the special Killing type, which this category normally is
not). Zero and all eligible negative outcomes are non-hits. A custom -10 not
equal to Death remains an attempt. Each outcome displays float32
`(float32(occurrences) / float32(Swings - Hits)) * 100f` and
`(float32(occurrences) / float32(Swings)) * 100f`.
The category summary percentage is `(float32 sum(outcome occurrence counts) / float32(Swings)) * 100f`;
label collisions can make that count smaller than the number of non-hits.
Zero denominators follow CLR IEEE NaN/infinity behavior. Do not replace them
with a claimed zero rate/proportion.

AttackType.Average is original Damage/Hits when Hits>0, otherwise NaN. Damage
adds positive amounts through Dnum arithmetic, including signed overflow and
implicit-Dnum conversion; zero/negative samples do not add damage. **Existing
outgoing AttackType Hits caches are keyed by list count**, not by blockIsHit.
After a prior true-mode Hits read, forcing false with unchanged rows can leave
that original pool's Average cached under true-mode hit counts. The fresh target
pool uses false; fallback may read the earlier cached denominator. The oracle
observes 50 from cached [100,0] versus 100 from a fresh false-mode pool.
Do not recompute the outgoing fallback from raw actions and claim exact parity.
Read actual original-equivalent graph statistics under the owner boundary.
Restore temporary settings in finally in aftercast, including errors; the
original's visible restoration is on its normal end path.

## Encounter ally-name/alias selection

ShowEncounterAvoidance collects registered-damage non-death actions from each
combatant's individual incoming skill entries into one category (no per-outgoing
category routing in this path). It builds unique outcomes as already documented.
It gets the ordered `encounter.GetAllies(true)` list and appends a CombatantData
named literal **YOU**. Use that result, not a new inferred alliance algorithm.

For every outcome label, it scans allies and activates the first matching name
group (String.Contains, ordinal case-sensitive), then breaks. Names are deduped
by List.Contains. For each activated group, the report subsequently rescans
**all original unique outcome labels** and includes every label containing that
name. Thus an outcome mentioning Ann and Bob can appear in both active groups
if another outcome activates Bob. Ordinary Miss does not match an ally name;
`ann` does not match `Ann`. YOU is an uppercase literal, not localized. Empty
names match every string under String.Contains; do not silently sanitize and
claim unchanged reference behavior. Aliases/overlapping names need explicit UI
scope labeling. No combatant-style percentage denominators are displayed here;
this path displays counts, estimated damage and estimated damage/time.

## Executable evidence

- [estimates.json](../managed/tests/fixtures/avoidance/estimates.json) executes
  actual original AttackType Damage/Hits/Swings/Average. The victim-selection
  loop is a translation of the guarded IL; original UI estimator is not invoked.
  It checks victim-specific averages, NaN fallback, inclusion of another swing,
  fresh false-mode denominators, and preserved same-count cache behavior.
- [policy.json](../managed/tests/fixtures/avoidance/policy.json) tests transformed
  category/alias loops after the IL guard verifies critical original branches.
  These policy outputs are **IL-derived contracts**, not native UI execution.
- Existing outcomes fixture still directly executes original outcome methods.

```sh
DOTNET_ROOT=/usr/lib/dotnet dotnet run --project managed/tests/OriginalReference/OriginalReference.csproj -c Release -- binaries/act/'Advanced Combat Tracker.exe' crates/aftercast-winforms/managed avoidance-estimates managed/tests/fixtures/avoidance/estimates.json
DOTNET_ROOT=/usr/lib/dotnet dotnet run --project managed/tests/OriginalReference/OriginalReference.csproj -c Release -- binaries/act/'Advanced Combat Tracker.exe' crates/aftercast-winforms/managed avoidance-policy managed/tests/fixtures/avoidance/policy.json
```

Local unshipped IL evidence: `/tmp/aftercast-avoidance-il.txt` and
`/tmp/avoid-attack.txt`. ShowAvoidanceReport 03FC..04BD pins block/hit ratios,
04F4..0555 sample pool branch, 0594..0606 victim average/fallback;
ShowEncounterAvoidance 0251..030D activates first matching groups and
0357..038F rescans all labels for each active group.

## Minimum additional owned data boundary

Prefer a bounded managed report capture instead of sending the full mutable
attacker graph to Rust. Capture under CombatGate and owner dispatch:

- session, selected encounter/combatant identity, registration/report revision,
  ordered categories and aliases, duration and partial/drop indicators;
- exact category action projection, including duplicates and source identities;
- outcome representative/value/label, occurrences, fresh category Swings/Hits and
  precomputed ratios with finite/NaN/infinity status;
- each failed-action estimate, selected original sample pool key, target/fallback
  origin, actual Average read including cache state, and bounded sample evidence;
- all contributing source-log pages under the same report revision.

Existing action snapshots provide raw values/membership/death flags and logs but
not outgoing links, individual incoming list structure, ordered aliases or actual
attacker skill/category caches. Combining separate live reads would violate
report consistency. Fail explicitly on missing mappings/sample objects and work
limits, restore global settings even on failure, and expire report IDs on a new
capture or session replacement. UI can then page immutable report rows/samples
using this narrow boundary. No further product decision is needed for these
contracts; native layout and custom localization remain separate verification.
Independent review is still quota-blocked and has not been retried.
