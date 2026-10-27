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

| Rule                | Current managed implementation                                                                                                                                                                                                                                   |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Bounds              | `ImportRange` contains inclusive DateTime wall-clock bounds. Equal bounds are valid; reversed bounds fail before admission. No timezone conversion is inferred.                                                                                                  |
| Same timestamp      | Every matching physical record is dispatched in file order with a distinct global sorter.                                                                                                                                                                        |
| Undated/blank lines | DateTime.MinValue follows ACT's comparison; for ordinary finite history ranges undated records are skipped. Blank lines count as read without hook dispatch.                                                                                                     |
| End                 | Stop at the first timestamp strictly after the end, including out-of-order files. Do not extend to combat end.                                                                                                                                                   |
| Source context      | Scan the prefix from byte zero; preserve preceding raw zone-regex matches without dispatching out-of-range callbacks. No source-file guessing from folder hints.                                                                                                 |
| Reader              | Reuse strict UTF-8, optional byte-zero BOM, CR/LF/CRLF framing, final unterminated record, bounded 8 MiB records and captured initial file length. ACT's configurable `LogEncoding` and companion-log reader are not implemented by this existing import reader. |
| Progress            | LinesRead includes skipped and terminating records consumed by the parser; RecordsDispatched counts nonempty in-range hook delivery. Read-ahead bytes can precede consumption and need not equal total bytes on range completion.                                |
| Effects             | Existing import=true hooks and trigger/timer isolation apply; only admitted records populate new encounters. Existing graph remains retained.                                                                                                                    |
| Failure/cancel      | Existing cooperative cleanup drains accepted actions and restores live parser state. Partial imported history remains visible. Reparse is not transactional and does not deduplicate repeated imports.                                                           |

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
GUI source-path/range controls are the next unit. They must distinguish inclusive
reparse bounds from exclusive metadata search bounds and reject stale session/UI
responses. Native file dialog/input and Windows execution remain separate QA.

## Remaining ACT compatibility assessment

| Gap                       | Assessment and next boundary                                                                                                                                                                                                                                                                                  |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Configurable LogEncoding  | A real ACT import compatibility gap, not a Windows-only limitation. Linux can implement explicit supported encoding selection, strict decoding/framing and byte-progress tests. It needs its own reader/ABI/UI unit; never silently guess or reinterpret input as UTF-8.                                      |
| Companion logs            | A real ACT pipeline gap. Linux can test synthetic synchronized streams, cancellation/ordering, missing files and cleanup. This requires companion registration/dispatch and bounded multi-reader ownership beyond the single-file range unit. Native plugin-specific companion effects still need Windows QA. |
| Automatic source matching | Folder/character hints do not establish identity. Keep explicit source selection for this unit; evaluate ACT's metadata-based matching separately with missing/ambiguous-file behavior.                                                                                                                       |

These gaps remain tracked follow-ups; this API does not claim arbitrary ACT log
encoding, companion-file or automatic source-location parity.
