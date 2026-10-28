# Hierarchical report contracts and managed task-XML slice

Reference: ACT 3.8.5.288, SHA-256
`934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`.
`OriginalReference report-export-policy` reads the pinned method IL only. It does
not execute native export controls, graph delegates, clipboard, browser, original
file writers or network/database services.

## Existing ACT options: separate XML and HTML contracts

| Route                             | Verified scope and structure                                                                                                                                                                         | Current bounded implementation                                                                              |
| --------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| `IO_XmlFile.ExportXml1` → depth 1 | One selected encounter; `EncounterTable/Row` contains all combatant SQL columns                                                                                                                      | Managed hierarchy XML main table                                                                            |
| `ExportXml2` → depth 2            | Each actor Row additionally contains `CombatantTable`; each category Row contains `DamageTypeTable` with attack-summary Rows                                                                         | Managed hierarchy XML middle depth; it adds two row levels, not just one                                    |
| `ExportXml3` → depth 3            | Each attack Row additionally contains `AttackTypeTable` with action Rows                                                                                                                             | Managed hierarchy XML full depth                                                                            |
| `ThreadExportHTML`                | Checked encounter nodes in tree order, or the single selected encounter; four nested table levels, and four corresponding graph choices                                                              | Managed static multi-encounter HTML and typed Bridge/Rust worker API are connected below; GUI remains       |
| `IO_ExportHtml` depth tree        | Tables and graphs each have Encounter → Combatant → DamageType → AttackType nodes, initially checked. Parent/child checks enforce dependencies; the encounter-table root is required by the exporter | Future explicit table/graph depths must preserve dependencies; no ignored depth/graph option is exposed now |
| HTML graph sizes                  | UI increments 8, range 128..1600 in both axes; defaults X=640, Y=256                                                                                                                                 | Future static renderer must honor exact requested dimensions; no placeholder graph is produced              |

These task-tab XML reports are distinct from both the four current-table context
menu XML routes and ACT encounter archive/interchange XML/gzip. XML task export
has one selected encounter; the HTML checked-encounter set must not be silently
substituted for that XML selection policy.

`IO_XmlFile.ExportXml` IL establishes the four nested roots, depth branches
(027d..027f and 0427..0429), and action first-column override (04f9..0528).
Every table has a `Name` attribute. The root uses `EncounterData.ToString()`;
child captions append actor.Name, category.Type and attack.Type with `|`.
Columns are all SQL names/values in registry order, including hidden columns.
The first action field is replaced by the containing encounter's `EncId` by
**column index zero**, regardless of its registered SQL name.

The model now supplies the original `DurationS`, `ToString()` and `EncId` contracts.
`DurationS` uses TimeSpan Hours/Minutes/Seconds components, not total hours/days.
Unset start time omits date/time; encounters older than 12 hours include the short
date, all dated captions use the current culture's long time. `EncId` caches the
virtual object hash formatted `x8`. It is process-local and unrelated to the
host's session-owned encounter IDs, private archive identity, source-log identity
or historical file provenance; it is not a new durable identifier.

## Managed capture/render

`HostEnvironment.CaptureHierarchyXml(encounterId, depth)` captures the exact
original included parent collections, rows and column bindings, with a separate
hierarchy revision, counts and owned caption/EncId. A missing scope returns null
and retires the previous hierarchy selection. A replacement capture retires old
hierarchy revisions. The existing current-table capture is independent and is
not consumed/replaced. No SQL/display/color callbacks execute during discovery;
virtual encounter caption/hash methods run under the owner/read-only guard and
scope/error/budget checks. The captured caption and EncId remain stable through
rendering; native's repeated caption/hash accesses are not reproduced.

`ReadHierarchyXml(revision)` invokes original SQL callbacks on the original owner
thread and original objects, validates all included scopes around callbacks and
at completion, and returns owned UTF-8 XML only. It rejects wrong owner, stale
revision, parent/row/registry replacement, cross-node mutations, reported errors,
reentry and unsafe Dispose. Callback and post-validation failures retain both
causes. It never creates a file or sends output externally. Model checks are not
a deep immutable freeze of every public action/metric field, do not roll back
plugin side effects and cannot preempt a callback that never returns.

SQL callbacks run at most once per field. With only one action SQL column its
formatter is not called, matching the native first-field shortcut. When later
columns exist, the first formatter runs once (failures remain visible), but its
unused returned value is replaced by EncId without XML/value validation. Native repeatedly obtains whole
`ColCollection` arrays while writing individual fields, which may invoke unused
or repeated formatters. Repeated callback multiplicity/side effects are deliberately
not reproduced. All actually used values and original ordering/structure are
preserved; invalid SQL metadata/UTF-8/XML characters fail explicitly. There is no
implicit row filter, sorter, category selection, HTML/graph fallback or ignored
option in this task-XML API.

| Host-only bound | Behavior                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tables / rows   | At most 1024 included tables and 16384 aggregate rows across all nested levels, including empty child tables and repeated aggregate views                                                                                                                                                                                                                                                                                                                                          |
| SQL values      | Existing 4096 UTF-8 bytes per field and 8 MiB aggregate raw text across all nodes                                                                                                                                                                                                                                                                                                                                                                                                  |
| XML             | Original-style XmlTextWriter, UTF-8 declaration, four spaces, CRLF framing/attribute escaping; serialized output ≤16 MiB including nested markup and escaping                                                                                                                                                                                                                                                                                                                      |
| Time            | Capture checks two seconds; each node's value collection retains its two-second cooperative bound; whole rendering checks four seconds between operations/callbacks                                                                                                                                                                                                                                                                                                                |
| Memory / CPU    | Capture and owned values/output are bounded; whole-topology rescans can cost quadratic work in rows/columns. A synthetic 2000-action/3-SQL-column render completed in about 2.1 s on ser7 in a dedicated run; under aggregate-check load the per-node two-second budget expired explicitly at about 2.0 s and recovery passed. This is not a portable latency guarantee. The representative regression accepts an explicit cooperative timeout under load, then verifies recovery. |

Regression vectors independently construct the same nested framework-writer
operations observed in original IL and compare layout-normalized bytes. Tests
also cover actual empty data, absent scope, independent current-table capture,
first action field override, same-name collection replacement, callback failures,
invalid values/names, owner/reentry/Dispose, counts, limits and cooperative recovery.
This is Linux managed/IL contract evidence, not native ACT export execution.
Bridge/Rust session-typed hierarchy selection and actual isolated-worker tests are
connected below. GUI depth selection and new-file save are connected as described below.

## Remaining existing HTML/graph work, in order

1. Connect the verified multi-encounter static HTML API to explicit selection/level/key/sort
   GUI and safe source/no-overwrite save. Managed capture/render, Bridge/Rust and
   actual isolated-worker synthetic tests are connected below; GUI completion
   must not be inferred from the managed/API tests.
2. Graph calculation contracts: `GenEncounterGraphAdv`, `GenCombatantGraph`,
   `GenDamageTypeGraph` and `GenAttackTypeGraph` use time-series actions, graph
   averaging/rolling options, selected graph categories/actors and registered
   sort keys. The verified solo path includes only positive Dnum amounts, truncates
   second offsets, uses window buckets (+1 offset) or forward rolling accumulation,
   and derives dimensions separately from values. These are not the existing
   contribution bar chart. Finish pinning each default/options/filter/series
   formula and edge case before implementing its bounded numeric model.
3. Static visual graphs: render the verified numeric model with the requested
   dimensions and escaped labels, without scripts/external resources. A static
   generated SVG is a viable Linux path; numeric sample correctness and visual
   axes/layout/font correctness require separate evidence. ACT native GDI+/PNG
   drawing, custom bitmap graph delegates, pixel equivalence and Windows font/DPI
   behavior are not established by a Linux numeric or serializer test.
4. Connect the verified hierarchy/depth/graph options through owner/session APIs
   and the incumbent Rust/UI snapshot/no-overwrite save flow. Reject unsupported
   native/custom graph paths explicitly; never treat an omitted graph as success.

The static/no-external-resource requirement supplies the implementation direction;
no new product decision is required to continue the independent numeric/table
work. Native drawing/plugin-graph execution and Windows visual verification remain
platform boundaries. L7b is not complete; no ODBC, upload, AI or WinForms rendering
bridge implementation is added by this slice. OS clipboard provider approval is
still pending and has not been retried.

## Bridge/Rust hierarchy API

`HostHandle::capture_hierarchy_xml(encounter, depth)` returns an optional opaque
`HierarchyXmlSelection`. It privately binds the originating session-owned encounter,
separate hierarchy revision, exact depth, table count and aggregate row count.
`hierarchy_xml(&selection)` renders once and returns an owned source snapshot with
the same selection. The type cannot be substituted with a current-table selection
or ordinary-text revision. Wrong-session encounters/selections are rejected before
dispatch; changed/missing graph selections and retired revisions fail in the worker.
A missing capture returns None and retires older hierarchy captures. Current-table
and hierarchical captures remain independent.

The existing owner-serialized report transport now has command 3 (capture) and
4 (render), sharing its normal Busy/shutdown/process isolation. Requests use strict
UTF-8 JSON, known fields, recursive duplicate-key checks, exact command shape and
validated enum depth. The request length is bounded by both 256 KiB and its actual
fixed buffer capacity **before** copying input, including the small capture buffer.
All owner/header/version/reserved checks precede owner operations. Caption/hash
methods may run during capture; SQL render callbacks never run as a size probe.

| Reply        | Fixed pre-reserved capacity and validation                                                                                                                                                                                                                                                                       |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Capture      | 4096 bytes (also holds its bounded input JSON); response is presence plus revision/encounter/depth/table/row metadata. Missing is exactly a zero presence flag.                                                                                                                                                  |
| Render       | 16 MiB + 32 bytes; metadata and byte length precede raw complete UTF-8 XML. XML is not re-escaped into JSON. This remains below the existing 128 MiB worker frame bound.                                                                                                                                         |
| Rust receive | Exact command/session/version/reserved envelope, nonzero revision, requested encounter/depth, tables 1..1024, rows ≤16384, tables ≤rows+1 and main-depth tables=1. Render metadata must exactly match the private selection; content must be nonempty, bounded, well-formed UTF-8 without NUL or trailing bytes. |

Errors preserve bounded inner callback causes. No automatic retry, depth downgrade,
partial XML result or successful-empty fallback is added. Cooperative timeouts are
**explicit unsupported-workload outcomes**, not evidence of normal large-report
support. A returning callback's timed-out request may be followed by an explicit
new request; a callback that never returns still needs the existing worker shutdown
boundary. Counts admitted by capture do not guarantee completion within the
per-node two-second and whole-render four-second budgets; quadratic validation
cost documented above remains. No timeout limit is silently extended.

Synthetic actual isolated-worker tests cover all three depths, SQL order/hidden
columns versus registration labels/display callbacks, Unicode/CRLF caption,
per-encounter EncId override, repeat rendering, independence from current-table
capture, retired revisions, removed/missing encounters, old handles and restart.
Separate cases cover SQL/first-action callback failures, malformed Unicode/cell
limits, one returning 2.1-second callback with explicit same-selection recovery,
and escaped XML exceeding 16 MiB followed by an explicit shallow capture/render.
Rust malformed-response tests include valid control cases, unknown identity/depth,
count/budget violations, exact 16 MiB byte transport, invalid UTF-8/NUL/trailing
bytes, and command-specific envelopes. ABI tests check invalid requests before
owner callbacks, including capture input exceeding 4096 bytes.

The worker tests use temporary synthetic log files and owned plugin objects only.
They do not execute native ACT exporters, clipboard, browser or Windows UI. GUI
depth selection and explicit new-file save are connected; multi-encounter
hierarchical HTML and graph calculation/static rendering remain separate L7b work.

## Hierarchy XML GUI and new-file save

The existing Rust/UI-adapted Dioxus Button controls select the three ACT depths
for the explicitly selected encounter. This editor is separate from current-table
XML/HTML and encounter archive interchange. Main is the default; summaries add
category and attack-summary tables (two row levels), and all adds action tables.
There is no implicit row filter, display-column selection or current-table sort.

Generate admits one encounter/depth snapshot, then captures and renders once on
the existing serialized Host report path. Depth edits cannot enter a pending
request. Completed preview metadata describes the returned depth, tables and
rows, not mutable draft settings. Depth edits retire preview/save confirmations;
encounter selection, session replacement, navigation and cancellation retire late
results. Cancel discards the result without preempting a plugin callback; pending
admission remains held until its response. Draft depth survives failure/Stop.
Time/output/callback failures remain explicit errors with no partial XML or
automatic retry/shallow fallback.

The shared preview renders source as escaped text (64 KiB UTF-8-safe preview
limit), never executable markup. Save uses the complete owned UTF-8 source, an
explicit path and confirmation, same-directory staging and atomic no-clobber
publication. Existing destinations are not overwritten. Failed saves retain the
generated source. Cancellation before publication prevents a new file; already
admitted publication can finish after Stop and late UI replies are ignored.
No archive, clipboard, browser, external resource or upload operation occurs.

Three actual isolated-Host controller regressions cover the three depths and
exact saved bytes, Unicode, duplicate requests, stale edits/confirmation,
selection/navigation/session retirement, cancellation before save, missing-parent
and existing-file errors, explicit returning-callback timeout and serialized
16 MiB failure followed by an explicit shallow request. The existing shared
source-preview truncation and file-write/flush/publication regressions remain in
force. Synthetic temporary logs/files only; native clicks, Windows UI and native
ACT exporter execution are not established by these checks. Hierarchical HTML
and graph work remains separate and unfinished.

## Managed static HTML table hierarchy

`CaptureHierarchyHtml(encounterIds, lastLevel)` accepts 1..64 distinct explicit
encounter IDs and one of four table row levels: Combatant, Category, Attack or
Action. The mandatory root displays combatants for each selected encounter;
deeper choices include every preceding level. Selected encounters are captured
in existing zone/encounter tree order, ignoring request/click order, and missing
IDs return null while retiring the previous HTML selection. HTML, hierarchy XML
and current-table revisions are independent. No unbounded search is added.

`ReadHierarchyHtml(revision, levels)` requires exactly one explicit setting for
each included level, with 1..32 distinct registered display keys per level. Key
strings are headings, distinct from labels and SQL names. Original display
callbacks run once per used field on the original owner thread/objects; SQL,
color, graph and EncId-override callbacks do not run. Combatant, attack-summary
and action tables accept explicit registered sort keys/direction using the
existing guarded comparer. ACT's category exporter copies its source collection
without sorting, so a category sort is rejected. Child sections retain source
order independently of each table's row sort. Unknown keys/sorters and invalid
level sets fail before display callbacks, including levels where no child table
exists. Their registry revisions remain validated until completion.

Original IL evidence is pinned in `OriginalReference report-export-policy`:
`ThreadExportHTML` scans checked zone/encounter nodes, generates four identifiers
(ed/cd/dt/at) and calls the four `ExportTBodyHTML` overloads. The actor, attack and
action overloads sort/reverse; the category overload does not. Checked column
keys feed `GetColumnByName`, not SQL formatters. This is code/contract evidence,
not native Windows exporter execution or byte-identical native HTML.

The owned output is one UTF-8 static document with nested sections, plain tables,
escaped headings/keys/values and CSP denying resources, forms and base URLs.
It has no scripts, stylesheets, images, external URLs or executable data. Unlike
native hidden divs, JavaScript navigation/client sorting, PNG graphs and resource
files, every included table is visible. Captions use captured encounter
`ToString()` plus the actor/category/attack path, rather than copying native
heading markup/date layout. Explicit per-level settings replace native global
WinForms checked lists and sort controls; there is no silently inherited setting
or ignored graph option. Native appearance and plugin callback side-effect
multiplicity are not claimed compatible.

Bounds are shared across the selected set: 1024 tables, 16384 aggregate rows,
4096 UTF-8 bytes per field, 8 MiB raw display values and 16 MiB serialized source.
Capture has a two-second cooperative budget; render retains two seconds per
node and four seconds overall. These are explicit rejection/error budgets,
not promised large-workload capacity or preemption of a nonreturning callback.
A caller must explicitly request a smaller depth or retry after a returning
failure; no fallback, partial-success source or automatic callback retry occurs.

During review, virtual `DamageTypeData.Type`/`ToString()` exposed a shared XML
name-validation gap. Both HTML and XML now activate read-only guarding and
owner-error baselines before initial topology/name capture and before render's
first validation. Name callbacks receive budget/error checks and structural
post-validation without recursively invoking more name callbacks. A throwing
name callback plus structural mutation retains both causes. Public model fields
are still not an immutable freeze or rollback boundary. Repeated full-topology
checks remain a cost; duplicate per-name whole scans were removed without
extending time limits.

Synthetic managed regressions cover all four depths, tree-order multi-selection,
explicit hidden/key order versus labels/SQL, sorted rows versus child order,
caller-array mutation, stale/missing/replaced selections, independent XML/current
reports, empty parents, empty-level registry changes, owner/reentry/Dispose,
virtual names in capture and initial render (including reported errors and body
plus structural errors), callback/sort failures, escaping, cell/raw/output limits,
returning timeout and explicit recovery. No files, clipboard or resources are
exported by these methods/tests. Bridge/Rust and actual isolated-worker HTML tests are connected below;
GUI/save remains a subsequent slice. Graph numeric models, SVG/other static
rendering, native custom bitmap delegates and Windows visual parity remain
unimplemented; do not infer their completion from table output.

After the name-guard changes, the dedicated existing XML suite passed; its
2000-action/three-column representative returned the explicit per-node timeout
in about 2.024 seconds, then shallow recovery passed. A pre-final-edit whole
check also passed in 65.50 seconds. These observations do not establish normal
large-report support; final hook-after checks are recorded in the task checkpoint.

## Bridge/Rust static HTML hierarchy API

`capture_hierarchy_html(&encounters, last_level)` returns an optional opaque
`HierarchyHtmlSelection` binding this worker session, HTML revision, requested
level, original tree-order encounter IDs and table/aggregate-row counts. Request
order is not presumed to be tree order: capture validates exact set equality,
nonzero/distinct IDs and counts. `hierarchy_html(&selection, &levels)` requires
exactly one explicit setting per included level; each holds 1..32 distinct
registration keys and optional sorting (never for Category). The returned source
metadata must exactly match the captured revision, depth, counts **and order**.
New/missing captures retire older HTML revisions; XML and current-table captures
remain independent. Wrong-session IDs/selections are rejected before dispatch.

The existing owner-serialized report transport adds command 5 (capture, fixed
4096-byte body capacity) and command 6 (render, fixed 16 MiB + 1024 bytes). The
maximum actual render metadata is 540 bytes including the UTF-8 length and 64
IDs; source remains at most 16 MiB and is never JSON re-escaped. Requests remain
bounded by 256 KiB strict UTF-8 JSON and their command's fixed capacity before
copying. Individual option maxima do not promise that every maximum/escaped-key
combination fits the total request budget. Unknown/duplicate fields, command
shape, nonzero owner, enum/level/key/count rules and reserved fields are checked.
All capture/render operations execute once through `PluginSession.OnOwner`.
There is no capacity probe, automatic callback retry or silent shallow fallback.

Rust validates exact envelope/session/command, nonzero revision, IDs 1..64,
tables between encounter count and 1024, aggregate rows <=16384, tables <= rows

- encounter count, and exactly one table per encounter at main depth. It accepts
  original tree reordering only on capture; reordered render IDs are rejected.
  Source must be nonempty, strict UTF-8, NUL-free, without trailing bytes and within
  16 MiB. This is typed transport validation; it is not a sanitizer for arbitrary
  untrusted HTML. The managed static serializer supplies escaping/CSP.

Three actual isolated-worker regressions use the synthetic hierarchy plugin:
all four depths, multi-encounter tree order, key/label distinction, explicit
descending display sort, Unicode/CSP/escaping, repeat, independent XML/current
captures, retired revisions, removed/missing IDs, Stop and restart; callback,
malformed-Unicode, cell/sort and returning-timeout failures with explicit recovery;
64 small encounters and rejection above 64; and actual serialized 16 MiB failure
using eight synthetic lines/32 explicitly selected expanded display columns,
followed by shallow recovery. The original XML-mode fixture remains unchanged
by the derived HTML mode and its worker regressions remain in the same suite.
ABI tests reject malformed inputs before owner operations; Rust unit/envelope
tests cover malformed set/order/depth/count/content, exact 16 MiB transport,
UTF-8/NUL/trailing data, options and command-specific capacities.

An earlier 200-line/two-column output fixture hit the cooperative four-second
render budget before output size; it was replaced by the independent small-row
expansion vector rather than extending time limits or accepting timeout as a
size-bound pass. Counts and 64 small encounters passing do not establish normal
large-report support. Existing quadratic topology validation and nonreturning
callback isolation limits still apply. This is synthetic Linux worker/IL evidence,
not native ACT HTML export, browser rendering, Windows UI or graph equivalence.
No source, clipboard, file export or network action is performed by these APIs.
