# Owned current-table reports: first managed slice

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

This checkpoint is managed-only. Bridge/Rust typed session/revision binding,
actual isolated-worker report tests, GUI/table selection and no-overwrite file
save are subsequent slices. Full HTML encounter/zone hierarchy, checked encounter
sets, depth options and graph/image dimensions remain in the finite L7b plan.
Archive XML/gzip and ordinary text reports retain their existing separate paths.
No clipboard provider, upload, real database or user data was accessed.
