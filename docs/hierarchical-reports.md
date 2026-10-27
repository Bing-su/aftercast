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
| `ThreadExportHTML`                | Checked encounter nodes in tree order, or the single selected encounter; four nested table levels, and four corresponding graph choices                                                              | Current-table static HTML is connected; the multi-encounter hierarchical HTML path remains to implement     |
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
Bridge/Rust session-typed hierarchy selection, worker tests, GUI depth selection
and new-file save remain subsequent slices.

## Remaining existing HTML/graph work, in order

1. Multi-encounter HTML table hierarchy: explicit selected set in captured tree
   order, four table levels, per-level registered display keys and explicit sort.
   Reuse the guarded SQL/display row core and owned topology. Emit escaped static
   tables/anchors; do not copy ACT JavaScript navigation/resources or execute
   report content. Add synthetic multi-encounter/depth/order/stale/error vectors.
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
