# Asynchronous file import

For saved ACT encounter XML/gzip and persistent history, use the separate
[archive APIs](encounter-archives.md).

`host.import_log(path).await` returns an owned `ImportReport` after reading the captured
file prefix and draining submitted ACT
actions. Async disk input feeds bounded batches, while the internal owner thread
dispatches parser hooks in order between callback/timer/command processing.

| Operation                  | Contract                                                                                                      |
| -------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `start_import(path).await` | Admit one operation and return an `ImportJob`; conflicting mutations return Busy                              |
| `job.progress()`           | Coalesced read/dispatch counters and input byte range; read-ahead bytes can lead dispatched records           |
| `job.cancel()`             | Request cooperative cancellation independently of normal command capacity                                     |
| `job.wait().await`         | Wait for input cleanup, accepted actions and live-state restoration; canceled imports return Error::Cancelled |
| Dropped job/future         | An admitted import keeps running; cancellation is explicit                                                    |
| Host shutdown              | Cancel the active import and wait for restoration before plugin teardown                                      |

Already processed history is retained on cancellation as a partial import. Live
timers keep their original monotonic clock while imported timestamps change the
parser clock. Plugin callbacks cannot be preempted; no fixed cancellation deadline
is promised. The managed compatibility entry point remains synchronous.

## Input and ownership

| Boundary           | Policy                                                                                                                                                             |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Source             | Explicit UTF-8 path; open once and read only its initial byte length; later appends are excluded                                                                   |
| Framing            | Strict UTF-8, optional byte-zero BOM, CR/LF/CRLF, final unterminated record; empty records are counted but not dispatched                                          |
| Bounds             | At most 8 MiB per record; invalid encoding, oversized records and premature truncation throw                                                                       |
| Live combat        | Active live combat rejects import; `host.end_combat()` ends it before importing                                                                                    |
| Coordination       | Pause and quiesce live file delivery before import notification; prevent foreign action producers and nested lifecycle operations from taking import ownership     |
| Callbacks          | `LogFileChanged(true,path)`, mutable Before/On raw hooks with import flag, then `LogFileChanged(false,livePath)` after restoration                                 |
| Time               | The installed timestamp parser supplies hook time; MinValue does not replace LastKnownTime; GlobalTimeSorter increments before dispatch and is not reset by import |
| Aggregation        | New individual encounters enter the import sentinel; six-second idle boundaries are checked before raw hooks; EOF ends combat and drains submitted actions         |
| Restoration        | Restore live zone, path, character, timestamps and prior pause state in `finally`, including input/subscriber failures                                             |
| Failure            | Retain partial imported history; report failure after draining and restoration rather than returning a successful partial report                                   |
| Owner dispatch     | Periodically pump control callbacks while importing and while draining; callbacks cannot preempt execution or recursively import/stop                              |
| Host category keys | The host environment provides ACT's built-in category keys; bare static defaults remain empty and registries remain plugin-owned                                   |
| Player alias       | ACT's default `YOU`; the original parser intentionally returns `ActGlobals.charName` for its current player instead of the logged actor name                       |

Inclusive source date windows are available through `ImportRange`,
`start_import_range` and `import_log_range`; see [source range contracts](source-range-import.md).
Companion files, encodings other than strict UTF-8 and remaining ACT import UI
options are unsupported.
The public ACT data graph remains mutable for compatibility; direct concurrent
replacement of its collections bypasses the host's coordination.

The captured length bounds the read, not an immutable copy of the file. Use a
stable archive for reproducible analysis; concurrent overwrites or a rapid
truncate-and-regrow can change bytes without an observable short read.

## Report meaning

| Field                | Meaning                                                                                             |
| -------------------- | --------------------------------------------------------------------------------------------------- |
| `lines_read`         | Framed records, including empty records                                                             |
| `records_dispatched` | Nonempty records delivered to raw hooks; not an accepted/parsed count                               |
| `encounters`         | New individual import encounters                                                                    |
| `combatants`         | Sum of participant counts across those encounters; repeated names are counted again                 |
| `damage`, `healed`   | Allied totals calculated from actual actions and plugin-registered categories; signed 64-bit values |

Plugin-private failures can be caught inside the original parser without reaching
host diagnostics. Successful raw dispatch and zero host diagnostics alone do not
establish parser acceptance. The original plugin also retains certain deferred
events until a later timestamp. EOF drains the ACT action queue; it does not flush
those parser-private events. The importer does not invent a future input record.
The [compatibility tests](compatibility.md) cover selected synthetic and recorded
inputs, with no guarantee of complete deferred-effect handling.

## Verification

| Check                  | Scope                                                                                                                                                                      |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Reader regressions     | Partial reads, split Unicode/BOM/newlines, exact size limit, malformed bytes, premature EOF/truncation and ignored appends                                                 |
| Root regressions       | 400-damage actions, mutable hook order, MinValue timestamps, empty/repeated imports, idle segmentation, exclusive ownership, failure restoration and resumed live delivery |
| Native boundary        | Validate the complete session/header/path and reserve output before side effects; reject malformed UTF-8, NUL, short/oversized requests and invalid ranges                 |
| Rust integration       | Fixture plugin generates actions through the actual CLR boundary; repeated imports and failure recovery preserve session ownership                                         |
| Original FFXIV fixture | Repeated seven-record imports, 100/300-damage `YOU` actions, UTC instants, ordered sorters, live-context restoration and private parser diagnostics                        |

The test fixture [ffxiv-synthetic.log](../managed/tests/fixtures/ffxiv-synthetic.log)
is generated by the supplied original `LogFormat` and `LogOutput.WriteLine`, including their real
checksum routine. It is synthetic input, not a game recording or an FF Logs
acceptance test. It contains territory/player/actor/party state and two plain
action effects with 100 and 300 damage. Generation does not start the writer's
capture thread.
