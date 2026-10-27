# Source-log range reparse

Source reparse runs the original raw file through the initialized parser. It is
separate from checkpoint restoration, which reconstructs saved graph data and
preserves saved action/log identities without parsing the original file.

## Reference and boundaries

The pinned ACT 3.8.5.288 binary has SHA-256
`934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`.
`FormActMain.ThreadHistoryImport` passes each record's StartTime/EndTime to
`ImportFile(false, false, start, end)`. The latter dispatches records when
`dateTime >= dtBegin && dateTime <= dtEnd` and breaks at the first
`dateTime > dtEnd`. This differs from the exclusive end of metadata search.

| Rule                | Current managed implementation                                                                                                                                                                                                                                                                                                       |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Bounds              | `ImportRange` contains inclusive DateTime wall-clock bounds. Equal bounds are valid; reversed bounds fail before admission. No timezone conversion is inferred.                                                                                                                                                                      |
| Same timestamp      | Every matching physical record is dispatched in file order with a distinct global sorter.                                                                                                                                                                                                                                            |
| Undated/blank lines | DateTime.MinValue follows ACT's comparison; for ordinary finite history ranges undated records are skipped. Blank lines count as read without hook dispatch.                                                                                                                                                                         |
| End                 | Default strict ranges stop at the first timestamp strictly after the end, including out-of-order files. Opt-in managed continuation follows the [verified ACT branch](import-boundaries.md); existing callers default to false.                                                                                                      |
| Source context      | Scan the prefix from byte zero; preserve preceding raw zone-regex matches without dispatching out-of-range callbacks. No source-file guessing from folder hints.                                                                                                                                                                     |
| Reader              | Reuse the selected import encoding, ACT preamble/BOM framing, CR/LF/CRLF, final unterminated records, 8 MiB source-record bound and captured file length. Default root decoding is strict UTF-8; plugin inheritance and explicit options are described in [import encodings](import-encoding.md). Companion readers remain separate. |
| Progress            | LinesRead includes skipped and terminating records consumed by the parser; RecordsDispatched counts nonempty in-range hook delivery. Read-ahead bytes can precede consumption and need not equal total bytes on range completion.                                                                                                    |
| Effects             | Existing import=true hooks and trigger/timer isolation apply; only admitted records populate new encounters. Existing graph remains retained.                                                                                                                                                                                        |
| Failure/cancel      | Existing cooperative cleanup drains accepted actions and restores live parser state. Partial imported history remains visible. Reparse is not transactional and does not deduplicate repeated imports.                                                                                                                               |

ACT uses seek heuristics to find a starting neighborhood. The managed reader scans
sequentially to retain deterministic physical-record boundaries and prefix zone
context. Parsing/decoding failures are surfaced, rather than silently skipping them.
No user file is modified by the reparse operation.

## Validation and remaining wiring

`ImportRangeChecks` uses disposable synthetic files to verify inclusive/equal bounds,
same-time records, undated/blank rows, prefix zone context, first-later termination
with a subsequent backward timestamp, UTF-8 BOM/mixed line delimiters, empty range,
malformed UTF-8 with retained prefix, explicit cancellation, reuse and live-state
restoration. Repeated boundary-at-final-queue imports check that internal producer
cancellation is not reported as user cancellation or a read error.

The Bridge's existing import operation adds start command 4 with inclusive ticks at
bytes 24/32, validates DateTime bounds and reserved fields before admission, then
uses the existing advance/cancel commands. Full import remains start command 1.
Rust `ImportRange::new(start_ticks, end_ticks)` validates bounds before constructing
a value; `start_import_range(path, range)` returns the same observable/cancellable
ImportJob, and `import_log_range(path, range)` awaits its result.

The real-worker import regression verifies matching inclusive/equal-time rows,
retained action/log identity, empty range, malformed prefix/suffix, progress,
partial results, cancellation/reuse, shutdown and stale-handle rejection.
The existing Import / Export pane now has a separate Rust/UI source-reparse
section with an explicit path and required inclusive start/end. Calendar dates
include the whole end day; exact timestamps include all matching boundary rows.
Drafts remain after validation, I/O failure, cancellation and host stop. Checkpoint
restore remains a distinct action with its own metadata search semantics.

Controller regressions use an actual Host for duplicate admission, missing paths,
partial encoding failure, empty ranges, navigation/cancellation, new partial
encounter identity, shutdown and stale responses. A VirtualDom-owned Signal drives
the actual shared ImportJob wait loop to check UI cancellation propagation and
stale progress exclusion after session change. Shared civil parsing also covers
DateTime's maximum day and 100 ns precision without timezone conversion.
The Windows desktop scenario is discoverable but unexecuted on ser7; native
clicks, focus and Windows runtime/input remain separate QA.

## Remaining ACT compatibility assessment

| Gap                       | Assessment and next boundary                                                                                                                                                                                                                                                                                                             |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Configurable LogEncoding  | Managed reader and explicit Bridge/Rust options are implemented with strict/standard-replacement policies and byte-progress regressions. Shared GUI selection, request snapshots and session-only drafts are connected with Host controller regressions. See [import encodings](import-encoding.md); no locale or content guess is used. |
| Companion logs            | A real ACT pipeline gap. Linux can test synthetic synchronized streams, cancellation/ordering, missing files and cleanup. This requires companion registration/dispatch and bounded multi-reader ownership beyond the single-file range unit. Native plugin-specific companion effects still need Windows QA.                            |
| Automatic source matching | Folder/character hints do not establish identity. Keep explicit source selection for this unit; evaluate ACT's metadata-based matching separately with missing/ambiguous-file behavior.                                                                                                                                                  |

Live-follower encoding is implemented with the [documented reference limits](follower-encoding.md).
Companion dispatch and automatic source matching remain tracked in the finite
[companion/source continuation](companion-source-plan.md). See the encoding contract
for the supported decoder scope and native QA limits.
