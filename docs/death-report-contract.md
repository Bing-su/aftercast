# ACT death report data contract

Scope: ACT 3.8.5.288, SHA-256
`934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`.
This resolves the L8b bucket questions. The native report window was not opened.
The original nested `DeathData`/`DeathSecond` classes were executed on Linux with
synthetic actions, the shipped headless Forms assembly, original localization,
and an uninitialized FormActMain used only by CreateDamageString. No form
constructor, clipboard, game input or drawing was invoked. Global fields are
restored after the probe.

## Implementation rules

| Concern                        | Original contract                                                                                                                                                                                                                                                                                                                           |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Source                         | Selected combatant's incoming `All` list; not an encounter-wide outgoing query filtered by victim. Incoming-only registrations must remain visible.                                                                                                                                                                                         |
| Order                          | Copy actions and sort using MasterSwing.CompareTime: **TimeSorter first, DateTime second**. Neither input enumeration order nor timestamp alone is sufficient. Exact comparator ties have no promised stable ordering (List.Sort).                                                                                                          |
| Death selection                | ShowDeathReport compares Damage with Dnum.Death using Dnum equality. Custom `-10` labels can fail equality. Keep repeated death markers, even identical times; initially select the final death in sorted order. Preserve original fractional timestamp in the selector.                                                                    |
| Anchor                         | DeathData truncates the selected time to year/month/day/hour/minute/second (DateTime kind becomes Unspecified). Its TimeOfDeath is this truncated value D.                                                                                                                                                                                  |
| Buckets                        | Exactly ten, indexed i=0..9, covering `[D-i seconds, D+(1-i) seconds)`. Index 0 is the death second; index 9 is the earliest. Total window `[D-9s,D+1s)`.                                                                                                                                                                                   |
| Label                          | DeathSecond stores the **exclusive end** of its interval. Its tooltip timestamp for i=0 is D+1s, although the chart label is `-0`.                                                                                                                                                                                                          |
| Membership                     | Every incoming action inside the interval reaches AddAction. DamageSwingTypes and HealingSwingTypes are checked independently; neither is inferred from sign or English text. Unregistered actions contribute to neither sum nor tooltip.                                                                                                   |
| Accumulator                    | Each registered branch evaluates `long(Dnum(accumulator) + action.Damage)`, including the long-to-Dnum conversion's clamp below -10. This is not signed integer addition, nor simply summing positive numbers under all inputs.                                                                                                             |
| Normal negative/special values | Starting at zero/nonnegative totals, negative amounts (miss/death/custom/etc.) do not reduce the total. They still add a tooltip entry in every matching registered branch. Zero also gets an entry.                                                                                                                                        |
| Overlap                        | A swing registered for both contributes independently to both totals, and appends its tooltip **twice**, damage branch before healing branch. Result is unchecked `damage - healing`.                                                                                                                                                       |
| Overflow                       | Original Dnum positive addition wraps Int64. A negative accumulated result is converted back through Dnum on the next action (values below -10 clamp to -9). Negative plus positive becomes that positive; negative plus negative becomes zero. Preserve this contract or report an explicit overflow difference; do not saturate silently. |
| Tooltip                        | Per matching branch append `attacker - attack - CreateDamageString(long(action.Damage), true, true) + LF`. Numeric accumulation and tooltip formatting are separate. Raw custom labels are not substituted into this formatter. Retain original action evidence alongside rendered strings.                                                 |
| Text                           | DeathData.ToString emits T-0 through T-9; each line shows damage, result, and cumulative **damage** in reverse chronological bucket order. This bracketed running sum is not running net damage.                                                                                                                                            |
| Chart                          | Selection handler traverses buckets 9 down to 0, drawing damage/healing and cumulative **result** chronologically. Text's cumulative damage and chart's cumulative result are distinct. Native color/font/pixel layout is separate QA.                                                                                                      |
| Missing evidence               | Original bucket code cannot recover omitted logs or unrecorded actions. Show snapshot dropped counts/partial status and preserve Missing/Ambiguous source links rather than selecting a guessed row.                                                                                                                                        |

Dnum equality and numeric conversion are already implemented in managed code;
prefer the snapshot's `is_death` flag over reimplementing localization-sensitive
equality in Rust. Snapshot membership captures registration state at capture;
original ShowDeathReport reads current registration while building the report.
The report UI should rebuild explicitly to adopt new registrations.

## Evidence and reproducible fixture

The metadata-only IL dump is local `/tmp/aftercast-death-il/dump.txt`.
Key offsets: ShowDeathReport 00A2..011E reads/sorts incoming actions and finds
markers, 0240..026A selects the last; DeathData constructor 0011..0044 truncates,
0050..00ED builds intervals; DeathSecond.AddAction 0000..0071 handles damage,
0071..00E2 independently handles healing. MasterSwing.CompareTime 0002..002E
compares sorter before time. These statements are IL evidence, distinct from
the executable cases below.

[Fixture](../managed/tests/fixtures/deaths/buckets.json) and
[original oracle](../managed/tests/OriginalReference/DeathBuckets.cs) verify:
whole-second inclusive/exclusive edges, fractional death truncation, all ten
buckets, empty buckets, tooltip exclusive-end labels, sorter order across
out-of-time timestamps, overlapping registrations, negative sentinels,
Int64 wrapping and subsequent recovery. Cases execute the original data classes,
not a translated replacement. They do not exercise ShowDeathReport's native UI.

```sh
DOTNET_ROOT=/usr/lib/dotnet dotnet run --project managed/tests/OriginalReference/OriginalReference.csproj -c Release -- binaries/act/'Advanced Combat Tracker.exe' crates/aftercast-winforms/managed death-buckets managed/tests/fixtures/deaths/buckets.json
```

DateTime values too near MinValue/MaxValue can throw during AddSeconds in the
original; define explicit report errors rather than clamping buckets. Exact
comparator ties, original numeric formatting under additional localization,
and native chart rendering remain outside these fixtures.

## Aftercast implementation and verification

The incoming snapshot carries a separately formatted numeric tooltip alongside
the raw Dnum label and rendered action damage. This prevents a custom -9 label
from being mistaken for ACT's numeric tooltip. The snapshot freezes formatting
under the owner thread's current culture. [Numeric formatting fixtures](../managed/tests/fixtures/deaths/formats.json)
are checked by both original ACT and managed tests in en-US and fr-FR, including
NaN/infinity sentinels, negative outcomes, suffix thresholds and rounding. Other
localization overrides are not covered.

The Rust/UI screen presents a static magnitude bar for damage/healing and a
chronological cumulative-net value; it is not the native ACT line graph.
The Host controller regression exercises selection, source cancellation, stale
identity and session changes with synthetic logs. Native window focus, input,
and pixel rendering remain unverified. Independent review is pending because
the reviewer reached its usage limit.
