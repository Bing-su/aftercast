# Raw import file/time boundaries

## Verified ACT branch

The pinned ACT 3.8.5.288 artifact is SHA-256
`934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`.
`OriginalReference import-policy` reads its PE metadata/IL without constructing
FormActMain or executing its native import thread. The opcode/target checks are
specific to that pinned artifact; this is reproducible source-contract inspection,
not a native scheduling or UI parity claim.

| Original location                | Verified rule                                                                                                                                           |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ImportFile IL_0a11–IL_0a56       | Dispatch when `time >= begin` and either `time <= end` or both `extendToCombatEnd` and `InCombat`, observing InCombat **before** ParseRawLogLine        |
| ImportFile IL_0a58–IL_0a63       | If rejected and `time > end`, terminate at this first rejected later row; do not scan for subsequent backwards timestamps                               |
| ImportFile IL_01ec–IL_0233       | UI file beginning uses DateTime.MinValue. File end uses DateTime.MaxValue while idle; the original has a separate already-in-combat LastKnownTime clamp |
| IO_ImportLog.InitializeComponent | Constructor defaults select file beginning, file end and checked continuation; saved settings may subsequently override defaults                        |
| ParseRawLogLine IL_0014–IL_001e  | Captures pre-parse combat state and calls CheckIdleEndCombat inside parsing, after the import admission predicate                                       |

Aftercast already requires ending live combat before importing, so its file-end
sentinel for an admitted idle import is DateTime.MaxValue. This unit does not add
import-during-live-combat support or claim the original active-live clamp was run.
No timezone conversion is inferred from civil bounds.

Continuation is a per-record predicate, not a promise to keep only one encounter.
An admitted post-end row can end the old encounter through idle detection and
start another through parser callbacks. A later row that goes back inside the
window can also be admitted after combat ended, until the first rejected later
row terminates the source. The implementation preserves these consequences rather
than silently substituting a sticky cutoff or encounter-ID filter.

## Managed checkpoint

`ImportRange` retains its existing inclusive Start/End validation and adds optional
`ExtendToCombatEnd=false`. Existing strict range/history capture callers therefore
retain their exact bounds. DateTime.MinValue/MaxValue express effective file-side
bounds without another reader or timestamp-seek heuristic. The normal full-file
path is unchanged.

RootImport checks continuation before TryDispatch, whose normal idle detection and
parser callbacks remain unchanged. Companion readers retire only a **rejected**
past-end head; an admitted extended head must not prematurely close its reader.
Existing main-clock/EOF gates, tie policy and bounded read-ahead remain unchanged.
Malformed data actually reached during extension fails visibly and retains accepted
partial history; an unconsumed strict-range suffix retains the earlier contract.
Cancel and EOF use the existing action drain and live-state restoration.

Synthetic ImportBoundaryChecks cover strict versus extended boundaries, equal
endpoints/duplicate timestamps, no active encounter at the end, EOF/empty input,
clock regression, a new encounter started by an admitted later row, effective
file-side sentinels, malformed extended suffix/partial results, explicit cancel/
reuse and a companion continuing beyond the nominal end. The original IL/default
probe is part of `mise run test:original`; managed regressions are part of the
ordinary contracts run.

## Bridge/Rust admission checkpoint

Rust `ImportRange::new` keeps strict `false`. `with_combat_end_extension(true)`
opts into continuation; `from_bounds(None, None, extend)` resolves file-side
choices to MinValue/MaxValue. One-sided `None` is also supported. Each range is a
copied value owned by the admitted request; changes to a caller's later options
do not change that import or the next request's default.

| Transport                            | Admission contract                                                                                                                                |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| ImportOperation range Begin (4)      | Bytes 24/32 retain inclusive ticks; byte 60 bit 0 selects continuation. Unknown bits fail. Full-file Begin (1) requires zero boundary flags.      |
| ImportOperation Advance/Cancel (2/3) | All request option fields, including boundary flags, must be zero; rejection precedes dispatch/cancellation.                                      |
| Companion import Begin (1)           | Optional `extendToCombatEnd` requires paired time bounds and snapshots into the same managed ImportRange.                                         |
| Companion continuation/live/catalog  | Reject any non-null continuation option, including false, before changing owned input state.                                                      |
| One-shot history capture             | Captured saved bounds remain strict, as ACT ThreadHistoryImport passes false. Raw-import extension is not accepted in the capture replay request. |

Actual isolated-worker tests in `import_boundaries.rs` cover copied admission
policy, duplicate/equal endpoints, idle-ending rows that start another encounter,
backwards timestamps, no active combat, EOF/empty input, effective file-side
bounds, companion extension, both transports' cancellation/decoding failure with
partial graphs, reuse and strict one-shot capture replay after an extended raw
import. ABI tests reject changes on continuation and captured replay without
consuming the owned operation. These are synthetic file/parser checks, not native
ACT thread execution.

GUI controls remain the next bounded unit. Native ACT import
threads, Windows/native input and timing remain unverified. This checkpoint does
not implement clipboard import, manual character/viewpoint selection or a separate
progress window.
