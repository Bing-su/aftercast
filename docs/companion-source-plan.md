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

| Unit                                | Deliverable and completion evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | State                                                                                     |
| ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| C1 Names and candidate catalog      | Public ACT path API; literal, top-level sibling discovery, case-insensitive unique names, deterministic order; reject linked/malformed/ambiguous candidates and fail explicitly above 64 companions/4096 directory entries. Synthetic path/name/link/bounds regressions and original pure API comparison.                                                                                                                                                                                                                         | Implemented; explicit managed import and live admission reuse the catalog.                |
| C2 Managed readers and dispatch     | Open selected main/companion handles together at admission, capture identity/length/encoding, retain handles rather than reopening catalog paths. Reuse parser/owner dispatch; populate companionLogName and each record's actual source path, distinct sorter and action/log identity. Test same timestamps, undated/empty/partial lines, range boundaries, malformed bytes, rotation/truncation, pause/cancel/stop and one-source failure with partial results. Specify stable ordering without claiming ACT thread tie parity. | Managed import/live connected; Rust source transport verified; GUI pending.               |
| C3 Bridge/Rust and existing UI      | Expose bounded request-owned companion choices/catalog and source status through existing import flow. Preserve explicit encoding snapshot, revisions/session ownership, draft/error/cancel/navigation/duplicate request behavior and controller tests. Do not present discovery alone as synchronized-import completion.                                                                                                                                                                                                         | Explicit catalog/selection/status GUI connected; synthetic Host regressions verified.     |
| S1 Bounded history source proposals | Reuse selected history snapshot/time bounds. Inspect only explicit paths, recorded source references, or individually allowed roots; no parent climbing, recursive scan or external lookup. FolderHint/CharName/LastWriteTime are advisory. Return missing/ambiguous/changed outcomes; require explicit user selection when identity is not established.                                                                                                                                                                          | Managed bounded metadata proposals implemented below; transport/selection wiring pending. |
| S2 Selection and reparse wiring     | Bind chosen source and file identity to the owned request, revalidate at admission and retain open handle. Connect to existing source-range import separately from checkpoint restore. Preserve selection drafts and reject stale history/catalog/session responses, changed files and duplicate requests. Synthetic Host/API/controller tests.                                                                                                                                                                                   | Pending.                                                                                  |

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
thread itself has not been executed. Live following is described in C2b below;
Bridge/Rust/GUI selection remains unconnected.

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
is connected below; C3 transport/UI remains pending.

## C2b managed live source groups

`HostEnvironment.OpenLogWithCompanions(companions, getCurrentZone, maxClockWait)`
uses the explicitly configured main LogFilePath and a copied, name-sorted source
selection. It reuses ImportCompanion(Name, Text) for the source codec descriptor;
this does not start an import. Ordinary ACT OpenLog stays a single-source path.
`ReadLiveLogSources()` returns managed status snapshots for the current reader selection, including ordinary single-main following.
The Bridge endpoint is connected below; Rust/GUI selection and status transport
are **not connected** yet.

Every main/companion reader opens before the dispatcher starts and seeks its
initial EOF. Existing history is not replayed; the initial zone snapshot belongs
to the main handle only. Source names, codecs and paths are captured. Reader file
identities reject aliases, and matching links or a detected admission change
fail explicitly. A failed replacement retires the old group as existing OpenLog
already does; it does not silently reactivate an old selection. Open failures
join any earlier tail readers and release their handles even when another reader
reports a cleanup error. The normal follower and a fresh explicit group can then
be opened again.

| Resource or time boundary | Contract/configuration                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Live companion count      | Fixed implementation safety limit of 8 companions plus main. Metadata/import catalogs may contain up to 64; this does not authorize 64 live worker threads. No saved user option or GUI setting changes this limit.                                                                                                                                                                                                                            |
| Queues and heads          | Fixed queue capacity 1 and at most one dispatcher head per source, plus one producer callback record in transit. Backpressure stops further framing rather than growing an unbounded queue.                                                                                                                                                                                                                                                    |
| Buffered text             | Fixed aggregate 64 MiB of UTF-16 text reserved by transport callbacks, queued records and heads. This is not a process RSS bound: decoder work/transient strings and thread stacks are additional. Exceeding the budget fails the offending source explicitly.                                                                                                                                                                                 |
| Physical record           | Existing fixed 8 MiB source-byte limit. Large completed-record decoder allocations are released rather than retained per reader. Malformed/oversized records report diagnostics and recover at the next delimiter.                                                                                                                                                                                                                             |
| Clock wait                | Default 30 seconds of uninterrupted active future-head waiting. A positive per-open managed maxClockWait up to 5 minutes is allowed; it is not persisted and has no GUI control yet. Pause or an eligible transition resets the wait interval, even if another ready head is selected first. A later clock regression starts a fresh interval. Timeout is checked cooperatively; it cannot preempt a stuck plugin callback or blocking OS I/O. |
| Retired readers           | Existing fixed 32-unit root lifecycle budget now counts file workers and group dispatchers, preventing reentrant group replacement from bypassing the old reader bound.                                                                                                                                                                                                                                                                        |

The single live dispatcher runs timestamp parsing and raw callbacks in the
existing live reader-thread style, **not the Host owner thread**. Disk workers
only frame and queue records. Eligible ready heads are selected by timestamp;
ties among currently ready heads prefer main then companion name. Main timestamp
look-ahead and LastEstimatedTime admit dated companion heads. MinValue bypasses
the time gate; every source still honors dispatch pause. EOF remains an idle wait
for append, unlike import's terminal watermark. No waiting for an absent source
or claim about future equal-time arrivals is made. Native ACT thread tie order
and timing remain unverified.

Companion raw timestamps and names are preserved; late/undated records do not
rewind LastKnownTime. Automatic raw zone context follows main only; plugins may
still explicitly change zone from their hooks. OriginalLogRecord carries actual
source path/name before mutable hook edits, and every dispatched record receives
a distinct sorter. Import suspension waits for the group's active parser cycle,
while disk transport may retain bounded input. Resume retains those records.

File replacement/truncation resets the reused follower decoder and increments
its source generation. Pending prior-generation records are invalidated with
diagnostics and Dropped counts rather than dispatched under a new image. A
periodic identity/length check also releases a future head blocked ahead of a
replacement. Complete queued old-image records or old unread suffixes can be
omitted during this invalidation; this is explicit partial-state handling, not
a lossless recording promise. Missing paths during rename/create keep the old
handle until a replacement appears. An in-place truncate/regrow beyond the
observed read position can remain indistinguishable from append, as with the
single follower; concurrent overwrites are not immutable snapshots.

Timeout, budget or fatal I/O failure marks that source failed and stops its
reader. Healthy main/companions continue when a companion fails; main failure
stops the group. There is no automatic retry or silent skip-to-current-time.
An explicit reopen starts a fresh tail selection. Status exposes following,
paused, waiting_for_clock, changing, failed or stopped, pending time, bounded
queue count, source generation, dropped framed records and last diagnostic.
These are current-group observations, not authority tokens. C3 must bind root
selection generation and session/revision ownership; source generation alone
resets on a new selection. Status disappears when the root detaches the group.

Synthetic append-file tests cover Unicode/949/BOM and explicit BOM suppression,
raw path/name/clock and ready-head ties, late records, future waiting across
pause and eligible-tie/backward-clock transitions, aggregate budget saturation,
invalid options preserving an active group, generation invalidation during paused rotation/truncation, malformed
record recovery, timeout/partial operation/explicit retry, joined stop/exclusive
opens, reentrant replacement and import suspension. A dedicated last-source
exclusive-lock test reaches FileStream open **after main and an earlier companion
reader started**, then verifies earlier handle release, old dispatcher exit and
ordinary/group retry. Native ACT companion execution, Windows filesystem/UI
interaction and live game capture have not been run.

## C3a Bridge checkpoint

CompanionCommand is a separate bounded ABI; the existing 64-byte import operation
and ordinary OpenLog defaults remain unchanged. Each call supplies a 32-byte
version-1/session header and exactly 512 KiB reply capacity, reserved before any
callback or mutation. Input JSON is 2..128 KiB, strict UTF-8, at most depth 16,
object-only, rejects duplicate/unknown keys, and validates command-specific
fields. Main paths are absolute, control-free, at most 8192 UTF-8 bytes. Source
name/codec/count, inclusive ticks and positive clock timeout <=300000 ms are
validated before admission. Output diagnostics are capped to 2048 UTF-16 units
with an ErrorTruncated marker; path/count limits bound the rest of the reply.

| Command                     | Ownership and response                                                                                                                                                                                                                                                               |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1 Begin source import       | Explicit main Path, nullable Text, explicit Companions array (empty means single source), optional paired inclusive StartTicks/EndTicks. Returns a new opaque owner ID; no native pointer or caller object is retained.                                                              |
| 2 Advance / 3 Cancel import | Require that exact active owner ID. Reply contains pending progress, final result or cancellation, always tagged with that owner. Progress/result carry CompanionHeadsExcluded. Completion/failure/cancel retires the ID. Legacy continuation cannot advance an owned source import. |
| 4 Open/replace live         | Owner=0 opens a fresh selection; replacing requires the current owner. Path, Text, Companions, GetCurrentZone and per-open ClockWaitMilliseconds are captured. Main explicit decoding does not overwrite plugin LogEncoding. Returns the captured selection and owned source status. |
| 5 Live status               | Require current live owner and the root generation captured at open, checked again after snapshot creation. A plugin's external OpenLog invalidates that native selection; merely changing a draft path does not redirect an owned reader.                                           |
| 6 Close live                | Require the exact current owner/generation, join readers without shutting down the Host or combat engine, then retire the owner. Reopening is a new explicit request, never implicit handle reuse.                                                                                   |

Owner IDs are monotonic within the PluginSession and commands also require the
active outer session header. Import and live IDs are distinct allocations.
Invalid option/schema/capacity requests preserve an active live selection. An
attempt that reaches a failing open retires the old capture according to existing
OpenLog semantics and leaves no new live owner; a fresh Owner=0 request can retry.
Session stop clears import/live ownership. Current-group status exposes source
name/path, state, queue count, pending ticks, per-image generation, dropped head
records and bounded diagnostic text. Single-main status now exposes its owned
path, image generation and last diagnostic too. Source image generation alone is
not a session or selection authority.

CompanionAbiChecks calls the actual managed entry point with native buffers and
a synthetic initialized parser. It covers short buffers/reserved/session fields,
malformed UTF-8/schema/duplicate JSON, invalid codecs/ranges/options, owner-bound
import completion/exclusion/cancel, caller snapshot independence, stale IDs,
empty-companion single-source mode, external plugin reopen and a later locked
source's real partial-open failure/cleanup/retry. Managed full checks pass.
These tests are in-process ABI tests, **not isolated Rust-worker coverage**.
C3b now supplies Rust Request/delegate binding, typed session-owned selections
and isolated worker tests below. GUI remains a separate pending C3 unit. Native
ACT thread timing and Windows execution are still unverified.

## C3b Rust worker checkpoint

`HostHandle::start_import_sources` / `import_sources` accept an owned
`SourceImportOptions` with an explicit companion list, optional inclusive range
and main/reader codec snapshots. They reuse `ImportJob` progress, explicit cancel,
partial-result retention and joined cleanup. Dropping an import observer does
not cancel work. Host shutdown cancels the active import before worker exit.
Legacy import entry points still use their existing 64-byte command and omit
companions; an explicit empty list also reads only the main source. Progress and
results expose `companion_heads_excluded` (legacy single-source results use zero).

`open_live_logs`, `reopen_live_logs`, `live_logs` and `close_live_logs` use an
opaque `LiveLogSelection` containing private session and owner IDs. The managed
owner also binds the root generation, so an external plugin OpenLog retires old
selections. Copying/dropping a selection does not close capture; explicit close
joins readers while the Host remains usable. Dropping the Host requests worker
shutdown even if cloned HostHandles remain. Invalid request preflight preserves
a selection; actual failed replacement retires it and requires a fresh open.

Each live source carries `selectedName` (exact admitted request identity) separately
from the catalog's physical `name`/`path`. The Bridge performs .NET
OrdinalIgnoreCase matching and Rust verifies exact selected-name membership,
one-to-one coverage, physical name/path component structure and bounded status.
This avoids inventing a different Unicode casing policy in Rust. The Bridge is
trusted to resolve Unicode equivalence and the main filename's case-insensitive
catalog prefix/extension; a second canonicalization RPC would not establish a
new trust boundary. Malformed headers/session/owner, inconsistent source identities,
invalid states/codecs/counts/diagnostics and contradictory import replies fail
without publishing a typed result. This validation is not recovery from an
arbitrarily corrupt worker after an already-mutating command.

| Evidence                           | Verified scope                                                                                                                                                                                                                |
| ---------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rust decoder/unit tests            | Header/session/size/owner, exact selected-name pairing, physical filename component (including extensionless main), unknown state/codec, bounded counters/status and contradictory progress/results.                          |
| Actual isolated application worker | Explicit import merge/main EOF exclusion, legacy/empty-list single source, inclusive same-time range with per-reader UTF-16 BOM, Unicode case-preserving selection and nonalphabetic request order.                           |
| Actual isolated worker lifecycle   | Cancel with retained partial graph, observer drop, decoding error/retry, active shutdown/restart; live reopen/close/stale and foreign session; clock timeout diagnostics and explicit retry; external root generation change. |
| Linux synthetic file locking       | Later selected source open fails after main/earlier reader opens; error returns after handle cleanup; old owner retires; fresh retry and Host drop/restart release readers/data lease.                                        |

Tests use disposable files and synthetic plugins only. The actual worker checks
exercise the subprocess IPC/delegate path, unlike the earlier in-process ABI
checks. They do not establish Windows locking/UI behavior, native ACT companion
thread scheduling parity or live game capture. No companion selection GUI or
source candidate proposal/automatic matching is connected by this unit.

## C3c explicit source UI checkpoint

Import's existing main path field now scopes an explicit candidate query. Command
7 reuses the managed literal sibling catalog (64 candidates / 4096 entries),
without parser initialization or opening readers. It validates the bounded JSON
contract and does not make candidates into leases. Changing the main path clears
choices; stop clears session-owned candidates/selections. Existing C2 admission
resolves names and validates/open-owns all selected sources again. A missing,
ambiguous or linked replacement fails visibly. No recursive roots, automatic
history source matching or recorded-file identity guarantee is added.

The candidate editor reuses the incumbent Rust/UI-derived Button, Input, Label
and Accordion, and the existing Dioxus/native-select encoding pattern. Selection
and reader-specific codec/error/BOM drafts are captured by the actual full/range
import requests. A nonempty selection must match that exact catalog main path;
a different range path fails before native import. Empty choices keep the old
single-main entry points. Import progress and completed results distinguish
excluded future companion heads from live dropped records.

The same panel exposes explicit live open/reopen/close, initial-zone lookup and
per-open clock wait (1..300000 ms). It states the live 8-companion and fixed
64 MiB text budget. Draft settings are visibly separate from admitted import
snapshots and active live settings. Inherited plugin codecs are identified as
captured-at-admission rather than claiming an explicit code page the ABI did not
return. Source status includes exact selected/physical names, path, generation,
queue/drop counts, pending ticks and bounded/truncated error text. Editing a
codec or timeout never relabels or changes an existing reader; explicit replacement
admits the next snapshot.

Accepted work is app-owned across tab changes. Session/generation guards reject
late responses after stop/restart; duplicate mutation requests are refused. Failed
open probes the previous owner before retaining it: preflight failure may leave
it active, whereas actual failed replacement retires it and fresh open can retry.
Read-only status invalidation during path editing clears its pending flag.

Actual isolated Host controller regressions cover candidate/selection ownership,
request snapshots vs later drafts on the same file, full/range import and equal
boundaries, cancel with partial graph/draft retention, duplicate requests, tab
navigation, changed/missing files, Linux partial-open locking/cleanup/retry,
stale response invalidation and late-open shutdown. Earlier ABI/worker tests
remain separate evidence. Source proposal/automatic matching S1/S2 stays pending.
Native screen clicks/focus/layout, Windows filesystem/UI and native ACT companion
thread timing have not been verified; controller tests are not visual QA.

## S1a managed metadata proposal checkpoint

The original `ThreadHistoryImport` (pinned decompile lines 13278–13420) walks up
from the current log folder, recursively searches `FolderHint` / `LogFileFilter`,
optionally filters paths by `CharName`, sorts local `LastWriteTime` and picks the
first later than the record's `EndTime`. This is a heuristic, not an identity
contract. Native ACT history dialogs/thread execution have not been run here.

Current `history/index.xml` provides revision, record identity, inclusive source
start/end times, character and folder hints. Private encounter XML keeps action /
retained-log sorter identity but does **not** persist original physical paths.
`OriginalLogRecord.Path` is an ephemeral observer field. Archive payload paths,
FolderHint and character strings are not treated as recorded source references;
this unit invents none and never derives new search roots from them.

`HostEnvironment.ReadHistorySources[Async]` accepts exactly one selected history
ID and its current index digest, explicit absolute files (at most 16), explicitly
authorized roots (at most 8), and a filename-only glob. It copies inputs before
I/O, lists only top-level entries, bounds **all** root entries to 4096 total
(including nonmatching entries), and rejects above 64 combined candidate paths
including exact observations revisited from a previous snapshot. No parent walk,
recursive search, automatic fallback or automatic source selection is present.

Snapshots are immutable metadata proposals bound to the managed owner, selected
history revision/ID, inclusive times and normalized authorized scope. Refresh with
a different owner/history/scope fails. Index changes before/during discovery fail
without publishing a new result. Folder/character/local-write-time matches are
advisory only. Every snapshot requires explicit selection, even with one candidate.

| Observation            | Contract                                                                                                                                                                                                                   |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Overlap / aliases      | Exact input overlap merges provenance; device+file-ID duplicates merge aliases rather than become ambiguous independent matches. Full paths remain visible.                                                                |
| Multiple files         | Distinct physical files remain ambiguous even with equal basenames/size/mtime; none is chosen.                                                                                                                             |
| Refresh                | File ID, size, modification and change stamps distinguish ordinary replacement, metadata change, moved identity, missing path and unavailable observations. Moves are suggestions, never path substitution.                |
| Missing vs unavailable | An unreadable/unsupported scope reports incomplete diagnostics; it is not a proven missing/no-match result. Missing roots/paths are explicit. A failed or linked root is not followed through old candidate paths.         |
| Filesystem scope       | Final symlinks/devices/directories are rejected. Linux uses metadata-only `statx` and rejects FIFO before any data-stream open. Windows file-ID/basic-info observation compiles; native Windows execution remains pending. |
| Cancellation           | Checks across catalog/root/path work throw cancellation before a snapshot is returned. Proposal work does not modify history files or encounter graphs.                                                                    |

These stamps are **not content hashes, byte-equality proofs, leases or authority
to reparse**. Change time catches ordinary same-size rewrites with restored mtime,
but metadata resolution/filesystem behavior and concurrent writes limit it.
A proposal does not prove a particular byte image was retained. S2 must define
selection/admission using owned, verified read bytes (or verify consumed bytes
with explicit partial-result/error semantics); hash-before-reparse plus an open
descriptor alone cannot prevent concurrent in-place writes. No stronger durability
policy or journal is added.

Synthetic managed regressions cover equal-name ambiguity, explicit/root overlap,
hard-link aliases, bounded nonmatching entries/candidates, nested paths not searched,
character/folder hints not used as roots, same-size same-mtime rewrite, replacement,
rename/deletion refresh, immutable output/caller input snapshots, stale history,
foreign scope/owner/thread, cancellation, missing/non-directory/linked roots and
Linux FIFO rejection. This was the managed metadata checkpoint; the subsequent S2a,
Bridge and Rust checkpoints below complete captured replay and transport. GUI
selection remains subsequent. Existing explicit manual reparse stays unchanged.

## S2a managed captured-byte replay checkpoint

`CaptureHistorySourceAsync` requires an explicitly selected available path/alias
from an owner-bound S1 snapshot. It rechecks the history revision and the main
file's observed version, copies request/codec choices, resolves only explicitly
named sibling companions, and opens every original descriptor before copying.
Duplicate physical inputs, missing/linked/changed files and stale history fail
before any parser callback or encounter mutation. Capture work and ready images
are mutually exclusive with import/archive mutation; metadata queries remain
read-only. One pending/ready capture is owned by each managed Host.

| Resource / lifetime        | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Byte budget                | Caller may lower the aggregate limit; the default and hard ceiling are 4 GiB of source bytes across main plus at most 64 companions. Subtraction checks reject even huge/sparse observed lengths before copying, without sum overflow. Growth never extends the admitted source length. This is not a filesystem quota; metadata/allocation overhead is additional.                                                                                                     |
| Copy / verification memory | One 64 KiB copy buffer; SHA-256 for each 64 KiB chunk plus a whole-image digest. At most 65536+65 chunk hashes across the set (bounded managed object overhead in addition to digest bytes). Each opened replay reader has one 64 KiB verification buffer. Existing decoder/head limits still apply. This does not bound arbitrary plugin allocations.                                                                                                                  |
| Temporary storage          | New private `.aftercast-source-<nonce>.tmp` files in the application data directory, CreateNew/read-write/Share.None/DeleteOnClose, Linux mode 0600 and immediate unlink. No original path is opened for writing. No archive or permanent raw-log copy is created. Capture failure, explicit ready-image Dispose, import completion/cancel/failure and Host shutdown close all owned images/handles. No durability, secure-erasure or OS-crash atomicity claim is made. |
| Capture publication        | All selected handles are acquired and within budget before any copying. Handle/path versions are checked again across the completed set and the history digest is rechecked. Ordinary concurrent growth/rewrite/rename/replacement or index changes discard the entire preparation; no graph has been published. Cancellation closes intermediate images and originals.                                                                                                 |
| Replay authority           | An opaque managed `HistorySourceCapture` owns the exact copied byte sequence, paths/names, decoder/BOM policies and inclusive history bounds. It is one-shot. Preflight owner/history/combat errors preserve a ready capture; once transferred, failed admission closes it rather than silently reusing an uncertain request. Disposing the observer after transfer cannot close admitted readers.                                                                      |

`BeginCapturedHistoryImport` revalidates the owner/current history before transfer
and reuses the existing import engine. The single-main batch path stays intact;
companion replay reuses its deterministic owner timestamp/clock gate, initial zone
context, inclusive range, EOF exclusion and retained action/log identity. Both
paths open verified image readers, **never the original source paths**. Plugins
still see the captured original path and companion name, not scratch names.
The selected history character is applied for replay and the live character/parser
context is restored afterward. Parser plugin code/settings are not themselves
snapshotted.

Each replay chunk is read, checked against its capture manifest, and returned to
the decoder from that **same owned buffer**. Verification never rereads a path
or checks one buffer then decodes another. This also covers range imports that
stop before source EOF. Corrupt/truncated images fail before exposing the affected
chunk. Previously accepted records/actions can remain as explicitly failed or
cancelled partial results, matching existing import semantics.

After a ready capture is published, original-file changes do not retarget it:
replay uses the retained image even if the originals were moved, replaced or
deleted. A user must explicitly capture again for new bytes. History is checked
at admission; its immutable bounds/character then remain the admitted request,
not an ongoing external-index lease.

The guarantee is **replay of checksum-verified captured bytes**, not proof that
the chosen file was the historical original. S1 still lacks trusted historical
path/content references. Metadata version checks detect ordinary changes but do
not turn concurrent reads into a filesystem transaction; a writer restoring all
observable metadata could leave a mixed-time copied sequence. SHA-256 manifests
describe that captured sequence. Malicious native plugin memory access, storage
hardware failure beyond detectable reads, crash recovery and stronger journaling
are outside this contract.

Shutdown first performs a side-effect-free owner/callback/advance preflight.
Once admitted, captured import terminal errors are collected while readers,
combat runtime, controls and ACT globals continue through cleanup. Original input
errors and independent cleanup errors are reported together. A recorded single
input producer error is not lost merely because cancel arrived before the owner
consumed its failed channel. Unsafe callback disposal remains rejected before
changing state; legacy import stop preflight remains unchanged.

Synthetic managed regressions cover frozen main/949/UTF-16 companion replay after
original replacement/deletion, source path/character/codec restoration, chunk
corruption with partial retention, single-use ownership, stale history, exact
aggregate and sparse 4 GiB+1 rejection without copying 4 GiB, all-open admission
cleanup, cancellation and six in-progress capture boundaries (cancel, growth,
same-size/mtime rewrite, replacement, index edit, Stop), active-import Stop and
callback rejection, input-error Stop/Dispose plus independent settings failure,
and deterministic partial-write/flush/adapter-cleanup fault injection with actual
scratch descriptor cleanup. Fault injection does **not** claim a real full-disk
or Windows I/O failure was exercised. All files are disposable synthetic inputs.

This section records the managed backend checkpoint. Bridge/Rust transport,
typed capture/proposal handles and actual isolated-worker coverage were completed
in the checkpoints below; GUI selection remains subsequent. Native Windows
filesystem/disposal behavior, real screen clicks and ACT history/companion native
thread timing remain unverified.

### S1/S2 managed Bridge checkpoint

`HistoryOperation` commands 3–9 use the existing fixed 4 MiB reply / 128 KiB request envelope and session header. They return immediate JSON responses; command 0 remains exclusively the checkpoint query/restore continuation. This was the managed-only checkpoint; the Rust client is described below and GUI remains subsequent.

| Command            | Admission / ownership                                                                                | Result                                                                             |
| ------------------ | ---------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| 3 propose          | Exactly one history selection and explicitly bounded paths/roots                                     | New proposal owner, pending                                                        |
| 4 proposal poll    | Current proposal owner                                                                               | Pending or metadata-only snapshot; no historical byte identity claim               |
| 5 proposal release | Current proposal owner, no captured request                                                          | Cancel/join discovery and retire owner, including faulted tasks                    |
| 6 capture          | Ready proposal owner, selected path, explicit companion/codec options and optional lower byte budget | Distinct capture owner, pending                                                    |
| 7 capture poll     | Current capture owner                                                                                | Progress or captured file lengths/digests and inclusive history range              |
| 8 capture release  | Current capture owner                                                                                | Cancel/join and dispose image, retire owner even when reporting an earlier failure |
| 9 reparse          | Ready capture owner and initialized parser                                                           | One-shot transfer to existing companion import owner/Advance/cancel commands       |

Ready images survive failed import preflight; any consumed image is retired even if admission fails. Shutdown cancels and joins pending source I/O before plugin teardown, collecting release failures while continuing cleanup. A dropped client request alone is not cancellation: the managed session retains one proposal and one capture until explicit release, replay transfer or Stop. The subsequent Rust checkpoint below covers request-future/drop handling and isolated-worker tests.

The in-process ABI regression uses temporary synthetic files to check stale owners, budget failure/pending polling and release, frozen bytes after original replacement, both inclusive range endpoints, and repeated transfer rejection. It exercises the real ABI/session boundary, **not** the isolated worker transport, actual GUI, Windows runtime or native ACT scheduling.

### S1/S2 Rust and isolated-worker checkpoint

The Rust API reuses `Request::History` / `HistoryOperation` commands 3–9 across the existing authenticated worker transport. No second native ABI or import scheduler is introduced. Captured replay is admitted as the existing Host `ImportJob`, using owned companion import continuation commands for progress/cancel and normal shutdown drain.

| Rust object / operation        | Contract                                                                                                                                                                                                                                                                 |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `HistorySourceProposal`        | Private session + proposal owner, selected history revision/ID/range and normalized authorized search scope; poll exposes metadata only                                                                                                                                  |
| `HistorySourceCapture`         | Separate private session + capture owner; poll exposes captured file SHA-256/length/codec and the selected inclusive range                                                                                                                                               |
| Source paths/roots             | Absolute lexical normalization before admission, without filesystem canonicalization; missing files can remain explicit candidates                                                                                                                                       |
| Ready capture replay           | Mutable handle preserves failed preflight; successful transfer retires its owner and cannot replay again                                                                                                                                                                 |
| Proposal/capture release       | Mutable handle preserves Busy/foreign-session pre-dispatch rejection; successful acknowledgement retires it                                                                                                                                                              |
| Faulted capture release        | Managed release retires ownership before reporting the original fault; a subsequent stale-owner reply is expected                                                                                                                                                        |
| Handle or response future drop | Does not automatically cancel or free managed resources. One proposal/capture remains bounded and Host-owned until explicit release, replay transfer or shutdown. A dropped admission response may require Host shutdown because the client did not receive the owner ID |

Capture replies echo the exact selected companion name separately from the physical filename component. Rust verifies selected names/codecs, sibling-directory association, counts, hashes, budgets, envelope/session/owner and history range. ACT/.NET case-insensitive prefix/extension matching stays in the managed catalog, so physical `MAIN.Alpha.LOG` is accepted for selected `main.log` + `ALPHA`; Unicode casing is not approximated by a Rust filename reconstruction.

Actual isolated-worker tests use only temporary synthetic sources/history and cover frozen main/companion replay after replacement/deletion, inclusive bounds, one-shot transfer, legacy import afterward, budget faults/release/reopen, stale history preflight, foreign and restarted sessions, cancel/drop/shutdown, parser failure with partial graph and recovery, and dropped capture-admission responses. Unit tests reject malformed envelopes, unknown/contradictory response shapes, out-of-scope candidates, wrong owners/ranges/path-name associations/codecs and unbounded counters. These tests do not establish native ACT thread timing, Windows UI behavior or a stronger historical/atomic filesystem identity guarantee. The GUI source matching/capture/reparse checkpoint follows below.

### S1/S2 History GUI checkpoint

The existing History results now provide a separate **Find source logs** action, including for already restored checkpoints. Checkpoint restore stays separate from raw-source replay. The source panel uses the existing Rust/UI-derived Button/Input/Label/Badge and Chorale Accordion, existing native encoding selectors and the shared companion codec display; no UI dependency or WinForms bridge is added.

| User operation             | Ownership and state contract                                                                                                                                                                               |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Choose history / scope     | Explicit current row, explicit file and/or one allowed immediate-child directory, bounded filename filter; no ambiguous or single-result auto-selection                                                    |
| Select candidate           | Metadata/hints are labeled as proposals, never historical identity proof; select a main file and explicit companion names/codecs                                                                           |
| Capture                    | Request-owned codec/companion/inclusive checkpoint range and 1–4096 MiB aggregate input-byte budget (default 4 GiB); show copied counters and verified file lengths/SHA-256                                |
| Reparse                    | Separate deliberate button for Ready image; existing ImportJob progress/cancel and partial-result behavior, with action/log/table selection invalidation before and after accepted work                    |
| Change selection / release | Release the current image and proposal before installing a new history selection; selecting a different candidate releases its image; Busy/cleanup failures retain recoverable ownership and expose errors |
| Navigate / cancel / stop   | Application-owned tasks survive panel unmount; navigation/cancel joins input cleanup via explicit release, Stop owns final Host shutdown; stale session/generation responses cannot publish old selections |
| Drafts                     | Scope, budget and codec input survive failure/release; companion drafts are associated with their source path, preserved on cancellation and retained only for names still present on catalog refresh      |

Actual isolated-Host controller tests cover explicit candidate selection, frozen main/companion options, both range endpoints, source replacement after Ready, one-shot replay, graph-reference invalidation, selection/navigation cleanup, duplicate requests, cancellation, late responses and session stop, partial-open failures, stale history preflight with Ready image retention, codec draft preservation and stale A Replay/Release events against a newer B capture. Existing whole/ACT comparisons remain required. These are controller contracts, not native mouse/keyboard/focus QA. Linux startup can be checked in an isolated temporary executable directory; ser7 accessibility/CUA limits and actual Windows/native ACT timing remain unverified.
