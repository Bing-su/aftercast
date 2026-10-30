# Live follower encoding

Import GUI is connected and tested separately in [import encodings](import-encoding.md).
Its per-request choices do not configure live capture. Forward capture snapshots
the plugin LogEncoding at OpenLog; initial zone search remains a separate policy.

## Pinned ACT evidence

The inspected reference is ACT 3.8.5.288, SHA-256
`934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`.
The local decompiled FormActMain entry points establish different policies:

| Path                  | Observed reference behavior                                                                                                                      | Consequence                                                                                                                                                                                                                           |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `OpenLog` / `ReadLog` | OpenLog seeks the stream to EOF before starting ReadLog. ReadLog constructs one StreamReader using LogEncoding and later reads appended content. | Capture the configured encoding for that reader; do not replay history or treat later property reassignment as a new decoder. BOM behavior must be checked at the actual tail position, rather than assuming import's byte-zero read. |
| `ReadLog` framing     | Split decoded content on CR and LF; keep an incomplete suffix unless the read ends in CR or LF.                                                  | Encoded code-unit boundaries and CR-only completion are part of the live contract. Root handling of empty input must remain distinct from low-level framing tests.                                                                    |
| `FindZoneName`        | Construct a default StreamReader without passing LogEncoding, then seek/read windows of the underlying file.                                     | Do not blindly make initial-zone decoding inherit plugin LogEncoding. It is a separate UTF-8/default-reader path; verify BOM/seek behavior with focused reference contracts before changing it.                                       |

These are inspected code/constructor contracts. The native ACT reader thread and
zone-search UI have not been executed on Linux. The existing LogEncodingProbe
checks property and StreamReader contracts; it does not execute FindZoneName.

## Current aftercast and bounded implementation order

`OpenLog` validates/clones the same nine supported encodings and fallback policies
as import before retiring an existing reader. Reassigning LogEncoding affects the
next OpenLog, not an active reader. The forward decoder frames encoded CR/LF units,
retains incomplete bytes at temporary EOF, and resets to the captured codec at
truncation/replacement. Complete old records drain before rotation; incomplete old
records are reported and discarded, including fragments discarded by truncation. A source record is bounded to 8 MiB; strict
malformed records are reported and skipped, allowing later records to continue.

Selected preambles precede other Unicode BOMs at byte zero and at the first
read from a nonzero initial tail **when the captured history ends at a known
record boundary or consists solely of the selected codec's preamble**. A recognized tail BOM starts a new code-unit alignment and
clears any historical CR carry; split BOMs are retained across temporary EOF.
Only this first reader prefix is inspected: later BOMs are content. With detection
disabled, only the selected encoding's own preamble is consumed. If the captured
history ends in a partial record or code unit, its continuation is discarded until
an encoded delimiter and is never treated as a fresh BOM. This safety boundary
still differs from ACT's newly constructed StreamReader at a partial tail. Strict
fallback survives BOM override, unlike ACT StreamReader's replacement fallback
reset. No encoding is guessed. Native ACT reader-thread timing remains unverified.

`ReadInitialZone` uses the default StreamReader text policy: UTF-8 replacement
without a BOM, or UTF-8/UTF-16 LE/BE/UTF-32 LE/BE selected by a byte-zero BOM.
It never inherits plugin LogEncoding or import options. Reverse scans frame CR,
LF and CRLF at aligned code units, skip empty/encounter-annotation records, and
choose the newest complete matching record before the captured offset. Incomplete
final records/code units are excluded. Each retained record is bounded to 8 MiB
source bytes; oversize records are reported and skipped. The same owned handle is
used across rename/replacement, and snapshot release/cancellation/truncation remain
explicit outcomes. A new reader starts with a fresh header policy.

### Reference differences and execution limits

| Behavior                 | ACT evidence                                                                                                                                                                         | aftercast policy                                                                                                                                                                                                       |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Encoding                 | Inspected FindZoneName constructs default StreamReader without LogEncoding. Constructor/BOM/CR/replacement contracts pass against the pinned binary's reference probe.               | Same default replacement codec family, independent of forward selection.                                                                                                                                               |
| Incomplete EOF record    | Default ReadLine returns the last unterminated fragment. The inspected search can consume it when its stream-position loop admits that read.                                         | Preserve the existing completed-record snapshot contract; exclude it so an unfinished zone cannot change current state.                                                                                                |
| Large-file BOM           | Inspected search initially seeks backward by 250000 bytes; the reader may begin beyond the original header. Its underlying-stream-position loop also depends on buffered read-ahead. | Read the actual byte-zero header once, then scan aligned reverse blocks. Do not reproduce window-dependent BOM/read-ahead artifacts or claim exact window parity.                                                      |
| BOM at nonzero live tail | The selected StreamReader at a nonzero initial position detects a newly appended Unicode BOM; a focused tail probe verifies this.                                                    | Detect a first-read BOM at a known historical record boundary, preserving partial-history discard and strict fallback. Later BOMs remain content; partial historical tails intentionally retain the safety difference. |

An attempted invocation of the pinned ACT private FindZoneName failed with
TypeLoadException because the existing Linux headless Forms shim does not define
System.Windows.Forms.Application. Its native search/UI was **not executed**.
The failing exploratory probe was removed rather than expanding the shim; tests
exercise the inspected constructor/framing contracts and actual aftercast search.
No Windows native parity is claimed.

| Unit                                | Required completion evidence                                                                                                                                                                                                                                                                                                                           |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| F1 Encoding policy and admission    | Focused synthetic/reference tests for StreamReader at EOF, selected preamble/BOM and default zone-reader seek behavior. Capture/validate a supported encoding before retiring the previous follower. Keep forward plugin selection distinct from initial-zone policy and document deliberate strictness differences.                                   |
| F2 Forward record decoder           | Reuse the supported import encoding set and validated fallbacks where applicable, with per-reader/per-file state. Test split characters/code units, CR/LF/CRLF, historical partial tails, empty/BOM-only starts, strict error recovery, bounded oversized records, pause/resume and cancellation. No guessed encoding or external converter.           |
| F3 File generations and zone search | Keep identity and captured-offset ownership. Drain complete old tails before replacement; reset pending bytes/decoder/BOM state on truncation or rotation and report discarded fragments. Test positional reverse scans, code-unit alignment for the chosen zone policy, invalid records, search cancellation, replacement races and callback cleanup. |

Use only temporary synthetic logs. Preserve the current reader lifecycle and
retired-reader bounds, owner dispatch and import suspension. The existing tests in
LogFollowerChecks, LogHostChecks and HostLifetimeChecks remain mandatory regressions.
Windows native input/capture and arbitrary plugin effects remain separate QA.

## Implemented verification

Synthetic temporary files cover all nine forward encodings, byte-at-a-time BOM and
character delivery, CR/CRLF, misaligned historical UTF-16/32 tails, malformed strict
and replacement recovery, truncation with incomplete units, rotation from BOM-wide
to captured UTF-8, pause/resume, callback failure and joined shutdown. Host tests
check original decoded strings, encoding snapshot ownership, rejected admission
preserving the old reader, and independent initial zone decoding. The reference
LogEncodingProbe additionally checks StreamReader at a nonzero tail and the default
zone reader after seek; it does not invoke ACT native reader threads or FindZoneName.

The initial-zone regression matrix includes five Unicode BOM encodings, all three
line separators, split reverse-block boundaries, malformed Unicode replacement,
BOM-only/empty/nonmatching histories, incomplete trailing code units, oversized
records, cancellation, captured-end append exclusion, owned-handle rotation,
truncation failure, disposed/released snapshot rejection and fresh-reader reset.
