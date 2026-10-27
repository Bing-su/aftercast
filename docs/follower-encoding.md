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

Selected preambles precede other Unicode BOMs at byte zero. This keeps aftercast's
existing generation-start BOM policy: a BOM appended at a nonzero tail is content.
ACT's newly constructed StreamReader can detect a BOM at its initial tail position;
this is a deliberate difference, not verified native parity. Existing partial
historical records are discarded until their first encoded delimiter; they are
never replayed. Strict fallback survives BOM override, unlike ACT StreamReader's
replacement fallback reset. No encoding is guessed.

`ReadInitialZone` still uses bounded positional reverse UTF-8/LF scans. The test
selecting forward Korean code page 949 while finding a UTF-8 historical Korean zone
proves the policies are independent. Wide BOM zone history, CR-only reverse records
and ACT default-reader replacement parity remain follow-up work in F3. Forward
support does not claim these reverse-search cases are complete.

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
