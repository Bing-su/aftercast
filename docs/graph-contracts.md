# ACT graph numeric contracts for implementation

Reference: ACT 3.8.5.288, pinned repository executable. Evidence is method IL,
not native GDI+/WinForms execution. Managed static SVG rendering and opt-in hierarchy integration exist; Bridge/Rust/GUI graph options and native rendering remain unimplemented. These four routes must not share one generic rolling-series model.

| Route                | Input and ordering                                                                                                                                                        | Numeric result                                                                               |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| GenEncounterGraphAdv | Encounter registry order (Sorting unused), filtered by SelectiveListGetSelected only when rbSParseExport is checked; each actor's localized All attack in outgoing damage | Positive damage time series, offset from encounter.StartTime                                 |
| GenCombatantGraph    | Checked clbSoloGraphTypes in checked-list order; localized All attack per chosen category                                                                                 | Positive damage time series; duration from parent encounter, offsets from actor.EncStartTime |
| GenDamageTypeGraph   | AttackType values sorted by registered selected SortComparer, then reversed; localized All supplies denominator                                                           | Selected SQL numeric shares, or Resist damage-type shares; no renormalization                |
| GenAttackTypeGraph   | Original actions copied and sorted with DualComparison(Sorting,Sorting), no subsequent Reverse                                                                            | One raw signed Dnum amount per ordered action; no averaging                                  |

## Line series

Let T=truncate((encounter.EndTime-encounter.StartTime).TotalSeconds), W=average
window, and s=truncate((action.Time-origin).TotalSeconds). Conversion truncates
toward zero: -0.5 becomes 0, -1.1 becomes -1. Only amount>0 is accumulated.

Rolling allocation has T+1 slots. If s>=0, add amount to each index i satisfying
s<=i<s+W and i<T. This is a forward accumulation of the event into subsequent
plotted seconds, equivalent to a trailing window at each point. Slot T is a
sentinel and is not populated by an event at s=T. Nonrolling allocation has
truncate(T/W)+2 slots; add amount to index truncate(s/W)+1. The nonrolling path
has no corresponding explicit negative-offset exclusion. Do not silently clamp
its index or replace native index errors with a zero sample.

Rates use signed integer division of the accumulated Int64 by W. Plotted sample
count is P=T for rolling and truncate(T/W) otherwise; only j<P is plotted. Thus
the +1 bucket offset can leave the first plotted bucket zero and hide populated
end buckets. Allocated arrays and plotted vectors must remain distinct.

Evidence: EncounterAdv IL0160..0180 allocation, IL01ea..028b accumulation,
IL0776..079b displayed range and IL07a7..07c6 integer rate comparison;
Combatant IL00a0..0103 duration/allocation and IL0156..01f2 accumulation,
IL036a..0399 displayed range. EncounterAdv rejects no actors, unset start/end
and T<2 as no-data. Do not assume identical early no-data checks for the solo
route without checking its branches. Solo checks P<2 after choosing rolling/nonrolling (IL0391..03c0).

Options_Graphing.InitializeComponent sets W=10 at IL03d5..03ea, minimum1 and
maximum120 at IL0370..03a3. Rolling is initially checked at IL06e7..06f9.
Simple is the initial screen graph mode; suffix and suffix-decimal switches are on. Saved settings override initial control values. Solo categories default to the registered outgoing and incoming keys, checked when added; saved settings can override them. Global sort defaults require further initialization-path evidence. HTML graph dimensions default640x256, UI128..1600;
low-level generators return a16x16 bitmap when either dimension is below16.

## Category distribution

Find the localized All attack. In the ordinary route, parse numerator using the
selected column's GetSqlData(attack) and usCulture. Include numerator>0, excluding
All; divide by the same selected GetSqlData(All). The native per-row catch drops
SQL/parse failures (IL02c9..0336). Values do not necessarily sum to1. Pie sweep
is (float)ratio*360 from starting angle -90 (IL0ae9..0b2b). NaN/Infinity and zero
or negative denominators are not converted to a normalized fallback by this
numeric path. A host implementation must report these explicitly before static
SVG output instead of emitting invalid geometry or claiming a valid zero graph.

For Sorting=="Resist", use actions in All, create groups by DamageType, and
accumulate only positive amounts into group and total. A DamageType beginning
with localized warded term plus slash is stripped using the native fixed
Substring(7), not the length of the localized prefix (IL0139..0164). An action whose group name equals All increments the shared All entry twice, so its positive amount contributes twice to the denominator. Divide each
non-All group by total (IL017c..026e); zero-valued groups can exist. Culture,
localization and the fixed-seven quirk need dedicated cases.

## Action bars

Sort the copied action list using original comparer semantics. DualComparison invokes the same selected comparer again on an initial tie, then compares TimeSorter and Time. Raw maximum scan
starts at1 (IL0380..03bc), while actions with zero/negative damage remain in the
ordered list. Geometry uses bottom-(float)damage*(float)scale and
(float)damage*(float)scale (IL06aa..06d5). Axis ceiling is rounded separately
(IL03d7..0445): start c=10^digits(max(1,rawMax)); repeatedly set c=integer(c/2) while integer(c/2)>max; if c/1.25>max, set c=Convert.ToInt64(c/1.25), with nearest-even rounding. raw maximum10 yields ceiling12, so amount10 occupies10/12 of the
plot height. Do not normalize every graph to its raw maximum. Preserve ordered
raw values separately from pixel geometry and sentinel/failed Dnum meanings.

## Deterministic acceptance fixtures

[contracts.json](../managed/tests/fixtures/graphs/contracts.json) contains explicit
raw bins, integer rates, plotted samples, non-normalized SQL shares, Resist
groups and signed action-height vectors. Run:

```sh
python3 managed/tests/fixtures/graphs/check_reference.py
```

The independent executable oracle validates the declared arithmetic. It is not
a production implementation or proof of native rendered equivalence. It does
not load plugins, read user logs/clipboard, launch a browser or export files.
Before integration, add checked owner/original-object capture, registry/topology
validation, bounded samples/series/work/output and explicit failure diagnostics
using the incumbent report machinery. Static SVG must use finite coordinates,
escaped labels and no scripts/resources; pixel/font/tooltip equivalence remains
Windows/native QA.

Remaining investigation gates: exact series ordering, extreme-value axis conversion/overflow, cumulative/stacked painting and
layout scaling, annotation options, overflow and nonfinite original behavior.
These gates prevent claiming a complete renderer from the supplied vectors.

Independent IL review confirmed these vectors and corrected allocated-versus-plotted
samples and rawMax-versus-axis scaling. The supplied Python Resist example uses
ASCII; future localized fixtures must implement UTF-16 Substring(7), not Python
code-point slicing. No full check is warranted for reference-only documents and
fixtures; production implementation/integration has not changed.

### Bounded line arithmetic helper

`GraphNumeric.Line` implements the fixture's arithmetic with explicit caller inputs. Its duration limit of 360,000 seconds and 16,384 action limit are aftercast safeguards, not ACT limits; its 1–120 second window follows the observed UI range. Invalid timestamps and out-of-range buckets fail explicitly. The helper does not select source actions, implement ACT's no-data decisions, render graphs, or connect graphs to reports.

ACT initializes the encounter delegate to `GenEncounterGraph` (Simple); a restored setting can subsequently select Advanced. For solo categories, `ValidateLists` adds missing outgoing and incoming registry keys as checked while preserving existing choices and removing obsolete keys. These are dynamic defaults, not a fixed category list.

`GraphNumeric.Share` carries labels and raw, unnormalized finite ratios for a future static renderer. `Distribution` expects callers to exclude the localized All row and to perform SQL parsing and native per-row failure handling. `Resist` groups supplied actions using the current culture and ACT's fixed seven UTF-16-unit prefix removal. Invalid or nonpositive totals and Int64 accumulation overflow are explicit errors; ACT overflow behavior remains unverified. `ActionBars` takes caller-provided selected and TimeSorter comparisons, preserves signed amounts and ordered actions, and returns a separately rounded axis ceiling and relative heights; geometry and float conversion remain renderer responsibilities. These helpers limit inputs to 16,384 entries; axis ceilings exceeding Int64 are rejected. These limits and explicit geometry errors are aftercast safeguards, not ACT limits. Native pixel rendering, callback ownership, sentinel Dnum meanings, and report integration remain unimplemented.
