# Owned current-table reports: managed core and worker API

## Original ACT evidence and scope

Pinned ACT 3.8.5.288 has SHA-256
`934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`.
`OriginalReference report-export-policy` reads its method IL without invoking
native UI, clipboard, original file exporters, browser, database or network.

| Original route                                        | Verified contract                                                                                                                   | Current managed mapping                                                                                    |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `cmiEDExportXML_Click`                                | `EncounterTable`, `Name` caption, `Row` children containing combatant SQL fields                                                    | `TableLevel.Combatant`                                                                                     |
| `cmiCDExportXML_Click`                                | `CombatantTable`, category SQL fields                                                                                               | `TableLevel.Category`                                                                                      |
| `cmiMDExportXML_Click`                                | `DamageTypeTable`, attack SQL fields                                                                                                | `TableLevel.Attack`                                                                                        |
| `cmiATExportXML_Click`                                | `AttackTypeTable`, action SQL fields                                                                                                | `TableLevel.Action`                                                                                        |
| All four current-table XML routes                     | More than one selected index restricts rows; zero/one retains the full current table; four-space indentation                        | Snapshot row indices, optional explicit sort, same XML selection rule                                      |
| `CombatantData.ColHeaderCollection` / `ColCollection` | All registered `SqlDataName` values and `GetSqlData` callbacks in registry order                                                    | Includes hidden columns; never substitutes label, registration key, cell formatter or SQL literal escaping |
| `ExportTBodyHTML`                                     | Checked column registration names become headings and `GetColumnByName` dispatches display values; sort/reverse apply to typed rows | Explicit selected column keys and cell callbacks; requested bounded sort                                   |
| `ThreadExportHTML`                                    | Checked encounters or a selected encounter, hierarchy options, JavaScript/CSS resources and graph images                            | Full hierarchical export, navigation and graph/image options remain separate and unimplemented             |

This XML is not the encounter archive/interchange format. An encounter-list/zone
scope is not silently assigned one of the four table roots. This initial static
HTML supports the same four current-table scopes, exports the captured scope, and
explicitly rejects nonempty XML row-selection input. It does not implement the
full HTML task-tab encounter selection or hierarchy controls.

## Host contract

`HostEnvironment.ReadTableReportCatalog(scope)` returns caller-owned columns and
row descriptors, with a revision and source indices. It returns null for an absent
scope. Each successful capture owns original row/parent references and the column
revision. There is one bounded current capture per Host: another capture retires
all earlier indices, including an attempt to capture an absent scope. Indices are
not durable action IDs, source-log links or indices into a subsequently sorted
result. A stale revision, same-name row replacement, moved selection or replaced
empty parent is rejected before rendering rather than rebound.

`ReadTableReport(revision, options)` renders an owned string once. XML accepts all
registered SQL columns in registry order; supplying a display-column filter is an
error. Static HTML requires explicit registration keys in output order. Duplicate
selection indices/keys and unsupported formats/scopes fail explicitly. Optional
sort uses existing original-object comparers and bounded comparison validation;
without a sort, captured source order is preserved. This is not an implicit copy
of the native current ListView order or persisted ACT global sort preferences;
the later GUI/Bridge stage must send its explicit display sort.
Callback, inconsistent-comparison or budget failures abort with their cause, never a
success-looking fallback order. XML filtering is applied to the owned source
indices after sorting.

Callbacks run on the Host owner thread and original ACT objects under the existing
read-only table/combat guard. Registry delegates, parent collections, selected
encounter topology and row references/keys are checked around callbacks and at
completion. Callback errors and post-validation errors are retained together;
`WriteExceptionLog` on the owner thread cannot masquerade as a successful report.
Reentry, mutation and unsafe Dispose are rejected, and the guard is released on
failure. Checks are not a deep immutable copy of every metric/action field and do
not roll back arbitrary plugin side effects. A callback that never returns still
requires the existing worker/process shutdown boundary.

## Output safety and explicit differences

| Boundary          | Contract                                                                                                                                                                                                                                                                                                                                                                 |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Input             | Four current-table levels; up to 16384 source rows; existing column metadata bounds; HTML 1..32 selected columns; caption at most 1024 UTF-8 bytes                                                                                                                                                                                                                       |
| Work              | Cooperative two-second budget between operations/callbacks; existing bounded comparison policy; cannot preempt a native or synchronous plugin hang                                                                                                                                                                                                                       |
| Values            | Nonnull well-formed UTF-8, no NUL, at most 4096 bytes per SQL/cell value and 8 MiB aggregate raw values                                                                                                                                                                                                                                                                  |
| Serialized output | At most 16 MiB UTF-8, enforced while the output stream is written, including escaping/markup expansion; no files are created by rendering                                                                                                                                                                                                                                |
| XML               | UTF-8 declaration, four-space indentation, deterministic CRLF layout, original caption/SQL field values escaped by the original-style XmlTextWriter after explicit XML-character validation. SQL names must be valid XML NCNames; invalid/unbound-prefix names fail instead of being rewritten or producing malformed XML. Duplicate SQL names remain repeated elements. |
| HTML              | Standalone UTF-8 document, escaped caption/title/registration-key headings/cell text, restrictive CSP. No scripts, inline events, external styles/resources, image paths, links or user-provided markup are emitted.                                                                                                                                                     |

ACT's original HTML uses script navigation and resource files and formats display
strings into markup. This first serializer deliberately emits escaped static data
with CSP and no resource execution. It preserves selected table values/column order
and sorting, not the original interactive document layout, fonts, navigation or
graph rendering. Those unsupported options are not accepted and ignored. XML uses
SqlDataName, not sanitized display labels; an invalid XML name is an explicit error.
The existing ACT public APIs and their registries are not changed by these host-only
request/response limits.

## Verification and remaining stages

Synthetic managed fixtures compare XML (including caption and SQL CR/LF/tab semantics) against the same framework writer calls
observed in the original IL (normalizing platform layout newline), parse roots and
values, distinguish SQL/display/hidden columns, test source-index selection and
sort, and exercise escaping, Unicode, empty/missing scopes, stale/caller-mutated
catalogs, row/parent replacement, callback/reentry/Dispose/reported errors and
output expansion limits. The original IL probe separately pins the four roots,
selection threshold, SQL getter use and HTML table/resource structure. No native
ACT exporter or browser is executed on Linux; this is contract evidence, not a
claim of Windows execution or byte-identical full HTML output.

The managed core and Bridge/Rust typed session/revision binding are connected,
with actual isolated-worker synthetic report tests. The current-table GUI and explicit no-overwrite file save are connected below. Full HTML encounter/zone hierarchy, checked encounter
sets, depth options and graph/image dimensions remain in the finite L7b plan.
Archive XML/gzip and ordinary text reports retain their existing separate paths.
No clipboard provider, upload, real database or user data was accessed.

## Bridge/Rust transport (current API slice)

`HostHandle::table_report_catalog(&scope)` captures one of the four table scopes.
Its opaque `TableReportSelection` binds the originating session, report-only
revision, scope and original row count. It cannot be substituted with a table
query or ordinary-text-report revision. Catalog descriptors use `TableRow` with
empty cells: discovery invokes no value/color callback. Caller edits to the public
row/column vectors do not alter the private selection or the worker's capture.
`table_report(&selection, &options)` renders once; it does not retry a revision or
probe response capacity by rerunning callbacks. Stop/restart rejects older
selection/session IDs, and a newer capture (including absent-scope discovery)
retires the earlier worker capture.

The Bridge command uses strict UTF-8 JSON requests (256 KiB maximum, unique keys,
known fields, validated owner/header/command/reserved fields before dispatch) and
bounded binary responses. Catalog responses reserve 56 MiB before capture: this
covers 16384 descriptors with three 1024-byte keys plus row fields and 256 columns
with four bounded strings. Render responses reserve 16 MiB + 20 bytes: revision,
format, row count, UTF-8 byte length and the unescaped original content. Thus XML
entities and Unicode are not escaped a second time into JSON, and a maximum-size
report fits below the existing 128 MiB worker frame bound. Serialization still
enforces the managed 16 MiB output limit. Header/length, scope, revision, row
ordinals/descriptors, column flags/keys, format, output count, UTF-8, NUL and
trailing bytes are checked on Rust receipt. A malformed reply is an error, not a
new selection. Bounded inner callback causes (including sort failures) survive the
Bridge diagnostic boundary; errors never become a success-looking empty report.

Synthetic isolated-worker coverage uses original owner-thread plugin objects,
all four roots, hidden SQL columns versus display callbacks, caption CRLF/Unicode,
zero/one/multiple XML selection, explicit sort and failure, escaped static HTML,
callback counts, registry changes, invalid values/limits, recapture, Stop and
restart. These are Linux worker/contract tests, not native ACT exporter execution.
The current-table GUI and explicit no-overwrite save are connected below; the full
HTML hierarchy/depth/graph tasks remain unfinished. No clipboard provider or external
resource execution is added.

## Current-table GUI and explicit new-file save

Analysis now shares one Reports panel for ordinary text and current-table reports.
The incumbent Rust/UI-adapted Accordion, Button, Input, Label and TextArea remain
the controls, with Dioxus owning the controller. The four row levels can be chosen
explicitly with exact combatant/category/attack keys, or copied from the existing
plugin-table browser (including its action drill-down). The report scope is an
explicit draft; subsequent analysis-table navigation does not silently retarget
it. Selecting another encounter/session invalidates the old result. Report sort
keys/direction are explicit and independent of the analysis table's draft sort.

Capture discovers the latest original row-index catalog. XML selections are
paged in groups of 16, retain the zero/one/all and multiple/subset ACT rule, and
are cleared on recapture because indices are not durable identities. XML always
uses all SQL columns. HTML exports the whole scope and uses 1..32 exact display
registration keys in explicit output order; its help states that XML row choices
are separate. Explicit empty/missing column choices remain in the draft rather
than silently falling back. Scope changes require a new capture. Missing scopes,
unknown keys, sort/callback/encoding/size failures retain drafts and show errors.
Drafts are session UI intent, not a new persisted report-preset format.

A successful preview owns the report, scope, options and encounter title, separate
from the draft. Editing either report draft clears the previous preview and save
confirmation. Duplicate requests are blocked; accepted tasks outlive component
navigation while their session/selection/epoch token determines whether replies
may be displayed. Cancel retires a result without claiming to interrupt plugin
code; admission remains held until completion. Leaving, selection changes and
Stop reject stale replies and confirmations using the shared text-report path.
Large immutable snapshots/catalogs are shared, and preview is at most 64 KiB on
a UTF-8 character boundary. XML/HTML is shown only as Dioxus text inside `pre`;
no HTML injection, browser, script/resource execution or upload is introduced.
The complete generated UTF-8 source, including original CRLF, is saved.

Saving reuses the ordinary-report same-directory temporary file + flush/sync +
no-clobber publication. Only an explicitly entered and confirmed destination is
used. Existing/concurrently-created files are never replaced, directories are not
created, and failure preserves the preview/path. Cancel/leave/Stop may prevent
publication before its atomic admission; already-admitted publication can finish
after Stop and is not undone. No stronger OS-crash/disk-failure guarantee is added.

Four actual isolated-Host controller regressions cover the four XML roots and
saved bytes, original-index subset values, sort and HTML column order, format
switches, text/table switching, safe source/UTF-8 preview limit versus full save,
no-overwrite/failure/cancel, recapture, stale confirmations/replies, selection,
navigation, Stop, empty scopes and SQL callback errors. Existing ordinary-text
controller and file failure/race regressions remain in the suite. Native
click/focus/visual verification and Windows execution remain unverified on ser7;
launch smoke is not a substitute. Full hierarchical HTML, depth and graphs remain
unfinished L7b work; archive XML/gzip is still a separate UI/API.
