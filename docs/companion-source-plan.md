# Companion logs and history source matching

This is the finite continuation of the gaps in [source range import](source-range-import.md).
The pinned ACT 3.8.5.288 artifact has SHA-256
`934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`.
Only disposable synthetic files are used for verification.

## Inspected ACT behavior

| Entry point                         | Observed contract                                                                                                                                                                                                                                                                     | aftercast implication                                                                                                                                                                                                 |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| GetCompanionFilePath                | Main file's directory / CompanionLogs / main stem + period + companion name + main extension. Blank names and periods are rejected.                                                                                                                                                   | Preserve public API and ordinary valid relative/absolute paths. Reject separators/control/glob/stream characters as an explicit path-safety tightening. No file creation.                                             |
| OpenCompanionLogs                   | Enumerates the configured log folder's CompanionLogs using the current main basename and extension; captures LogEncoding; seeks companion streams to EOF.                                                                                                                             | Need owned live readers, shared pause/stop/import lifecycle, and generation invalidation. Confine discovery to the explicitly selected main file's sibling CompanionLogs; do not search unrelated configured folders. |
| CompanionLogFile.ThreadCompanionLog | StreamReader(Stream, LogEncoding); CR/LF split, pending incomplete suffix. Each physical record obtains GetDateTimeFromLog, observes pause/abort and main LastEstimatedTime, increments GlobalTimeSorter and passes CompanionLogName to ParseRawLogLine.                              | Companion identity is a named source, not a same-line-number join. Reuse validated codecs and raw hook/capture/aggregation contracts. Native scheduling does not establish a deterministic tie order across threads.  |
| ImportFile                          | Creates paused companion readers at byte zero with SkipToTime=dtBegin; releases them around main import. Main/companion timestamp/queue gates coordinate progress. Cleanup aborts/waits readers.                                                                                      | Need bounded multi-source admission/read-ahead, inclusive range policy, cancellation/error cleanup and original source/name ownership. Existing one-reader import is not sufficient.                                  |
| ThreadHistoryImport                 | Starting from current log directory, searches FolderHint under a parent and recursively filters LogFileFilter; optionally requires CharName in path; sorts LastWriteTime and chooses first later than EndTime. Missing match opens file picker, then imports inclusive record bounds. | These are heuristics, not identity. Do not copy recursive broad search or automatically select an ambiguous/changed file. Folder/character/time hints may explain candidate suggestions only.                         |

These are original-code observations. The pure GetCompanionFilePath reference
probe runs against the genuine pinned binary and aftercast. Native companion
threads/history dialogs have not run on Linux. The existing shim cannot execute
FindZoneName because System.Windows.Forms.Application is absent; no UI shim
expansion or Windows validation is implied by constructor/path probes.

## Current gap and bounded work units

| Unit                                | Deliverable and completion evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | State                                                                                             |
| ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| C1 Names and candidate catalog      | Public ACT path API; literal, top-level sibling discovery, case-insensitive unique names, deterministic order; reject linked/malformed/ambiguous candidates and fail explicitly above 64 companions/4096 directory entries. Synthetic path/name/link/bounds regressions and original pure API comparison.                                                                                                                                                                                                                         | Implemented; used by explicit managed companion import admission. Live admission remains pending. |
| C2 Managed readers and dispatch     | Open selected main/companion handles together at admission, capture identity/length/encoding, retain handles rather than reopening catalog paths. Reuse parser/owner dispatch; populate companionLogName and each record's actual source path, distinct sorter and action/log identity. Test same timestamps, undated/empty/partial lines, range boundaries, malformed bytes, rotation/truncation, pause/cancel/stop and one-source failure with partial results. Specify stable ordering without claiming ACT thread tie parity. | Managed import connected with explicit selection; live follower remains pending (C2b).            |
| C3 Bridge/Rust and existing UI      | Expose bounded request-owned companion choices/catalog and source status through existing import flow. Preserve explicit encoding snapshot, revisions/session ownership, draft/error/cancel/navigation/duplicate request behavior and controller tests. Do not present discovery alone as synchronized-import completion.                                                                                                                                                                                                         | Pending.                                                                                          |
| S1 Bounded history source proposals | Reuse selected history snapshot/time bounds. Inspect only explicit paths, recorded source references, or individually allowed roots; no parent climbing, recursive scan or external lookup. FolderHint/CharName/LastWriteTime are advisory. Return missing/ambiguous/changed outcomes; require explicit user selection when identity is not established.                                                                                                                                                                          | Pending. Current history metadata contains hints, not a trusted exact original-file identity.     |
| S2 Selection and reparse wiring     | Bind chosen source and file identity to the owned request, revalidate at admission and retain open handle. Connect to existing source-range import separately from checkpoint restore. Preserve selection drafts and reject stale history/catalog/session responses, changed files and duplicate requests. Synthetic Host/API/controller tests.                                                                                                                                                                                   | Pending.                                                                                          |

A catalog result is a proposal, not a file lease or a security boundary against
concurrent path replacement. C2/S2 must validate selected files and own their
handles at admission; metadata such as size/mtime alone must not authorize a
changed file. Missing directory yields an empty catalog; inaccessible paths,
non-directory containers and matching symlinks fail explicitly. Unmatched entries
are not opened, and nested directories are not searched. An unselected catalog candidate is never read merely because its path was proposed.

Exact native plugin companion effects, WinForms history file dialogs and Windows
filesystem behavior remain separate execution QA. This plan adds no capture,
upload, AI feature, WinForms rendering bridge or stronger durability policy.

## C2a managed import checkpoint

`HostEnvironment.ImportLog` / `BeginImport` accept an optional copied list of
`ImportCompanion(Name, Text)`. Omission retains the single-file path and does not
implicitly read sibling files. Explicit selection uses the bounded catalog,
rejects missing/duplicate names and source file aliases, opens every selected
handle before publishing records, captures each initial length and codec, and
keeps those handles through rename/replacement. Null companion Text inherits the
admitted main codec/BOM choice; explicit text options affect only that reader.
Ordinary replacement detected during open/recheck fails admission. This is not
an adversarial race-proof path authorization mechanism; C3's displayed-candidate
revision/identity binding remains required before exposing automatic proposals.

| Contract       | Implemented behavior                                                                                                                                                                                                                                                                                                           |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Owner          | Timestamp parser, mutable raw hooks, capture and combat dispatch stay on the owner thread; only disk framing runs on the input task.                                                                                                                                                                                           |
| Ordering       | Preserve physical order within every file. Pick the lowest timestamp among current heads; ties choose main first, then companion names ignoring case. Backward timestamps are not globally resorted. This is deterministic scheduling, not ACT native thread tie parity.                                                       |
| Clock          | Main records keep the existing clock semantics. Companion timestamps may advance LastKnownTime but cannot rewind it; undated records retain the shared clock while keeping raw MinValue hook time.                                                                                                                             |
| Names/identity | Raw events carry companionLogName; immutable managed OriginalLogRecord captures actual source Path and CompanionLogName. Each dispatched record obtains its own global sorter, retaining distinct action/log identity across equal timestamps and private archive restart. Rust transport of companion metadata is pending C3. |
| Bounds         | One ready record per source, one response in transit, at most one outstanding request per source; reject aggregate ready text above 64 MiB UTF-16. Record framing remains bounded to 8 MiB source bytes. Large decoder buffers are released after each record rather than retained per admitted reader.                        |
| EOF            | Main retains its existing unterminated final-record support. Companion input follows ACT's complete-record requirement: an incomplete EOF suffix is excluded and reported as a warning, including a partial malformed character.                                                                                               |
| Range          | Apply inclusive bounds per source and retire each at its first later timestamp. Companion sources also stop at the terminal main watermark; they cannot drain future dated suffixes after main completion. Out-of-range main prefix may supply zone context; companion prefixes do not override it.                            |
| Failure/cancel | Keep admitted partial history, report errors, cancel input, dispose all readers and drain accepted actions before restoring live context. A low-level Host Stop still rejects active import; cancel/drain before disposal, as existing lifecycle requires.                                                                     |

Synthetic managed Host tests cover mixed UTF-8/949/BOM UTF-16 input, mutated hook
text versus originals, source and option snapshots, same-time totals/identities,
archive restart, malformed complete records and replacement policy, incomplete
EOF, inclusive ranges, backward/undated records, captured append/rename, premature
truncation, cancellation/reuse, missing selections, hardlink aliases on Linux,
head limits and exclusive file opens after cleanup. The native ACT companion
thread itself has not been executed. Bridge/Rust/GUI selection and **live companion
following (C2b)** are still unconnected; no completion claim covers those paths.

### C2a main EOF/watermark contract

The inspected ACT CompanionLogFile.ThreadCompanionLog waits for LastEstimatedTime
for dated records, skips that time gate for MinValue, and only advances the shared
clock. ImportFile observes main timestamps before its range comparison, locks
companions when the main loop finishes, then aborts them in finally. The genuine
CompanionPathProbe now also binds LastKnownTime/LastEstimatedTime: MinValue does
not replace known time; ordinary main assignments can move backward. These are
actual property contracts, not execution of the native companion thread.

Offline replay uses the main file's current head as timestamp look-ahead and the
last observed non-MinValue main timestamp as its terminal watermark. It does not
advance an import watermark with elapsed wall time. This removes the former
unbounded post-main drain and makes the following rules deterministic:

| Boundary                  | Companion inclusion/exclusion                                                                                                                                                                                                                                                            |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Normal main EOF           | Drain complete companion records no later than the terminal main timestamp; include equal-time rows. Retire a companion at its first later head and never search past that head for older records.                                                                                       |
| Empty or blank-only main  | Validate/open requested files, but do not decode or dispatch companion records. Empty input supplies no main clock or meaningful replay context.                                                                                                                                         |
| Undated-only main         | Undated complete companions may dispatch; dated heads cannot pass the unset main watermark. A later undated main record does not erase a previously known timestamp.                                                                                                                     |
| Main clock moves backward | Terminal watermark follows the last observed dated main record, not the maximum ever observed. Already admitted records stay retained.                                                                                                                                                   |
| Inclusive range end       | The first main timestamp beyond End is an observed termination boundary even though it is not dispatched; companions still require the requested inclusive range. Out-of-range/future heads retire their source without reading later suffix records.                                    |
| Cancellation/error        | Admit no more raw callbacks after cancellation is processed or an input error is observed. Dispose input and drain already accepted actions, retain partial history and restore live context. Do not use EOF filtering to hide an error needed to obtain the next candidate's timestamp. |

Main blank records are consumed before opening the companion read gate. All
handles are still admitted together; missing/aliased/invalid selected sources
remain admission errors even when the main file is empty. A known rejected
future head prevents decoding its later suffix, so malformed bytes behind that
head cannot turn excluded data into an import error. Malformed bytes in a needed
head remain an explicit error because no trustworthy timestamp can be inferred.

Managed ImportProgress/ImportResult expose CompanionHeadsExcluded: the number of
already framed future heads rejected by the main terminal watermark, at most one
per retired companion. This is not the unknown number of unread suffix records.
LinesRead includes those framed heads; RecordsDispatched excludes them. C3 must
carry this metadata through Bridge/Rust/UI; the existing transport still has no
companion selection or exclusion field.

These are deliberate offline scheduling rules derived from ACT's main-clock and
termination gates. Equal-time records use main then name order; empty/undated
races and wall-clock drift are not reproduced. **They are not a claim of native
ACT thread timing parity.** The genuine path/clock/StreamReader probes run, while
native companion threads and history dialogs remain unexecuted. C2b live following
and C3 transport/UI remain pending.
