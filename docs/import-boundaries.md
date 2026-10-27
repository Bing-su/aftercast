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
| ImportFile IL_0a11–IL_0a56       | Dispatch when `time >= begin && (time <= end                                                                                                            |     | extendToCombatEnd && InCombat)`, observing InCombat **before** ParseRawLogLine |
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

Bridge/Rust file-side selection and continuation options, actual isolated-worker
regressions and GUI controls remain the next bounded unit. Native ACT import
threads, Windows/native input and timing remain unverified. This checkpoint does
not implement clipboard import, manual character/viewpoint selection or a separate
progress window.
