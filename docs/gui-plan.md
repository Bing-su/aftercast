# Desktop GUI plan

The GUI is a future consumer of the asynchronous `aftercast` library. Host
ownership, bounded command admission, Windows STA, message pumping, import jobs
and shutdown now belong to the library; the GUI must not add another host thread
or duplicate its command queue.

The GUI executable also hosts the isolated child. Its synchronous main must call
`aftercast::run_child_if_requested()` before Dioxus, WebView, windows, logging
services or an async runtime are initialized. On true, return immediately; on an
error, report it and exit. Only false enters normal UI startup. Host relaunches
that same file with `--aftercast-child`; no separately deployed worker is used.
Use `run_child_if_requested_with_audio` here if the app supplies custom backends.

| Owner                       | Responsibility                                                                      |
| --------------------------- | ----------------------------------------------------------------------------------- |
| Dioxus executor             | Components, selection, filters, pending state and visible errors                    |
| `Host`                      | Session lifetime, playback error receiver and explicit shutdown completion          |
| `HostHandle`                | Cloneable Send + Sync access to typed asynchronous operations                       |
| Same executable, child mode | CLR/plugin lifecycle, native messages, ACT callbacks, timers, audio and graph state |
| UI state                    | Owned encounter summaries/details/pages, session and selection generations          |

Normal flow: UI action → await HostHandle method → owned result → Dioxus state.
Construct and initialize prerequisite plugins before their dependents, e.g.
FFXIV, OverlayPlugin, then cactbot. Gate addon readiness on its handler responses;
`InitPlugin` returning does not establish completion of deferred startup.

| UI behavior       | Contract                                                                                           |
| ----------------- | -------------------------------------------------------------------------------------------------- |
| Startup           | Report runtime/deployment/plugin failures before enabling host actions                             |
| Import            | Use start_import, show byte/record progress, request cancel explicitly and wait for restored state |
| Refresh           | At most one automatic refresh in flight; suppress conflicting work and display Busy errors         |
| Stale responses   | Discard responses belonging to an obsolete session or selection                                    |
| Playback failures | Consume Host::next_audio_error and display failures without pretending speech succeeded            |
| Shutdown          | Await Host::shutdown; report ShutdownTimeout as a forced exit with potentially lost unsaved state  |
| Blocked plugin    | Independent supervisor kills/reaps the child after the graceful deadline; a new Host can restart   |
| Native UI         | Plugin controls remain on the library-owned STA; Dioxus does not convert WinForms controls         |

## ACT analysis workflow target

The target is functional compatibility with ACT's analysis and plugin workflows.
Original layouts, widget choices and incidental gestures are reference evidence,
not requirements to reproduce verbatim. Preserve the data hierarchy, access to
individual actions/source logs and meaningful operation outcomes; allow a clearer
presentation. For example, settings search must help locate a setting, but need
not reproduce ACT's highlight colors or exact search interaction.

The GUI must preserve ACT's analysis workflow, including tree navigation and
secondary windows. The earlier B visual direction is provisional: flattening
categories into tabs is acceptable only if every navigation level, activation
action and selected context below remains accessible. A static aggregate table
does not establish ACT workflow equivalence.

### Investigation baseline — 2026-10-26

| Evidence | Scope and limit |
| --- | --- |
| Running Windows ACT 3.8.5.288 | Observed the main tree, skill aggregate and individual-action views, and an open `Encounter Logs` window. Automated input did not reliably reproduce all transitions; these observations are not a completed interaction test |
| Installed executable inspection | SHA-256 `934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`; matched `binaries/act/Advanced Combat Tracker.exe`. Event handlers and form implementations establish the behaviors below at the code level |
| Local evidence, excluded from distribution | `target/design-reference-act/`: `act-skill-detail.png`, `act-attack-events.png`, `act-encounter-logs.png`, `act-ui-evidence-20261026.json`, and `current/` decompiled types. These are maintainer reference artifacts, not portable tests or shipped application assets |

### Navigation and activation

The tree and the table are two paths through the same selected data hierarchy.
Selecting a tree node changes the table, its columns, chart and context menu;
activating an aggregate row selects the corresponding child in that hierarchy.

| Selected tree node | Table contents | Row activation, including double-click |
| --- | --- | --- |
| Zone (`ZoneData`) | Encounters | Select the encounter child |
| Encounter (`EncounterData`) | Combatants | Select the combatant child |
| Combatant (`CombatantData`) | Damage/healing categories | Select the category child |
| Category (`DamageTypeData`) | Attack/skill aggregates | Select the attack child |
| Attack (`AttackType`) | Individual actions (`MasterSwing`) | Open encounter logs and locate the selected action's source log entry |

| Behavior | Implementation target | Acceptance evidence still required |
| --- | --- | --- |
| Selection and return | Keep tree selection and the full selected path consistent with table activation. Parent navigation returns one level; ACT then collapses the selected parent node | Reach the same rows through the tree and through repeated table activation; return through all parent levels |
| Category coverage | Preserve parser-provided categories and incoming/outgoing direction, including aggregate `All` rows; four fixed damage/heal tabs must not hide other categories | Exercise categories beyond the mockup's four tabs and distinguish overlapping aggregates |
| Individual-action table | Show action time, attacker, target, skill, damage and available action-specific columns, rather than substituting aggregate rows | Compare a known skill's action rows and ordering with ACT, including repeated actions at the same timestamp |
| Tables and charts | Provide sorting and level-appropriate columns. Chart context follows the selected node; skill-level action charts differ from category-level contribution charts | Compare selection, sort order and chart source data at each level; do not reuse the mockup's contribution chart for every level |
| Main-chart double-click | ACT's main chart handler stops graph generation and clears tooltip items; it does not open the action-log window | Verify this separately from table activation before assigning a replacement gesture |

The installed reference's main handlers are `FormActMain.tv1_AfterSelect`,
`lvDG_ItemActivate`, `btnNavBack_Click`, `cmsTView_Opening` and
`pb1_DoubleClick`. Tree expansion populates children; it must not be confused
with activating an individual action.

### Log window and contextual tools

| Surface | Implementation target | Reference behavior / acceptance target |
| --- | --- | --- |
| `Encounter Logs` | Independent nonmodal window retaining the originating encounter and selected action; opening another window must not replace the first window's context | ACT constructs a new form, copies the encounter's log-entry list, then selects and reveals the entry matching the action's `TimeSorter` / log `GlobalTimeSorter`. Compare two windows from different encounters |
| Log search | Text and regex search, previous/next match, match highlighting and results-only view | Verify searches and selection in both full-log and results-only views |
| Log copying | Separate all entries, search results, selected entries and combat-log copy scopes | Compare copied contents; ACT's combat copy excludes entries of type `0` and `-1` |
| Encounter context menu | Log viewer, timeline and encounter VCR entry points, alongside relevant export commands | Entry points and secondary-tool implementations are code-inspected in [secondary analysis tools](act-analysis-tools.md); populated runtime comparisons remain pending |
| Combatant context menu | Death, special-attacks and avoidance reports | Source-inspected flows and calculation cases are recorded in [secondary analysis tools](act-analysis-tools.md); validate populated workflows before claiming equivalence |
| Category context menu | `Lookup by Combatant`, grouping outgoing actions by target or incoming actions by attacker, with further skill/action drill-down | Reference `FormByCombatantLookup.ShowLookup`; validate the secondary tree and table against the same source category |
| Attack context menu | Spell-recast calculation and its timer handoff | Reference `FormSpellRecastCalc.ShowMedian`; validate calculations and handoff separately from action-log activation |
| Timeline to logs | Open encounter logs at the selected time while retaining the timeline window | `FormTimeLine.pb1_MouseClick` handles a right-click on the time plot and highlights that timestamp in a new log window |

### Backend prerequisites and delivery boundary

The [secondary analysis investigation](act-analysis-tools.md) expands the
contextual-tool targets with source-confirmed report flows, calculation details,
data prerequisites and comparison cases. It includes the distinction between
VCR's generated interaction layout and actual game coordinates, and between
the recast tool's name and its minimum-interval timer handoff.

Existing `encounters`, `encounter_detail` and `encounter_attacks` queries support
an initial aggregate GUI. They do not provide the individual-action and
encounter-log queries needed to complete the workflow above.

| Required capability | Completion criterion |
| --- | --- |
| Hierarchical queries | Expose enough zone, encounter, combatant, category and attack identity to select and return consistently without relying on display names alone |
| Individual-action queries | Return bounded owned pages for the selected attack/category, with session-scoped identity and visible stale/removed/error handling |
| Encounter logs and action linkage | Retrieve retained logs for the originating encounter and link an action to its source entry by identity/order, including same-timestamp records. A live bounded subscription is not a historical encounter-log store |
| Retention and bounds | Define log/action retention, page limits and behavior when data is unavailable or omitted; never present an incomplete log as complete |
| Secondary windows | Keep each window's originating context through main-view selection changes; reject obsolete requests across session restart and report unavailable source data |
| Supporting reports | Identify the data/API requirements for timeline, target lookup, death/avoidance reports, VCR and recast calculation before implementing those surfaces |

Before selecting a final design, review a connected set of mockups: aggregate
view, individual-action view, action-linked log window, and stage-specific
context menu. Preserve this workflow when refining B's visual treatment. The
first aggregate GUI may precede these additions, but must be described as partial.

## Other ACT tabs — investigation and implementation targets

This 2026-10-26 follow-up inspects the same pinned ACT 3.8.5.288 executable.
The running window exposes Options, Custom Triggers, Plugins, Import/Export,
History Database, Notifications and About. Automated tab clicks and keyboard
navigation did not reliably change the visible page, and accessibility exposed
only the window/title. User-assisted tab switching subsequently allowed visual
confirmation of all seven tab entry screens. Captures and observed states are
listed below. Deeper settings pages, plugin-owned page interiors, additional
import/export tasks and operation results remain code-inspected or unverified;
viewing the entry screens is not a completed interaction comparison.
No plugin enablement, settings edits, imports, deletion, network updates or
feedback submissions were performed during this investigation.

### Visually confirmed entry screens

Captures are local maintainer evidence under `target/design-reference-act/`.
Observed selection/default values are evidence of this running session, not
required defaults for aftercast or proof that a feature executed successfully.

| Screen | Observed state | Capture |
| --- | --- | --- |
| Options / Miscellaneous | Full settings category tree, log status and open/pause/close controls, independent original-line and autosave policies, contextual Help area | `act-options-misc.png` |
| Custom Triggers / Triggers | Empty trigger list; regex, None/Beep/WAV/TTS choices, category/zone restriction, timer or tab name, independent Trigger timer/Add Results Tab flags, Add/Edit/Remove, preview and Benchmark CTs entry | `act-custom-triggers.png` |
| Plugins / Plugin Listing | Scrollable listing with names, status text, enabled checkboxes, move controls and remove icons; file selection, plugin-info area and loaded-assembly report entry. Separate plugin-owned tabs coexist above the listing, including FFXIV, OverlayPlugin and other installed plugins | `act-plugins-list.png` |
| Import/Export / Import a Log File | Task tree lists raw-log/clipboard/ACT imports and ACT/HTML/SQL-ODBC/XML exports. Start/end calendar/time selectors or file-boundary choices, import-trigger-results flag, continue-to-combat-end flag, file selection and Help area | `act-import-log.png` |
| History Database | Zone/title search scope, date/time search and calendar, empty results list, checked-result import/delete controls, history-saving control and age-based deletion control | `act-history-database.png` |
| Notifications | Existing Old Log File notification with title, message, time and Show control; Mark all as read, Clear read and badge-blinking control | `act-notifications.png` |
| About | Version 3.8.5 / Release 288, data-folder and publisher links, update controls and feedback form with optional return email and error-log attachment | `act-about.png` |

| ACT tab | Installed implementation and workflow | aftercast implementation target / acceptance evidence |
| --- | --- | --- |
| Options | Left category tree; right controls and contextual help. Search matches control text and help text and highlights matching controls and category nodes | Searchable settings navigation with discoverable help. Verify setting application, validation, persistence and restart behavior per setting; a generic settings form is insufficient |
| Custom Triggers | Category/trigger tree with enable checkboxes; selecting a trigger loads its regex, category, sound action, timer/tab name and flags. Add/Edit validates the regex. Actions include none, beep, WAV and TTS; category restriction, timer activation, tabbed results, sound preview and XML sharing are separate controls | Preserve editing, per-trigger/category enablement, zone restriction, timer/result-tab behavior and explicit preview actions. Compare save/reload, invalid regex handling, category toggles and XML round trips with controlled logs; editor UI is not proof of matching equivalence |
| Plugins | Plugin Listing contains file browsing, Add/Enable, per-plugin enabled/status controls, removal and ordering. Selecting a listing shows metadata; View Loaded Assemblies writes a report into the info area. Enabled plugins receive their own tab and status label via `InitPlugin`; disabling invokes `DeInitPlugin` and removes the tab | Separate host-owned plugin management from plugin-owned controls. Keep prerequisite initialization order, failures and readiness visible. Validate disable/re-enable, saved order and settings, dependency failures and native plugin UI separately; constructor or initialization return alone does not establish readiness |
| Import/Export | Left import/export task tree, task-specific controls and help. Import paths include raw logs, clipboard snippets and ACT files; export paths include ACT, HTML, ODBC and XML table data | Keep raw parsing, encounter interchange and report exports distinct. Show input/output scope, progress, cancellation and failures. Compare formats and side effects separately; existing XML/gzip support does not establish clipboard/HTML/ODBC/table-XML parity |
| History Database | Text search over zone names/encounter titles, date/time search, calendar with matching dates, checked result selection and Import Checked. History saving and retention/deletion controls are separate from searching | Distinguish querying the index, selecting results and restoring/reimporting data. Verify date boundaries, zone/encounter selection, unavailable source logs and restart persistence. Source-log reimport is a separate capability from the existing archive checkpoint restore |
| Notifications | Notification history entries have title, message, time and read state, with an optional Show callback. Controls mark all read, clear read entries and configure badge blinking | Provide readable error/notification history with explicit entry actions and read state. Verify per-entry action, unread badge and clear-read behavior. `HostHandle::diagnostics` is a bounded diagnostic source, not an implementation of ACT's notification lifecycle |
| About | Version and update controls, application-data-folder link, publisher links and feedback form with optional email/error-log attachment | Show aftercast's own version/runtime information and data location. Define aftercast-owned update/feedback policy before adding those workflows; do not reuse ACT publisher endpoints. Network and submission behavior remains untested |

### Options families to retain in the inventory

These are functional groups from `PopulateOptions`, not a requirement to copy
every legacy label or use the same final visual layout. Parser/plugin-defined
controls and platform-specific settings need their own validation.

| Family | Reference scope and examples |
| --- | --- |
| General and sound | Miscellaneous host behavior, sound settings, TTS engine selection, regex pronunciation corrections and a speech test |
| Import/export preferences | Import/export behavior, XML sharing and XML subscriptions; distinct from the Import/Export task tab |
| Main tables | General refresh/number formatting/sort behavior, encounter timing/culling and separate column configuration for zone, encounter, combatant, category and individual-action tables. Category-table settings include checked columns, reordering and primary/secondary sort selection |
| Output/display | Graphing, mini-parse, text exports, ODBC, web server and HTML output |
| Appearance | Colors/fonts, application controls, graphs and miscellaneous color rules |
| Selective parsing | Player list and modes separating display/export filtering from recording only actions involving listed players; these modes affect different data boundaries |
| Data correction | Miscellaneous corrections, combatant renaming and ability redirection |

### Cross-tab workflows and backend gaps

| Workflow | Required design and implementation boundary |
| --- | --- |
| Raw-log import | Start/end can be file boundaries or date/time bounds, with an option to continue to combat end. Importing custom triggers is explicitly limited to tabbed results in the reference. Show the separate progress window, cancellation and character/viewpoint context; do not imply existing full-file import covers every option |
| Encounter/report export | ACT export has an include-log-lines option. HTML selects table/graph depth and image dimensions; table XML selects data depth. Preserve explicit scope and report unsupported formats/options before output |
| History reimport | `ThreadHistoryImport` resolves source logs using record metadata, requests a file when no match is found, and parses the selected time range. Existing `save_history` / `restore_history` stores and restores archived graph data instead; add source-log location/range handling before claiming this workflow |
| Trigger output | Result tabs, spell timers and audio previews connect trigger management to other surfaces. Preserve output context and errors; headless trigger/audio support does not provide the editor, tabbed results or timer-definition editor |
| Plugin UI | `PluginListingPanel` supplies a plugin-owned `TabPage` and status label. Current Rust create/initialize/deinitialize calls cover lifecycle operations, not a rendered plugin-tab bridge or a complete management/query API |
| Settings and history ownership | Classify settings as GUI-owned, library-owned or plugin-owned. Define supported persistence and restore boundaries before wiring controls; original ACT history saving is not an automatic policy already supplied by aftercast's explicit checkpoints |

The rendered Options/Miscellaneous page confirms separate controls for opening,
pausing and closing logs; automatically following recently changed files;
recording View Logs entries; preserving original lines for View Logs versus
Custom Triggers; and automatically saving settings versus encounter history.
These distinctions belong in the settings model; observing their controls does
not verify their effects. Preserve independent policies rather than merging
them into a single generic autosave or log-recording switch.

The rendered Custom Triggers page also exposes `Benchmark CTs...`, connected to
`FormActMain.btnOpenCtBenchmark_Click`. Its separate window and invalid-input
warning were subsequently inspected with user-assisted clicks, as recorded
below. Successful benchmarking and populated-trigger behavior remain unverified.
The empty trigger/result lists do not validate editing, result tabs or matching.

### Button interaction investigation

These are user-assisted action/result observations on the running reference;
automated clicks remain unreliable. A visible result does not validate all
branches, persistence or cancellation. Record source-confirmed branches
separately rather than promoting them to runtime-tested behavior.
This ledger documents the reference; it is not a checklist requiring identical
buttons, popup wording or display mechanics. Prioritize investigations that
resolve a missing capability or a consequential behavior. Do not require repeated
manual walkthroughs for cosmetic or routine control behavior already understood.

| Entry and action | Observed result | Implementation target and remaining validation | Local evidence |
| --- | --- | --- | --- |
| Plugins / Plugin Listing: View Loaded Assemblies | The existing right-hand Plugin Info area becomes a scrollable report with ACT and plugin assembly sections, identities, versions and paths. The listing remains visible; no separate report window was observed | Support an in-place diagnostic report without changing enablement/order. `FormActMain.btnGetAssemblies_Click` also logs the report; the log write was code-confirmed, not independently checked | `act-button-loaded-assemblies.png` |
| Custom Triggers: Benchmark CTs... | Opens a separate Custom Trigger Benchmark window with an encounter tree, Restricted/Category/Custom Trigger/Time Spent columns and Perform Benchmarking. Current trigger list is empty | Preserve encounter/log context and the distinction between opening the tool and running a measurement. `ShowBenchmark` uses `Show()`, not a modal dialog; closing normally hides the shared form in the reference | `act-button-trigger-benchmark.png` |
| Benchmark: Perform Benchmarking with insufficient input | An Invalid Selection message box asks for an encounter with more than 1,000 log entries; no measurement result appears | Preserve visible validation before work. Code checks a selected EncounterData and rejects log counts at or below 1,000; the exact guard branch behind this observed warning was not independently identified. Valid measurement, running state and completion remain pending | `act-button-benchmark-invalid.png` |

The OverlayPlugin-owned general page was also visually observed at
`act-overlayplugin-general.png`: overlay selection and add/delete/rename controls,
event settings, update/support/cache controls and a lower plugin log pane. Those
buttons were not executed. This page illustrates the need to host plugin-owned
workflows; it is not ACT-owned UI behavior or proof of overlay configuration parity.

Further investigation should resolve core workflow gaps: action-to-log linkage,
plugin-owned configuration access, trigger/timer outputs, and encounter
import/export/history semantics. Trigger CRUD, plugin lifecycle/order changes,
imports/exports and persistence need meaningful disposable-state tests before
completion claims. Routine search/clear, standard file-dialog mechanics and exact
warning wording do not need independent manual reproduction unless they expose
a compatibility issue. Destructive or external actions remain outside the live
inspection workflow.

### Core capability gaps found in source comparison

The following 2026-10-26 findings compare the pinned reference's decompiled
implementation with the current repository. They are code observations, not new
runtime parity results. Priorities describe GUI prerequisites: P0 blocks the
connected analysis/plugin workflow; P1 completes the supporting tab workflows.
An aggregate-only preview can still precede them.

| Priority / workflow | Reference behavior and current boundary | Minimum implementation target / acceptance example |
| --- | --- | --- |
| P0 — Retained encounter logs | ACT records encounter logs when recording is enabled and combat is active, choosing original or processed text. The current live/import dispatch paths publish original records and invoke parser callbacks, but do not themselves append encounter logs. `EncounterData.LogLines` and XML support alone do not provide recording | Define encounter association, original/processed text policy and bounded retention before adding historical queries. Import a known encounter and open an action's source after the live subscription has advanced; show unavailable/omitted data explicitly |
| P0 — Action/source identity through restore | Current XML writes a swing's `TimeSorter`, but writes log entries without their `GlobalTimeSorter`; reading assigns new sequential log sorters. The private archive uses this encounter serialization too. Restored text does not establish preserved action-to-log identity | Preserve or explicitly reconstruct a validated relation in the appropriate format, with a policy for older archives lacking that relation. Two source lines with the same timestamp must remain distinguishable; never silently select a different entry |
| P0 — Plugin configuration access | Windows registration already attaches native plugin pages to a child-owned `TabControl`; this is more than a status label. Rust lifecycle calls do not expose a GUI operation to show/select those pages. Configuration is plugin-owned | Provide a deliberate route to the native configuration surface while retaining child/STA ownership. Verify a real plugin's controls, nested pages, focus and close/reopen behavior; do not duplicate arbitrary plugin settings in Dioxus |
| P0 — Plugin enable/retry lifecycle | A current instance can initialize only from `Constructed`; failure and deinitialization leave terminal `Failed`/`Stopped` states. Re-enabling cannot call initialize on the old `PluginId`. Creating a replacement does not unload the old assembly or undo arbitrary plugin effects | Model configured plugin entries separately from runtime instances. State the fresh-instance/restart policy, dependency ordering and failure outcome. Check disable → enable and failed initialization → retry using disposable plugin state |
| P1 — Trigger editing and saving | Managed trigger registration, execution and startup/shutdown persistence exist, but the Rust facade has no trigger-management operations. The key contains category and regex; editing those requires removal of the old key. Changing `ShortRegexString` alone does not recompile `RegEx` | Validate/compile a replacement before publishing an edit; define collision/delete behavior and when edits become durable. An invalid regex leaves the previous trigger usable; a category/regex rename leaves exactly one trigger after restart |
| P1 — Trigger result records | ACT can process triggers during import when enabled, append tabbed results, then skip import audio/timer side effects. Current import dispatch does not process custom triggers, and live execution has no result-record output; a `Tabbed` property is not a results implementation | Separate matching/result collection from live effects. Expose bounded timestamped results and source context for GUI display. Importing a matching fixture may populate results when enabled but must not play sound or start live timers |
| P1 — Timer definitions versus running timers | ACT saves/loads spell definitions, including enablement, timing, restrictions, cues and display fields. Current managed timer definitions/frames and execution exist, but no host timer-definition persistence or Rust management/query route was found | Define edit/list/save operations for definitions separately from transient running frames. Verify edited definitions survive restart and specify whether active countdowns reset. This is ACT spell-timer behavior; raidboss timelines are a separate feature |
| P1 — Plugin-defined table data | Managed registrations retain incoming/outgoing categories and default visible columns for the five analysis levels. Current typed Rust aggregate queries do not expose the registration metadata or arbitrary plugin-defined cell values | Define bounded metadata/value queries and sorting behavior if these columns are supported. A plugin-added category/column must be discoverable without hard-coding its label; execute plugin formatting/comparison on the host owner thread |
| P1 — History discovery and import meaning | Current archive restore is a checkpoint restore with a combined text filter, not ACT's date/search/result-selection workflow or raw-log reparse. The raw importer has progress/cancel but no reference-style time-range selection | Provide archive metadata discovery and explicit selection before restore; distinguish restoring saved aggregates from reparsing source logs. Show partial/cancel/error outcomes and source availability without describing either operation as the other |
| P1 — Actionable notifications | The current `NotificationAdd` explicitly throws `NotSupportedException`; diagnostic snapshots are a separate bounded mechanism. A diagnostic list alone does not implement ACT's notification callback behavior | Decide and document the supported notification contract, including the optional Show action. A supported notification must remain inspectable and invoke its action on the owning host context; unsupported behavior must stay explicit |

For a new visual comparison, retain B's visual direction and show a connected
set: hierarchy/detail, individual actions plus an independent log window,
plugin listing plus access to a plugin-owned page, and trigger definitions plus
result records. Mark unavailable capabilities as planned rather than implying
that current host APIs already support them. Exact ACT button placement,
settings-search highlighting and incidental warning wording are not acceptance
requirements.

| Maintained source evidence | Scope |
| --- | --- |
| [Live dispatch](../managed/ActCompat/LogHost.cs), [import dispatch](../managed/ActCompat/RootImport.cs), [log subscription](../crates/aftercast/src/logs.rs), [encounter XML](../managed/ActCompat/EncounterXml.cs) | Recording boundary, bounded original records and archive sorter behavior |
| [Plugin lifecycle](../managed/Bridge/PluginSession.cs), [host environment](../managed/ActCompat/HostEnvironment.cs), [Rust plugin API](../crates/aftercast/src/plugins.rs) | Child-owned native tabs, terminal instance states, trigger startup/shutdown persistence and unsupported notifications |
| [Trigger execution](../managed/ActCompat/CustomTriggers.cs), [trigger model](../managed/ActCompat/CustomTrigger.cs), [trigger persistence](../managed/ActCompat/TriggerPersistence.cs), [timers](../managed/ActCompat/FormSpellTimers.cs) | Managed execution versus missing GUI management/result/persistence capabilities |
| [Registration views](../managed/ActCompat/RegistrationViews.cs) | Plugin-defined categories and default column metadata |

Reference-side anchors are `FormActMain`'s log-recording branch,
`ParseCustomSingle`, `SaveXmlSpellTimers` / `LoadXmlSpellTimers`, and the
previously inspected plugin listing and history handlers in the local evidence
directory. No production settings, plugin enablement or archive contents were
changed to obtain these comparisons.

Before considering these tabs implemented, capture their actual rendered states
and compare one core workflow plus failure/cancel/restart behavior against the
pinned reference using disposable inputs/state. Plugin-provided pages require
separate evidence per plugin and platform. The local source evidence is retained
under `target/design-reference-act/current/`: `FormActMain`,
`PluginListingPanel`, `NotificationHistoryItem`, `FormImportProgress`,
`FormXmlSettingsIO`, the inspected `IO_*` controls and `Options_*` controls.
This is a feature/workflow inventory, not an exhaustive public-control audit.

| Stage                 | Delivery                                                                                                    | Evidence required                                                                                                                            |
| --------------------- | ----------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Platform feasibility  | Minimal Dioxus window using Host directly; choose/lock GUI dependencies at implementation                   | Windows native message/control operation while the GUI remains responsive; Linux WebView startup; hidden windows do not affect host progress |
| First usable workflow | Runtime/plugin selection, initialization, UTF-8 import, progress/cancel, encounters/details/attacks         | Fixture totals match direct queries; partial-import/error states and stale selection handling                                                |
| ACT analysis workflow | Full hierarchy, individual actions, action-linked nonmodal log windows and contextual tools described above | Compare against pinned ACT with a known corpus; verify double-click/keyboard activation, parent navigation, same-timestamp log linkage and independent window contexts |
| Live/history          | Existing file-following results, bounded refresh, end combat, explicit history checkpoints and ACT XML/gzip | Live results match host queries; restart restores the last checkpoint                                                                        |
| Distribution          | Reproducible GUI packaging and prerequisites                                                                | Packaged launches, keyboard/focus/scaling checks and platform-specific plugin evidence                                                       |

Raw-log viewing must retain subscription bounds and show loss counters. Native
capture, lossless recording, original plugin UI equivalence and upload acceptance
remain separate validation work.

## Deferred task — extensions after ACT reconstruction

Recorded 2026-10-26 at the user's request. Status: **deferred until ACT
reconstruction is complete**. These ideas are outside the current compatibility
and GUI implementation scope. An aggregate-only GUI or successful plugin startup
does not satisfy this prerequisite; assess completion against the agreed ACT
workflow targets and compatibility evidence before starting this task.

The extension goal is to go beyond ACT with an integrated AI agent that can
inspect combat data, explain findings with linked evidence, and navigate the
corresponding analysis views. [Page Agent](https://github.com/alibaba/page-agent)
is a reference for the natural-language interaction experience, not a selected
dependency or a verified desktop WebView integration.

| Candidate | Example / intended outcome |
| --- | --- |
| Evidence-linked analysis agent | Explain a death and open its contributing actions/source logs; distinguish observations from inferred causes and unavailable data |
| Cross-attempt comparison | Align comparable encounter segments and explain changes in actions, targets and survival time with inspectable evidence |
| Trigger authoring assistant | Draft a rule from selected logs, inspect matches against historical encounters, then review before applying |
| Configuration and troubleshooting assistant | Inspect plugin state, dependencies and diagnostics; propose reviewable changes with a clear recovery path |
| Reusable analysis tasks | Save a chosen filter/comparison/report workflow for repeated use after encounters |
| External agent integration | Consider exposing the same bounded application operations to external agents through MCP |
| Supporting analysis UX | Evaluate segment selection, bookmarks/notes, configuration history/undo and restoring or sharing analysis context |

Suggested first increment after the prerequisite is met: read-only analysis of
the selected encounter, evidence links and navigation to the relevant views.
Trigger/configuration writes and recurring execution are later candidates, not
automatically authorized by this backlog entry. Prefer explicit data/query/view
operations for the agent; investigate UI automation where needed. Reassess
priority, model/provider choice, data disclosure, cost and desktop compatibility
when this task is activated. Current data work should serve ACT reconstruction;
do not add speculative agent infrastructure now.

| Reference                        | Purpose                                        |
| -------------------------------- | ---------------------------------------------- |
| [Architecture](architecture.md)  | Library-owned scheduling and lifecycle         |
| [Usage](usage.md)                | Async calls, audio errors and startup batching |
| [Imports](import.md)             | Progress, cancellation and partial history     |
| [Combat queries](snapshots.md)   | Owned results and paging                       |
| [History](encounter-archives.md) | Explicit checkpoints and restoration           |
