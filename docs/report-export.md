# Report export: finite L7b/L7c slices

## Verified formats and selected scope

The reference is the pinned ACT 3.8.5.288 artifact with SHA-256
`934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`.
The new `OriginalReference report-export-policy` probe reads actual method IL;
it executes no form, clipboard, original file exporter or database operation.
`TextExportProbe` supplies the existing hard-coded formatting vectors. Linux runs
those vectors against aftercast; the native original text-export comparison is
still the separately configured Windows step of `test:text-exports`.

| ACT route / evidence                                                                                               | Contract / finite implementation slice                                                                                                                                                                                                                                                                                                            |
| ------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| GetTextExport(Encounter, ExportFormatting) forwards null font / default max-lines; core uses 256 when MaxLines < 1 | Ordinary text without font-based column alignment. A selected encounter, player/group templates, registered sort key, ShowOnlyAllies and ShowAlliedInfo are distinct choices. Unknown actor is excluded; missing variables retain the configured caps and discard the extra suffix. The current shared core preserves these established defaults. |
| Core Sort then Reverse, selective GetIsSelective + GetIgnoreEnemies bypass, GetAllies, LF Split                    | Descending ACT sort/fallback, allies semantics and trailing empty LF entries are not CSV/HTML or an actor-display filter. Current default keeps at most 256 LF-delimited entries.                                                                                                                                                                 |
| cmiEDExportXML_Click constructs EncounterTable / Row and column-name elements                                      | Table XML is not ACT encounter-interchange XML. The current-table selection/cell headings and the task-tab hierarchical data-depth options need their own serializers and acceptance vectors. Existing L7a interchange must not be relabeled as table XML.                                                                                        |
| ThreadExportHTML reads checked encounters or one selected encounter, tvHTMLExport choices, nudExportGraphX/Y       | HTML table/graph hierarchy and image dimensions are separate options. Reuse plugin table values; implement explicit supported depth/scope and escaping before output. Current absence of native graph/image rendering cannot be disguised with a placeholder or silent ignored option.                                                            |
| SQL names/types on registered columns                                                                              | These are metadata, not an ODBC writer. L7c offline schema/value/validation/sink contract follows L7b; real drivers, credentials, connections and writes remain an external authorization boundary.                                                                                                                                               |

Original text export also has native font/tabulation, RTF and additional output /
selective-export preference branches. This first bounded slice does not add those
missing compatibility overloads or claim their options are implemented. The catalog now copies the actual built-in default and registered
`TextExportFormats` in registration order, with selection copied into explicit GUI
drafts. L9d output preferences remain separate. Do not invent presets from column labels.

## Local text profiles

The text-report panel can save up to 32 explicitly named local profiles in the
app's existing settings file. Each copies the current player and allied templates,
sort registration key, and ally flags; it does not retain encounter IDs, plugin
objects, catalog revisions, or output destinations. An old settings file starts
with no local profiles. Applying one only changes the editor draft. Saving an
existing name explicitly replaces it; failed settings writes leave the prior file
and editor draft intact. These local profiles are separate from ACT/plugin preset
registration and are not automatically transmitted or copied to the clipboard.

Generation still uses the current session's catalog and existing managed ACT
formatting path. A saved nonempty sort key missing from that catalog is reported
instead of silently applying the original fallback sort; this is an intentional
safety difference for restored profiles. Unknown format variables retain the
existing ACT-compatible formatter behavior. Native ACT comparison remains the
separately configured Windows test; no external destination is accessed.

## First managed text-report slice

`HostEnvironment.ReadTextReportCatalog()` returns a caller-owned catalog of
player/group variable keys and sortable **registration keys**, with a revision.
`ReadTextReport(encounterId, revision, TextReportOptions)` returns an owned text
result with the original encounter ID/revision, or null if that exact selection
is absent. No file is created by this API. It does not automatically select a
replacement encounter or conflate a registration key with its label/SQL name.

The public ACT `GetTextExport` API retains its best-effort behavior, including
caught formatter/sort errors, the empty-output LF and missing-sort fallback.
The report API shares its formatting/framing core but reports failures explicitly
and uses these aftercast request boundaries (not original ACT limits):

| Boundary            | Meaning                                                                                                                                                                                                                                                                                                                                                                                                                      |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Templates           | Each at most 16 KiB of well-formed UTF-8; immutable options are copied by value. Sort key is at most 256 UTF-8 bytes without control characters. No file/clipboard encoding guess is involved.                                                                                                                                                                                                                               |
| Registries/parser   | At most 1024 variables per player/group registry, variable keys at most 256 UTF-8 bytes, parser pattern at most 16 KiB and combined missing-variable caps at most 1024 bytes. Catalog revision changes on dictionary/definition/delegate/parser/cap/sort-column changes. Public ACT registries remain unchanged by these host-only validation rules.                                                                         |
| Scope               | At most 16384 zone/encounter slots scanned and 16384 actors. Original zone/encounter/actor collection references, slot/count, actor key+reference, selected character/title/zone and actual used allies membership are checked around callbacks and at completion. Same-name clones do not count as the original actor. Selective IgnoreEnemies uses all actors as in ACT.                                                   |
| Work/output         | 1 MiB UTF-8 aggregate rendering budget including final LF, checked before appending each callback value; this is a work budget before the normal 256-LF framing, so oversized intermediate rendering fails even if a later line cap would shorten it. Regex matches use a 100 ms timeout; a cooperative two-second budget is checked between operations/callbacks.                                                           |
| Callback lifetime   | Run original delegates on the original owner thread and objects under the existing read-only table/combat guard. Reject reentrant report/table/Root mutation and unsafe Dispose. Registry and selected-scope changes cause no report to be returned; body and post-validation errors are retained together. Owner-thread WriteExceptionLog cannot become a success-looking report. The guard is released on failure/timeout. |
| Limits of isolation | A synchronous native/plugin callback that never returns cannot be interrupted by these cooperative checks; a plugin may allocate before returning. Existing worker/process isolation is the boundary. Public ACT objects remain mutable: checks establish selected identity/topology and registered delegates, not a deep immutable copy of every metric/action field or rollback of arbitrary plugin side effects.          |

`TextReportChecks` uses only synthetic objects and temporary Host directories.
It checks fixed formatting vectors and the unchanged legacy API, owner/original
object references, selected/missing/removed scope, catalog-array ownership,
revision replacement, callback/reentry/Dispose/reported errors, sort re-registration,
body+post-validation failures, output/template/registry boundaries, regex and
cooperative timeout recovery, direct scope changes, same-name cloned allies and
selective IgnoreEnemies behavior. Original TextExportProbe and the IL policy
probe are separate evidence, not a claim of native UI execution on Linux.

## Remaining delivery stages

1. Bridge `TextReportCommand` and Rust `text_report_catalog` / `text_report` are
   connected. Private session-bound revisions and encounter IDs reject old/foreign
   selections before dispatch. The owner thread renders once; fixed 16 MiB reply
   space includes worst-case JSON escaping of the managed 1 MiB output budget.
   Requests are bounded to 256 KiB; ABI size/version/session/command/reserved fields,
   strict UTF-8/JSON (including duplicate keys), reply identity/revision and catalog
   counts/keys are validated. No callback capacity retry or automatic revision
   retry occurs. Missing exact encounters return `None`.
   Presets expose `defaultOptions` separately from ordered `{ index, options }`
   entries. ACT's negative default index is not a registered-list slot. Copy a
   selection's option values into the draft; never re-resolve an old index after
   registry changes. Reference/order changes invalidate the catalog revision, even
   for equal-valued replacements. No plugin `ToString`/equality callback is run by
   catalog discovery. At most 1024 presets and 1 MiB aggregate UTF-8 option text
   (including the default) are accepted, in addition to individual template limits.
   Synthetic ABI and Rust malformed-envelope/body tests plus an actual isolated
   worker fixture cover dynamic variables/sort keys, original owner/object callbacks,
   descending order, Unicode, callback/invalid UTF-8/output-limit errors, registry
   mutation, removed selection, recovery, shutdown and stale IDs after restart.
2. Existing Rust/UI controls now connect explicit encounter/template/options,
   draft/preview ownership and new-file export. Synthetic temporary-path tests
   cover byte encoding, no-overwrite, failure/cancel boundaries and late replies.
   Native clicks and Windows execution remain pending. No clipboard route is added.
3. The first managed [current-table XML/static HTML slice](table-reports.md) now
   captures original rows/parents and registered columns, uses SQL names/values for
   the four ACT table-XML roots, and escaped cell values for static HTML. Its
   Bridge/Rust typed table-report selection and isolated-worker synthetic tests are connected.
   The current-table GUI/source preview and shared no-overwrite save are connected. Full HTML hierarchy/depth,
   the managed static checked-encounter HTML table core is implemented with explicit levels/columns/sorts; its Bridge/Rust API and actual isolated-worker regressions are connected; Rust/UI multi-selection/depth/column/sort editing, safe source preview and no-overwrite save are connected; graph models/rendering/image dimensions remain unfinished. The first managed [task XML hierarchy/depth core](hierarchical-reports.md) is connected with three ACT depth modes, owned topology and synthetic vectors; its Bridge/Rust API and isolated-worker regressions are connected; Rust/UI depth selection, safe source preview and explicit no-overwrite save are connected with actual Host/controller regressions;
   unsupported choices and security differences are explicit before I/O.
4. L7c synthetic ODBC/schema/sink work and the shared L9d output preference audit
   remain separate. No external upload/service/real database has been accessed.

The ordinary text GUI/new-file flow is connected; L7b as a whole is **not complete**.
HTML/table-XML serializers, native font/RTF variants and the L7c ODBC writer remain separate.
L6b OS clipboard provider remains approval-blocked and has not been retried;
WinForms bridge remains plan-only.

## Text report GUI and new-file save

Analysis now contains a text-report accordion using the existing Rust/UI-adapted
Accordion, Button, Input and Label components, plus a multiline companion to the
same Input adapter. Dioxus owns the controller; no JavaScript/clipboard bridge is
introduced. Load the catalog, copy the ACT default or a registered preset into the
draft, edit explicit templates/options, then generate for the selected encounter.
Preset pages expose 16 entries at a time. Reload preserves an explicit draft.

The generated preview owns its report, template/options, encounter/title and
session/selection token. Draft edits clear it. Selection changes, restored context,
panel navigation, cancellation and Stop invalidate pending UI results. A canceled
plugin callback is not preempted; the existing worker/process boundary handles
shutdown. A pending operation retains admission until its response is retired, so
cancel does not permit overlapping owner operations. Empty templates preserve the
ACT LF-only report; missing exact selections show an unavailable error.

Only an explicitly entered destination can be reviewed and confirmed. Saving uses
the generated report bytes, UTF-8 without BOM and unchanged LF, independent of any
later draft. A same-directory temporary file is written, flushed and file-synced,
then published with `persist_noclobber`; existing or concurrently created files are
never replaced. No directories are created. Failures before publication and
cancellation clean the temporary file. Platform/filesystem publication failures
are surfaced, not treated as a durability guarantee; no OS-crash/disk-failure
promise or directory-sync policy is added.

Cancellation, leaving and Stop can win an atomic permit before publication begins.
Once publication is admitted, the explicit save may complete after Stop, and late
UI replies remain ignored. This boundary is stated beside the save action. A
completed file is not deleted to simulate cancellation. Failed saves preserve the
preview and path for retry. Callback/render failures preserve the draft and never
save a partial result. Real clipboard access and upload are absent. Current-table HTML/XML uses the separate explicit editor in the same panel; hierarchical static-table HTML has its separate multi-encounter editor and shared source save; graph output remains unfinished.

Synthetic file tests inject write/flush failures and publication races; actual
isolated-Host controller tests exercise preset copy, Unicode bytes, repeated save,
no-overwrite, missing directory, stale confirmation, cancel, panel/selection/session
changes, failed/empty/missing results, and Stop before/after publication. Native
click/focus/dialog behavior and Windows execution remain unverified on ser7.
