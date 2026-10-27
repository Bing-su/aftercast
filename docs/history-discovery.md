# Checkpoint discovery and explicit selection

The managed history service provides metadata browsing and explicit checkpoint
selection, reusing private archive restoration and action/source identities.
Bridge/Rust transport now uses the shared asynchronous host lifecycle; the History GUI is not yet connected. This is distinct
from ACT's `ThreadHistoryImport`, which locates source logs and reparses a time range.

| Managed operation                  | Contract                                                                                                                                                                                                                                                                                                                                            |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ReadHistory` / `ReadHistoryAsync` | Owned metadata pages of 1..16 rows. Query title, zone, character or their combined fields with a case-insensitive substring. Character scope is an aftercast extension.                                                                                                                                                                             |
| Date search                        | Encounter **start** is within `[StartTicks, EndTicks)`, matching ACT's date/time search. This is not overlap search. Values are DateTime ticks after existing binary DateTime interpretation; time kinds remain explicit and no UTC conversion is applied by this query.                                                                            |
| Snapshot                           | SHA-256 of exactly the index bytes read from one descriptor. Parsing, sorting and IDs refer to those same bytes. Byte-identical index contents share a revision, including after replacement.                                                                                                                                                       |
| Stable ordering                    | Start ticks descending, then archive filename ordinal. Equal-time encounters remain distinct. Offset/limit page this ordered snapshot.                                                                                                                                                                                                              |
| Missing/empty/error                | A missing index is an empty snapshot with `IndexPresent=false`. A valid empty Records document is present with zero rows. Filters can yield zero results while IndexedCount remains positive. Malformed/unreadable existing indexes fail visibly.                                                                                                   |
| Metadata effects                   | Query does not add public ACT history records, intern every queried string, restore graph data or rewrite files. AlreadyRestored describes the current graph snapshot.                                                                                                                                                                              |
| Presence                           | PayloadPresent is a basic file-existence hint. It does not verify readability, payload integrity or availability of the original source logs. Integrity is checked during restoration of pending payloads.                                                                                                                                          |
| Selection                          | Expected index revision and 1..64 distinct content-addressed sibling IDs. Copy the caller's array before asynchronous work. Unknown IDs, duplicates, traversal and stale revision fail before attachment.                                                                                                                                           |
| Writer coordination                | Hold the existing checkpoint.lock lease from catalog validation through payload verification, decoding and graph attachment. Cooperating aftercast checkpoint writers cannot replace the catalog mid-restore. A final revision check detects observed out-of-band index changes; arbitrary external tools do not participate in the lease protocol. |
| Atomic graph publication           | Hash every pending payload, expand once, count the same XML bytes decoded and prepare every selected encounter before attachment. Corrupt, missing or over-budget pending entries attach none from that call; preserve the live sorter on failure. Already restored payloads are skipped.                                                           |
| Resource bounds                    | Index and individual compressed/expanded files remain at most 64 MiB; catalog at most 100,000 records. Selected restoration accepts at most 128 MiB **total expanded XML input**, not an exact managed-heap bound. Nested gzip is not XML and is rejected.                                                                                          |

Read metadata without initializing parsers. Reconstruction still requires the same
parser category registrations; Bridge must enforce its existing initialized-plugin
admission. The managed async service uses the original owner continuation context,
quiesces log/action processing for restoration and performs disk I/O off the owner.
Shutdown must await accepted work through the existing host operation lifecycle;
no cancellation contract is added by these managed methods.

Private payloads retain precise values and encounter-scoped swing/log sorters.
Selected restoration reuses the existing decoder and duplicate tracking; metadata
selection does not reconstruct identity from timestamps. A removed index also clears
the private candidate cache used by the legacy combined-filter restore path.
The service does not delete checkpoints, locate raw log ranges or change automatic
history-saving/retention policy.

Synthetic temporary-data tests cover same-time records, scopes and date boundaries,
missing/filtered results, read-only discovery, stale/corrupt indexes, missing/corrupt
selected payloads, traversal/selection limits, writer lease, async caller ownership,
repeat restore and private action/source identity. Regression tests reject nested
gzip and use four 33 MiB ignored XML comments to exercise the actual aggregate
128 MiB policy while retaining only tiny encounter graphs.

## Rust transport and lifecycle

| API                                         | Result                                                                                                                                                                                                    |
| ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `query_history(&HistoryQuery)`              | An owned page; default query uses 16 rows. Revision and every ID belong to both the worker session and exact index snapshot. Time kinds are explicit.                                                     |
| `restore_selected_history(&revision, &ids)` | Newly restored count. Reject mixed snapshots, old sessions, duplicate IDs and selection counts outside 1..64 before sending work. The managed service rechecks the actual catalog under its writer lease. |
| Begin/Advance                               | Fixed-capacity packets are validated before admission. One pending history or archive operation occupies the host slot; diagnostics/combat reads remain available and conflicting mutations return Busy.  |
| Shutdown                                    | Completion is consumed through the common poll/drain loop. Stop cannot dispose the managed session while a history response is pending. Dropping a future does not promise cancellation of accepted work. |

Synthetic ABI and direct Begin/Advance tests cover cold discovery without plugins,
shared admission, pending-response Stop rejection and parser-required restoration.
Worker integration covers equal-time entries, pagination, scopes/date bounds,
snapshot/session rejection and restored action/log identity. A FIFO diagnostics
barrier confirms admission before abandoning a query future and shutting down;
restart verifies release of the worker lease. This last case does not force a
long-running restore at the moment of shutdown; exact pending Stop rejection is
covered by the managed lifecycle test and shared operation implementation.

## History GUI

The History workspace uses the existing Rust/UI Accordion, Button, Input, Label
and Badge components. Search drafts remain separate from the applied result query;
paging keeps selection only within the same snapshot. Dates are inclusive civil
days, exact times are exclusive at the end, and invalid precision/timezone/leap-second
inputs are rejected without changing the draft.

| Transition         | Controller contract                                                                                                                                                 |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Query cancellation | Ignore the accepted read's late success/error, but keep the operation busy until it finishes.                                                                       |
| Confirmation       | Freeze snapshot IDs; cancel/reopen and selection changes invalidate old confirmation buttons.                                                                       |
| Restore            | Require an initialized parser; invalidate Analysis and independent log-window references both before and after the attempt, including failures.                     |
| Navigation/stop    | App-owned work survives pane navigation; session/query tokens reject responses after session reset. Accepted restore work follows the Host shutdown drain contract. |
| Failed restore     | Keep the error, require refreshed metadata and explicit selection/confirmation; never implicitly retry restoration.                                                 |

`mise run test:gui-state` exercises a real isolated Host with 20 same-time synthetic
checkpoints across restart, pages, search changes/cancellation, stale catalog and
corrupt payload failures, selected restore and retained action/log identity. It also
checks late responses after session reset. This is controller coverage, not native
window input or a forced mid-restore shutdown test. The Windows desktop scenario
covers search, invalid dates, confirmation cancel, navigation and selected restore;
Windows clicks remain unexecuted on ser7.

## Source-log reparse follow-up

This remaining ACT parity work is feasible on Linux using synthetic files and the
existing parser/import progress/cancellation infrastructure. It needs an explicit
source path, ACT-compatible range boundaries and source context, with no live timer
or audio effects during import. ACT `ThreadHistoryImport` supplies each selected
record's StartTime/EndTime to `ImportFile`; it does not restore saved aggregates.
Folder/character hints alone do not prove which raw file is correct. Missing or
ambiguous files should require an explicit source choice rather than silent guessing.
Implement and test this as a separate unit after the discovery/selection GUI;
no new native WinForms renderer is required for core range-import contracts.
