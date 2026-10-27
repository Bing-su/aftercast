# Desktop GUI plan

The [GUI library selections and research](gui-library-research.md) records
confirmed choices and candidates for components, tables, charts and desktop
integrations. Selected libraries may be added for their documented purposes;
candidate suitability, version resolution and Desktop compatibility still need
verification before implementation is considered complete.

Confirmed library constraints (2026-10-26): do not write application-level JS
bridges; use Rust/Dioxus component APIs. Dioxus or library-internal JS is distinct
from an application-owned bridge. Include `dioxus-i18n` and `dioxus-sdk` from the
initial GUI implementation, with translation resources for UI text and SDK
features selected for the initial screens. ECharts and Tabulator are excluded;
evaluate `dioxus-charts` and Chorale against the analysis requirements instead.
Use Tailwind CSS v4 with Rust/UI's Dioxus component sources, `tw_merge` and its
Lucide `icons` package as the default UI stack. Track copied sources and licenses;
check each component for Desktop support and compliance with the no-app-JS-bridge
constraint. Reuse unstyled Dioxus primitives for missing behavior as needed,
with the same application theme rather than a second styled component system.
Use `miette` instead of `anyhow` for application error reporting and `usage-rs`
(`usage_rs` in Rust) for the application CLI. Preserve concrete library error
types and include the internal child mode in the common CLI parser. Dispatch on
the parsed mode before UI or application runtime initialization. Terminal
diagnostic rendering and localized GUI error presentation use their respective
output paths; do not display terminal ANSI output as GUI error content.
Reuse managed .NET Regex for log regex searches and ACT trigger validation and
execution; do not add an application-owned Rust regex engine. Expose the required
bounded asynchronous search/validation operations through Host, with explicit
timeout, cancellation, error and match-position semantics. Those GUI-facing APIs
are prerequisites, not capabilities already established by the existing trigger
engine. Distinguish .NET UTF-16 offsets from Rust UTF-8 byte offsets for highlights.
Selected application utilities also include `tracing-subscriber` and
`tracing-appender` for diagnostic collection and non-blocking rotating log files,
`directories` for OS-specific paths outside ACT-related file storage, `open` for
opening files/folders/URLs with the desktop, and `jiff` for date/time-zone handling.
Use TOML with `serde` and `toml_edit` (with its `serde` feature enabled) for
application-owned settings. Declare these dependencies in the GUI/CLI executable
crate when implementing it. Keep the logging worker guard alive through shutdown
and flush it outside the UI executor; forced termination can lose buffered logs.
Keep slow desktop launch helpers off the UI executor and preserve existing
Host/plugin settings, source timestamp identities and monotonic elapsed-time
measurement.

ACT-related settings, plugins, data and generated ACT-related logs/history use
the application executable directory and its existing relative subdirectory
layout. Resolve paths from the executable location, not the current working
directory, and pass the resolved Host data path to the child. Do not relocate
these files into OS-standard application folders or silently fall back there
when writes fail; show the failure and target path. Preserve existing file names
and references. This policy does not automatically move/copy user-selected
external source files. `directories` remains available for other OS-standard
path needs, not for choosing ACT-related storage.

Store application-owned settings in `config.toml` beside the executable.
Include a `schema_version` and validate both the version and values. Keep an
editable TOML document and update changed items rather than serializing the
entire settings struct over the original; preserve comments, formatting and
unmodified unknown entries, accounting for `toml_edit`'s preservation limits.
Report malformed or unsupported settings without silently overwriting them.
Write a completed temporary file in the same directory and atomically replace
the destination using platform-appropriate semantics; preserve the old file on
save failure and keep file I/O off the UI executor. Existing ACT/plugin XML and
other owned settings retain their current formats. This replaces the earlier
JSON proposal for application settings, not other existing `serde_json` uses.

The GUI is a future consumer of the asynchronous `aftercast` library. Host
ownership, bounded command admission, Windows STA, message pumping, import jobs
and shutdown now belong to the library; the GUI must not add another host thread
or duplicate its command queue.

The GUI executable also hosts the isolated child. Its synchronous main first
parses the common CLI with `usage-rs`, including `--aftercast-child`, then
dispatches on the parsed execution mode. In child mode, call
`aftercast::run_child_if_requested()` before Dioxus, WebView, windows, logging
services or an application async runtime are initialized. Return when child mode
ends; report errors and exit rather than falling through to normal startup.
Only normal application mode enters UI startup. Host relaunches that same file
with `--aftercast-child`; preserve its argument format and supervisor
authentication. No separately deployed worker is used.
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

## Initial executable progress (2026-10-26)

| Area                        | Implemented boundary                                                                                                                                                                                                                              |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Executable                  | `crates/aftercast-app`; common `usage-rs` CLI with early child dispatch                                                                                                                                                                           |
| Async workflow              | Start/stop, ordered plugin initialization, import progress/cancellation, encounter paging/detail and stale-response protection                                                                                                                    |
| Screens                     | Shell, analysis, plugins and preferences are separate feature modules; native settings entry points are implemented; overlay management remains future work                                                                                       |
| Styling                     | Tailwind v4 layout and shared primitives; official Catppuccin package, default Mocha with all four flavours                                                                                                                                       |
| Localization/config         | Fluent Korean/English; validated executable-adjacent TOML with comment preservation and atomic writes                                                                                                                                             |
| Runtime distribution        | Cargo `build.rs` downloads official Windows/Linux archives pinned by version, target and SHA-512; compressed archives are embedded and extracted/reused beside the executable. No separate Nu preparation script or installed runtime is required |
| Frontend tooling/components | Bun and `bun.lock`; Rust/UI-derived button, input, label, separator, outline badge and progress are used in the current screens                                                                                                                   |
| Verification                | Rust app/CLI tests, real Host import fixture and Windows WebView2 Playwright workflows; see the app README for commands and limits                                                                                                                |

This is the first aggregate workflow, not completion of the full plan below.

## Async from the first GUI implementation

Confirmed requirement (2026-10-26): the first GUI must use asynchronous operations
from the start, including visible pending/error states and operation lifecycle
handling. Do not build a synchronous GUI first and defer this to a refactor.

| Area                      | Implementation rule                                                                                                                                                                                                                                                |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| UI operations             | Await HostHandle operations through Dioxus tasks/hooks. Do not use `block_on`, synchronous waits or long calculations on the UI executor; ordinary short state updates remain synchronous                                                                          |
| Blocking work             | Prefer async APIs for file/dialog operations where supported. Move unavoidable blocking I/O or expensive processing to bounded background execution, respecting native thread-affinity requirements; marking a function `async` alone does not make it nonblocking |
| Admission and results     | Keep automatic refresh bounded to one in flight, handle Busy/errors, and discard results whose session/selection has changed. For example, selecting encounter B must prevent a late response for A from replacing B's details                                     |
| Cancellation and lifetime | Tie UI tasks to their component/window/session lifetime. Explicitly request cancellation of host jobs where required; dropping a UI future does not prove the host operation stopped. Keep cleanup and shutdown completion observable                              |
| Executor ownership        | Reuse Dioxus execution and the existing library-owned Host scheduling. Host futures do not require a Tokio runtime; add runtime infrastructure only for a concrete API requirement and never duplicate the host thread or command queue                            |
| Startup boundary          | Keep the synchronous child-mode dispatch before UI/runtime initialization as specified above. Asynchronous GUI operation does not move native plugin controls off their owning STA                                                                                 |

Acceptance must include slow operations while navigation, repaint and cancel stay
responsive; stale selection/session responses; closing a secondary window during
a request; and shutdown with work pending. Fast fixture results alone do not
establish nonblocking behavior.

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

## Plugin-provided UI implementation plan

Confirmed direction (2026-10-26): on Windows, open original plugin configuration
pages in a separate child-owned WinForms window and retain the plugin's existing
CEF overlay windows. Dioxus provides management and entry points. Do not rewrite
arbitrary plugin controls or embed native controls into the Dioxus WebView in this native path.
A separately approved follow-up plan for partial Linux rendering is below.

| Surface                  | Current evidence                                                                                                                                                                | Implementation target                                                                                                                                                                                                  |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| .NET configuration       | Plugin initialization receives real controls and registers its page in a child-owned `TabControl`; no public Rust show/select API exists                                        | Show a nonmodal configuration window, select the requested plugin's available page, and focus an existing window on repeated requests                                                                                  |
| Web-based overlay        | Original CEF and external Edge rendering have been exercised by the [worker integration tests](overlay-cactbot-process-validation.md); the CEF test also captures hidden frames | Retain original plugin-owned CEF windows and settings. Add user-facing discovery, create/show/hide and configuration entry points through the plugin's supported operations; verify visible-window behavior separately |
| Dioxus management        | GUI is not implemented                                                                                                                                                          | List plugin status and available UI actions; provide Open Settings and overlay-management entry points, pending/error states and explicit unsupported/unavailable results                                              |
| Linux/native portability | The Linux WinForms compatibility layer is headless; Windows CEF/Edge evidence is not WebKitGTK evidence                                                                         | Report unsupported native plugin UI explicitly. Track Linux plugin/browser support separately rather than showing an empty placeholder as working UI                                                                   |

### Planned partial WinForms configuration bridge

Approved for planning on 2026-10-27; implementation requires a separate explicit
request. This does not change the current Linux native-settings unsupported result
or introduce Mono/Wine automatically.

The scope is behavioral compatibility for the settings of a named ACT plugin,
using the existing `aftercast-winforms` headless Control model. It is not arbitrary
WinForms compatibility or a pixel-perfect replica. First inventory the target
plugin's actual controls, properties and events; determine the initial supported
subset from that evidence (candidates: Label, TextBox, CheckBox, ComboBox, Button
and TabPage).

1. Define stable control IDs and a versioned tree/state snapshot or delta protocol.
2. Render that supported subset with existing Rust/UI components in Dioxus.
3. Dispatch user events back to the original worker UI owner thread, preserving
   Invoke semantics, reentrancy rules and event ordering.
4. Synchronize plugin-driven changes, dynamic addition/removal and disposal;
   reject stale input after disposal or instance replacement and explicitly report
   unsupported controls/operations.
5. Compare real target-plugin settings behavior against ACT, with synthetic
   regression tests for event ordering, thread ownership, lifecycle, stale IDs and
   state synchronization. Completion requires the selected settings flow to work,
   rather than merely rendering a tree.

HWND/WndProc, Win32 P/Invoke, GDI+ custom painting, ActiveX/COM and complex data
binding are initially excluded or require a separate feasibility evaluation.
Keep this bridge distinct from the Windows path that hosts the original WinForms
TabPage and from ordinary aftercast Dioxus windows.

### Ownership and API work

| Boundary             | Required work                                                                                                                                                                                                                                 |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rust facade and IPC  | Add typed asynchronous Host operations to discover UI availability and request show/select/focus or supported overlay actions. Validate session-scoped plugin identity and lifecycle state; native controls/handles do not cross into Dioxus  |
| Managed owner        | Execute UI actions on the existing child STA and message pump. Keep page ownership/parent relationships used by plugins, including plugins that remove or replace their own settings tabs                                                     |
| Configuration window | Host the existing tabs rather than rebuilding them. Closing the window hides the configuration surface for reopening; it must not deinitialize plugins or end the Host session                                                                |
| Plugin lifetime      | Disabling/removing a plugin must remove its UI availability and invalidate outstanding actions. Shutdown disposes owned windows/controls on their owner; a new session must not reuse old UI identities                                       |
| Overlay lifetime     | Use existing plugin lifecycle and configuration contracts. Do not introduce a second CEF instance or a replacement browser solely to display an existing overlay. Identify supported vendor operations before defining each management action |
| JS policy            | Original plugin/browser JavaScript remains plugin-owned. These entry points use Rust/managed operations; no application-owned JS bridge is added to Dioxus                                                                                    |
| Persistence          | Plugin configuration remains in the plugin's existing format and executable-relative deployment/data layout; `config.toml` owns aftercast settings only                                                                                       |

### Delivery and acceptance

| Step                            | Required evidence                                                                                                                                                                                                                                                     |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1 — Native configuration access | From Dioxus, open a real plugin's page in the child-owned window; exercise controls and nested tabs, keyboard/IME input, focus, scaling, close/reopen and repeated Open Settings. Verify unavailable/self-hidden pages return an explicit result                      |
| 2 — Native lifecycle            | Run imports/queries while the configuration window is open and after it is hidden. Disable/re-enable or restart according to plugin lifecycle policy; stale requests must not reopen disposed UI. Verify graceful/timeout shutdown cleanup                            |
| 3 — Overlay management          | Discover supported overlay state, create/show/hide/configure an original CEF overlay and verify visible rendered content plus any required input/lock behavior. Recheck cactbot data/audio routing and plugin shutdown/restart; a hidden bitmap alone is insufficient |
| 4 — Repeatable validation       | Extend disposable native tests where OS automation is reliable and document manual steps otherwise. Playwright covers Dioxus entry points, while native settings/CEF windows require their own evidence                                                               |

This is planned work, not a claim that existing public APIs expose plugin UI.
The current source boundaries are [plugin registration](../managed/ActCompat/HostEnvironment.cs),
[plugin lifecycle](../managed/Bridge/PluginSession.cs) and the
[Rust plugin facade](../crates/aftercast/src/plugins.rs).

## GUI test plan

Recorded 2026-10-26. This is a planned test strategy, not evidence that a GUI,
test harness or CI job already exists. Add the first tests alongside the first
GUI workflow and expand them as features become available.

### Test layers and tools

| Layer                      | Tool                                                                                        | Scope and acceptance                                                                                                                                                                                                  |
| -------------------------- | ------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| State and configuration    | `cargo test`, existing `rstest` and `tempfile`                                              | Selection/session generations, pending/error state, settings validation/version handling and TOML edits preserving comments and unmodified entries; save failures preserve the original file                          |
| Host integration           | Existing Rust process/fixture tests                                                         | Actual child startup/authentication, import/cancel, failures, bounded admission, shutdown and restart. GUI tests reuse known fixture expectations rather than treating mocked Host output as integration evidence     |
| CLI                        | `usage-rs` test feature in dev dependencies                                                 | Arguments, help/version, parse errors and exit codes; child mode uses the common parser and never initializes the GUI. Verify the real authenticated supervisor launch separately from parsing the child flag         |
| Desktop UI end to end      | Playwright against the running Windows WebView2 app, subject to the feasibility check below | Real Dioxus event handling, displayed results, navigation, dialogs rendered inside the WebView and independent secondary-window context; exercise a real Host with fixtures for core flows                            |
| Visual regression          | Selected Playwright screenshots after harness validation                                    | Stable key screens in fixed locale, theme, viewport and scale; review intentional baseline changes. Screenshots supplement behavioral assertions rather than replacing them                                           |
| Native and platform checks | OS automation where reliable, plus documented manual checks                                 | Native file dialogs, plugin-owned WinForms controls, clipboard integration, Korean IME, keyboard/focus, accessibility and DPI. Record evidence per OS/plugin; WebView DOM success alone does not cover these surfaces |

### Windows automation feasibility

Launch the actual desktop executable with test-scoped WebView2 remote debugging,
then try Playwright's `chromium.connectOverCDP` against its local endpoint. First
prove attach, a button-to-Rust state change, an actual Host result, a second
window and process cleanup on the pinned Dioxus/WebView2/Playwright versions.
CDP has lower fidelity than Playwright's own protocol, so do not assume every
Playwright feature or artifact is available until exercised.

The test runner's JS/TypeScript is external automation, not an application-owned
JS bridge. Do not add application JS hooks to bypass ordinary UI interactions.
Keep debugging flags local to isolated test launches and out of normal packaged
startup. A browser-hosted component test may supplement this suite but does not
establish Desktop/Host integration. Linux WebKitGTK needs its own native
feasibility and execution evidence; Chromium CDP results do not establish it.

| Primary reference                                                                                               | Purpose                                                                               |
| --------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| [WebView2 debugging](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/debug-visual-studio-code) | Enabling the WebView2 CDP debugging endpoint                                          |
| [Playwright CDP connection](https://playwright.dev/docs/api/class-browsertype#browser-type-connect-over-cdp)    | Connection API and fidelity/Chromium limitations                                      |
| [usage-rs testing](https://usage.jdx.dev/rust/testing)                                                          | Parser, help and executable-output test helpers; verify APIs for the selected version |

### First regression scenarios

| Scenario                                   | Required evidence                                                                                                                                                                                                             |
| ------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Start and import a known log               | Visible progress and expected encounter/combatant/action values matching direct fixture queries; errors and partial results remain explicit                                                                                   |
| Slow import and cancel                     | Navigation/repaint remain responsive, cancellation is acknowledged and restored Host state is usable; pending state is not cleared merely because a UI future was dropped                                                     |
| Rapid A-to-B selection or session restart  | A deliberately late response for A/the previous session cannot replace the current details                                                                                                                                    |
| Hierarchy and log linkage                  | Tree selection and row activation reach the same context; parent navigation works; same-timestamp actions open the correct source log entry                                                                                   |
| Two secondary windows                      | Windows retain their own encounter context after main selection changes; closing one during a request does not update a dead view or break the other                                                                          |
| Plugin-provided UI                         | Open/select/focus a native settings page, interact and close/reopen without stopping the plugin; test disabled/stale UI requests. Separately verify original CEF show/hide and visible rendering, not only hidden screenshots |
| Change locale/theme and restart            | Both rendered UI and persisted `config.toml` reflect the change; comments and unrelated entries survive saving                                                                                                                |
| Invalid/unsupported or unwritable settings | Visible actionable error, original file preserved, and no silent fallback to OS-standard ACT storage                                                                                                                          |
| Different working directory                | Executable-adjacent `config.toml` and ACT relative paths are still used; parent and child resolve the same intended state                                                                                                     |
| Large tables/logs                          | Correct sort scope, selection and match highlighting while scrolling/filtering; bounded query/DOM behavior and visible omissions/loss counters where applicable                                                               |
| Failure and shutdown with work pending     | Visible failure and completed shutdown or explicit timeout; child processes are reaped and the same disposable state can restart                                                                                              |

### Isolation, synchronization and execution

| Concern           | Rule                                                                                                                                                                                                                            |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Disposable state  | Stage the executable and required runtime/plugin fixtures into a unique temporary application directory per run. Keep `config.toml` and ACT data beside that executable; never write to the developer's production deployment   |
| Async determinism | Use controllable delayed fixtures for races and cancellation; wait for observable UI/operation completion with bounded timeouts, not fixed sleeps. Prefer accessible role/name locators, with stable test IDs only where needed |
| Assertions        | Check values, selection, file contents, exit status and process lifetime. Fix the locale for text assertions and test locale switching explicitly                                                                               |
| Failure evidence  | Capture available screenshots, diagnostic logs and runner errors with fixture identity and app/tool/runtime/OS versions; collect Playwright traces only where the validated connection supports them                            |
| Execution stages  | Run Rust/config/CLI tests in routine checks, a small real Windows Desktop smoke suite once the harness works, and broader native/plugin/platform checks before distribution. These are planned stages, not existing CI jobs     |
| Cleanup           | Close windows, await app/Host shutdown and verify owned child cleanup. On test timeout, terminate only test-owned processes and retain failure artifacts before removing disposable state                                       |

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

| Evidence                                   | Scope and limit                                                                                                                                                                                                                                                         |
| ------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Running Windows ACT 3.8.5.288              | Observed the main tree, skill aggregate and individual-action views, and an open `Encounter Logs` window. Automated input did not reliably reproduce all transitions; these observations are not a completed interaction test                                           |
| Installed executable inspection            | SHA-256 `934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`; matched `binaries/act/Advanced Combat Tracker.exe`. Event handlers and form implementations establish the behaviors below at the code level                                                 |
| Local evidence, excluded from distribution | `target/design-reference-act/`: `act-skill-detail.png`, `act-attack-events.png`, `act-encounter-logs.png`, `act-ui-evidence-20261026.json`, and `current/` decompiled types. These are maintainer reference artifacts, not portable tests or shipped application assets |

### Navigation and activation

The tree and the table are two paths through the same selected data hierarchy.
Selecting a tree node changes the table, its columns, chart and context menu;
activating an aggregate row selects the corresponding child in that hierarchy.

| Selected tree node          | Table contents                     | Row activation, including double-click                                |
| --------------------------- | ---------------------------------- | --------------------------------------------------------------------- |
| Zone (`ZoneData`)           | Encounters                         | Select the encounter child                                            |
| Encounter (`EncounterData`) | Combatants                         | Select the combatant child                                            |
| Combatant (`CombatantData`) | Damage/healing categories          | Select the category child                                             |
| Category (`DamageTypeData`) | Attack/skill aggregates            | Select the attack child                                               |
| Attack (`AttackType`)       | Individual actions (`MasterSwing`) | Open encounter logs and locate the selected action's source log entry |

| Behavior                | Implementation target                                                                                                                                             | Acceptance evidence still required                                                                                              |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Selection and return    | Keep tree selection and the full selected path consistent with table activation. Parent navigation returns one level; ACT then collapses the selected parent node | Reach the same rows through the tree and through repeated table activation; return through all parent levels                    |
| Category coverage       | Preserve parser-provided categories and incoming/outgoing direction, including aggregate `All` rows; four fixed damage/heal tabs must not hide other categories   | Exercise categories beyond the mockup's four tabs and distinguish overlapping aggregates                                        |
| Individual-action table | Show action time, attacker, target, skill, damage and available action-specific columns, rather than substituting aggregate rows                                  | Compare a known skill's action rows and ordering with ACT, including repeated actions at the same timestamp                     |
| Tables and charts       | Provide sorting and level-appropriate columns. Chart context follows the selected node; skill-level action charts differ from category-level contribution charts  | Compare selection, sort order and chart source data at each level; do not reuse the mockup's contribution chart for every level |
| Main-chart double-click | ACT's main chart handler stops graph generation and clears tooltip items; it does not open the action-log window                                                  | Verify this separately from table activation before assigning a replacement gesture                                             |

The installed reference's main handlers are `FormActMain.tv1_AfterSelect`,
`lvDG_ItemActivate`, `btnNavBack_Click`, `cmsTView_Opening` and
`pb1_DoubleClick`. Tree expansion populates children; it must not be confused
with activating an individual action.

### Log window and contextual tools

| Surface                | Implementation target                                                                                                                                   | Reference behavior / acceptance target                                                                                                                                                                          |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Encounter Logs`       | Independent nonmodal window retaining the originating encounter and selected action; opening another window must not replace the first window's context | ACT constructs a new form, copies the encounter's log-entry list, then selects and reveals the entry matching the action's `TimeSorter` / log `GlobalTimeSorter`. Compare two windows from different encounters |
| Log search             | Text and regex search, previous/next match, match highlighting and results-only view                                                                    | Verify searches and selection in both full-log and results-only views                                                                                                                                           |
| Log copying            | Separate all entries, search results, selected entries and combat-log copy scopes                                                                       | Compare copied contents; ACT's combat copy excludes entries of type `0` and `-1`                                                                                                                                |
| Encounter context menu | Log viewer, timeline and encounter VCR entry points, alongside relevant export commands                                                                 | Entry points and secondary-tool implementations are code-inspected in [secondary analysis tools](act-analysis-tools.md); populated runtime comparisons remain pending                                           |
| Combatant context menu | Death, special-attacks and avoidance reports                                                                                                            | Source-inspected flows and calculation cases are recorded in [secondary analysis tools](act-analysis-tools.md); validate populated workflows before claiming equivalence                                        |
| Category context menu  | `Lookup by Combatant`, grouping outgoing actions by target or incoming actions by attacker, with further skill/action drill-down                        | Reference `FormByCombatantLookup.ShowLookup`; validate the secondary tree and table against the same source category                                                                                            |
| Attack context menu    | Spell-recast calculation and its timer handoff                                                                                                          | Reference `FormSpellRecastCalc.ShowMedian`; validate calculations and handoff separately from action-log activation                                                                                             |
| Timeline to logs       | Open encounter logs at the selected time while retaining the timeline window                                                                            | `FormTimeLine.pb1_MouseClick` handles a right-click on the time plot and highlights that timestamp in a new log window                                                                                          |

### Backend prerequisites and delivery boundary

The [secondary analysis investigation](act-analysis-tools.md) expands the
contextual-tool targets with source-confirmed report flows, calculation details,
data prerequisites and comparison cases. It includes the distinction between
VCR's generated interaction layout and actual game coordinates, and between
the recast tool's name and its minimum-interval timer handoff.

Existing `encounters`, `encounter_detail` and `encounter_attacks` queries support
an initial aggregate GUI. They do not provide the individual-action and
encounter-log queries needed to complete the workflow above.

| Required capability               | Completion criterion                                                                                                                                                                                                 |
| --------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Hierarchical queries              | Expose enough zone, encounter, combatant, category and attack identity to select and return consistently without relying on display names alone                                                                      |
| Individual-action queries         | Return bounded owned pages for the selected attack/category, with session-scoped identity and visible stale/removed/error handling                                                                                   |
| Encounter logs and action linkage | Retrieve retained logs for the originating encounter and link an action to its source entry by identity/order, including same-timestamp records. A live bounded subscription is not a historical encounter-log store |
| Retention and bounds              | Define log/action retention, page limits and behavior when data is unavailable or omitted; never present an incomplete log as complete                                                                               |
| Secondary windows                 | Keep each window's originating context through main-view selection changes; reject obsolete requests across session restart and report unavailable source data                                                       |
| Supporting reports                | Identify the data/API requirements for timeline, target lookup, death/avoidance reports, VCR and recast calculation before implementing those surfaces                                                               |

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

| Screen                            | Observed state                                                                                                                                                                                                                                                                      | Capture                    |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------- |
| Options / Miscellaneous           | Full settings category tree, log status and open/pause/close controls, independent original-line and autosave policies, contextual Help area                                                                                                                                        | `act-options-misc.png`     |
| Custom Triggers / Triggers        | Empty trigger list; regex, None/Beep/WAV/TTS choices, category/zone restriction, timer or tab name, independent Trigger timer/Add Results Tab flags, Add/Edit/Remove, preview and Benchmark CTs entry                                                                               | `act-custom-triggers.png`  |
| Plugins / Plugin Listing          | Scrollable listing with names, status text, enabled checkboxes, move controls and remove icons; file selection, plugin-info area and loaded-assembly report entry. Separate plugin-owned tabs coexist above the listing, including FFXIV, OverlayPlugin and other installed plugins | `act-plugins-list.png`     |
| Import/Export / Import a Log File | Task tree lists raw-log/clipboard/ACT imports and ACT/HTML/SQL-ODBC/XML exports. Start/end calendar/time selectors or file-boundary choices, import-trigger-results flag, continue-to-combat-end flag, file selection and Help area                                                 | `act-import-log.png`       |
| History Database                  | Zone/title search scope, date/time search and calendar, empty results list, checked-result import/delete controls, history-saving control and age-based deletion control                                                                                                            | `act-history-database.png` |
| Notifications                     | Existing Old Log File notification with title, message, time and Show control; Mark all as read, Clear read and badge-blinking control                                                                                                                                              | `act-notifications.png`    |
| About                             | Version 3.8.5 / Release 288, data-folder and publisher links, update controls and feedback form with optional return email and error-log attachment                                                                                                                                 | `act-about.png`            |

| ACT tab          | Installed implementation and workflow                                                                                                                                                                                                                                                                                                     | aftercast implementation target / acceptance evidence                                                                                                                                                                                                                                                                        |
| ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Options          | Left category tree; right controls and contextual help. Search matches control text and help text and highlights matching controls and category nodes                                                                                                                                                                                     | Searchable settings navigation with discoverable help. Verify setting application, validation, persistence and restart behavior per setting; a generic settings form is insufficient                                                                                                                                         |
| Custom Triggers  | Category/trigger tree with enable checkboxes; selecting a trigger loads its regex, category, sound action, timer/tab name and flags. Add/Edit validates the regex. Actions include none, beep, WAV and TTS; category restriction, timer activation, tabbed results, sound preview and XML sharing are separate controls                   | Preserve editing, per-trigger/category enablement, zone restriction, timer/result-tab behavior and explicit preview actions. Compare save/reload, invalid regex handling, category toggles and XML round trips with controlled logs; editor UI is not proof of matching equivalence                                          |
| Plugins          | Plugin Listing contains file browsing, Add/Enable, per-plugin enabled/status controls, removal and ordering. Selecting a listing shows metadata; View Loaded Assemblies writes a report into the info area. Enabled plugins receive their own tab and status label via `InitPlugin`; disabling invokes `DeInitPlugin` and removes the tab | Separate host-owned plugin management from plugin-owned controls. Keep prerequisite initialization order, failures and readiness visible. Validate disable/re-enable, saved order and settings, dependency failures and native plugin UI separately; constructor or initialization return alone does not establish readiness |
| Import/Export    | Left import/export task tree, task-specific controls and help. Import paths include raw logs, clipboard snippets and ACT files; export paths include ACT, HTML, ODBC and XML table data                                                                                                                                                   | Keep raw parsing, encounter interchange and report exports distinct. Show input/output scope, progress, cancellation and failures. Compare formats and side effects separately; existing XML/gzip support does not establish clipboard/HTML/ODBC/table-XML parity                                                            |
| History Database | Text search over zone names/encounter titles, date/time search, calendar with matching dates, checked result selection and Import Checked. History saving and retention/deletion controls are separate from searching                                                                                                                     | Distinguish querying the index, selecting results and restoring/reimporting data. Verify date boundaries, zone/encounter selection, unavailable source logs and restart persistence. Source-log reimport is a separate capability from the existing archive checkpoint restore                                               |
| Notifications    | Notification history entries have title, message, time and read state, with an optional Show callback. Controls mark all read, clear read entries and configure badge blinking                                                                                                                                                            | Provide readable error/notification history with explicit entry actions and read state. Verify per-entry action, unread badge and clear-read behavior. `HostHandle::diagnostics` is a bounded diagnostic source, not an implementation of ACT's notification lifecycle                                                       |
| About            | Version and update controls, application-data-folder link, publisher links and feedback form with optional email/error-log attachment                                                                                                                                                                                                     | Show aftercast's own version/runtime information and data location. Define aftercast-owned update/feedback policy before adding those workflows; do not reuse ACT publisher endpoints. Network and submission behavior remains untested                                                                                      |

### Options families to retain in the inventory

These are functional groups from `PopulateOptions`, not a requirement to copy
every legacy label or use the same final visual layout. Parser/plugin-defined
controls and platform-specific settings need their own validation.

| Family                    | Reference scope and examples                                                                                                                                                                                                                                                        |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| General and sound         | Miscellaneous host behavior, sound settings, TTS engine selection, regex pronunciation corrections and a speech test                                                                                                                                                                |
| Import/export preferences | Import/export behavior, XML sharing and XML subscriptions; distinct from the Import/Export task tab                                                                                                                                                                                 |
| Main tables               | General refresh/number formatting/sort behavior, encounter timing/culling and separate column configuration for zone, encounter, combatant, category and individual-action tables. Category-table settings include checked columns, reordering and primary/secondary sort selection |
| Output/display            | Graphing, mini-parse, text exports, ODBC, web server and HTML output                                                                                                                                                                                                                |
| Appearance                | Colors/fonts, application controls, graphs and miscellaneous color rules                                                                                                                                                                                                            |
| Selective parsing         | Player list and modes separating display/export filtering from recording only actions involving listed players; these modes affect different data boundaries                                                                                                                        |
| Data correction           | Miscellaneous corrections, combatant renaming and ability redirection                                                                                                                                                                                                               |

### Cross-tab workflows and backend gaps

| Workflow                       | Required design and implementation boundary                                                                                                                                                                                                                                                                                     |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Raw-log import                 | Start/end can be file boundaries or date/time bounds, with an option to continue to combat end. Importing custom triggers is explicitly limited to tabbed results in the reference. Show the separate progress window, cancellation and character/viewpoint context; do not imply existing full-file import covers every option |
| Encounter/report export        | ACT export has an include-log-lines option. HTML selects table/graph depth and image dimensions; table XML selects data depth. Preserve explicit scope and report unsupported formats/options before output                                                                                                                     |
| History reimport               | `ThreadHistoryImport` resolves source logs using record metadata, requests a file when no match is found, and parses the selected time range. Existing `save_history` / `restore_history` stores and restores archived graph data instead; add source-log location/range handling before claiming this workflow                 |
| Trigger output                 | Result tabs, spell timers and audio previews connect trigger management to other surfaces. Preserve output context and errors; headless trigger/audio support does not provide the editor, tabbed results or timer-definition editor                                                                                            |
| Plugin UI                      | `PluginListingPanel` supplies a plugin-owned `TabPage` and status label. Current Rust create/initialize/deinitialize calls cover lifecycle operations, not a rendered plugin-tab bridge or a complete management/query API                                                                                                      |
| Settings and history ownership | Classify settings as GUI-owned, library-owned or plugin-owned. Define supported persistence and restore boundaries before wiring controls; original ACT history saving is not an automatic policy already supplied by aftercast's explicit checkpoints                                                                          |

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

| Entry and action                                        | Observed result                                                                                                                                                                                                 | Implementation target and remaining validation                                                                                                                                                                                                                               | Local evidence                     |
| ------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------- |
| Plugins / Plugin Listing: View Loaded Assemblies        | The existing right-hand Plugin Info area becomes a scrollable report with ACT and plugin assembly sections, identities, versions and paths. The listing remains visible; no separate report window was observed | Support an in-place diagnostic report without changing enablement/order. `FormActMain.btnGetAssemblies_Click` also logs the report; the log write was code-confirmed, not independently checked                                                                              | `act-button-loaded-assemblies.png` |
| Custom Triggers: Benchmark CTs...                       | Opens a separate Custom Trigger Benchmark window with an encounter tree, Restricted/Category/Custom Trigger/Time Spent columns and Perform Benchmarking. Current trigger list is empty                          | Preserve encounter/log context and the distinction between opening the tool and running a measurement. `ShowBenchmark` uses `Show()`, not a modal dialog; closing normally hides the shared form in the reference                                                            | `act-button-trigger-benchmark.png` |
| Benchmark: Perform Benchmarking with insufficient input | An Invalid Selection message box asks for an encounter with more than 1,000 log entries; no measurement result appears                                                                                          | Preserve visible validation before work. Code checks a selected EncounterData and rejects log counts at or below 1,000; the exact guard branch behind this observed warning was not independently identified. Valid measurement, running state and completion remain pending | `act-button-benchmark-invalid.png` |

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

| Priority / workflow                          | Reference behavior and current boundary                                                                                                                                                                                                                                                                                                                                                                                                          | Minimum implementation target / acceptance example                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P0 — Retained encounter logs                 | Live/import dispatch now records original or processed text after the parser hook, including start/end lines, with per-encounter count/text bounds and persisted omission counts. Owned Rust action/log pages and an inline GUI source browser now expose exact links and omissions; independent nonmodal log windows now reuse the owned query browser; native interaction validation remains pending                                           | Define encounter association, original/processed text policy and bounded retention before adding historical queries. Import a known encounter and open an action's source after the live subscription has advanced; show unavailable/omitted data explicitly                                                                                                                                                                                                                                |
| P0 — Action/source identity through restore  | Private history version 1 now preserves swing `TimeSorter` and log `GlobalTimeSorter`; invalid or legacy private payloads are rejected. ACT interchange still reconstructs sequential log sorters. Automatic recording and inline GUI action/source lookup now use preserved encounter-scoped identities; independent nonmodal windows now retain their originating encounter and selected source; native interaction validation remains pending | Preserve or explicitly reconstruct a validated relation in the appropriate format, with a policy for older archives lacking that relation. Two source lines with the same timestamp must remain distinguishable; never silently select a different entry                                                                                                                                                                                                                                    |
| P0 — Plugin configuration access             | Windows registration already attaches native plugin pages to a child-owned `TabControl`; this is more than a status label. Typed status/show operations and GUI entry points now select/focus a shared child-owned nonmodal window. Linux reports unsupported; native Windows execution remains pending. Configuration is plugin-owned                                                                                                           | Implement the [plugin-provided UI plan](#plugin-provided-ui-implementation-plan): a separate child-owned WinForms settings window, typed Host show/select operations, and original CEF overlay windows. Verify native input, focus, close/reopen and lifecycle behavior                                                                                                                                                                                                                     |
| P0 — Plugin enable/retry lifecycle           | A current instance can initialize only from `Constructed`; failure and deinitialization leave terminal `Failed`/`Stopped` states. Re-enabling cannot call initialize on the old `PluginId`. Creating a replacement does not unload the old assembly or undo arbitrary plugin effects                                                                                                                                                             | Configured enablement is saved separately from path-keyed runtime instances. GUI enable/disable now creates/cleans individual instances in the same running worker, preserving combat/history; DeInit failure is reconciled against terminal status. Startup failure still awaits worker cleanup before a fresh startup retry. Linux synthetic fixtures check repeated toggles, missing disabled callbacks, failures and stable encounter/log identities; native Windows UI remains pending |
| P1 — Trigger editing and saving              | Managed/Rust revision-checked CRUD/options persist before runtime publication. GUI edits preserve dirty drafts; new drafts safely rebase on refresh, while existing edits require review of saved values or conversion to a new trigger.                                                                                                                                                                                                         | Synthetic Host/controller tests cover duplicate/stale edits, failure and session ownership. Native Windows/GTK clicking remains pending; see [trigger workflow](triggers.md).                                                                                                                                                                                                                                                                                                               |
| P1 — Trigger result records                  | Live/import matching now emits bounded capture/source results before audio cooldown and import-effect suppression. GUI exposes sequence pages, omissions and explicit clearing. Results are transient per Host session.                                                                                                                                                                                                                          | Import defaults off and never starts audio/live timers. Source text and sorter are captured before OnLogLineRead mutations. Actual native input/window QA remains pending.                                                                                                                                                                                                                                                                                                                  |
| P1 — Timer definitions versus running timers | ACT saves/loads spell definitions, including enablement, timing, restrictions, cues and display fields. Managed definition snapshots, atomic standalone XML persistence and startup/shutdown restore now exist; the existing runtime/frames are reused. Session-owned Bridge/Rust management and the existing Rust/UI editor are connected; see [timer contract](timers.md)                                                                      | Managed tests cover strict restore, stale/colliding keys, failed writes and retained countdown identity. Rust/controller tests cover draft preservation, conflicts, session guards and paging. Native timer editor clicks remain Windows QA; enabled definitions and transient frames remain distinct. Raidboss timelines are a separate feature                                                                                                                                            |
| P1 — Plugin-defined table data               | Managed registrations retain incoming/outgoing categories and default visible columns for the five analysis levels. Current typed Rust aggregate queries do not expose the registration metadata or arbitrary plugin-defined cell values                                                                                                                                                                                                         | Define bounded metadata/value queries and sorting behavior if these columns are supported. A plugin-added category/column must be discoverable without hard-coding its label; execute plugin formatting/comparison on the host owner thread                                                                                                                                                                                                                                                 |
| P1 — History discovery and import meaning    | Managed metadata pages now support scope/date filters and snapshot-bound selected checkpoint restore, with bounded aggregate XML and writer coordination. Bridge/Rust and History GUI remain pending; see [discovery contract](history-discovery.md). Raw-log reparse is still separate.                                                                                                                                                         | Connect the managed discovery/selection service to the shared host operation lifecycle and GUI. Preserve private identities and invalidate stale window/selection state. Source payload presence is not raw-log availability; source-range reparse remains unfinished.                                                                                                                                                                                                                      |
| P1 — Actionable notifications                | The managed notification model, session-owned Rust API and Notifications tab support bounded history, omission counts and owner-thread Show callbacks. See [notification contract](notifications.md). Diagnostic snapshots remain separate. A diagnostic list alone does not implement ACT's notification callback behavior                                                                                                                      | Synthetic managed/Rust/controller checks cover the Show contract. Validate native callback effects on Windows; mark-all, clear-read and badge-blinking controls remain separate history work                                                                                                                                                                                                                                                                                                |

For a new visual comparison, retain B's visual direction and show a connected
set: hierarchy/detail, individual actions plus an independent log window,
plugin listing plus access to a plugin-owned page, and trigger definitions plus
result records. Mark unavailable capabilities as planned rather than implying
that current host APIs already support them. Exact ACT button placement,
settings-search highlighting and incidental warning wording are not acceptance
requirements.

| Maintained source evidence                                                                                                                                                                                                                | Scope                                                                                                                                                                |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Live dispatch](../managed/ActCompat/LogHost.cs), [import dispatch](../managed/ActCompat/RootImport.cs), [log subscription](../crates/aftercast/src/logs.rs), [encounter XML](../managed/ActCompat/EncounterXml.cs)                       | Recording boundary, bounded original records and archive sorter behavior                                                                                             |
| [Plugin lifecycle](../managed/Bridge/PluginSession.cs), [host environment](../managed/ActCompat/HostEnvironment.cs), [Rust plugin API](../crates/aftercast/src/plugins.rs)                                                                | Child-owned native tabs, terminal instance states, trigger startup/shutdown persistence and managed notification callbacks, session-owned Rust transport and Show UI |
| [Trigger execution](../managed/ActCompat/CustomTriggers.cs), [trigger model](../managed/ActCompat/CustomTrigger.cs), [trigger persistence](../managed/ActCompat/TriggerPersistence.cs), [timers](../managed/ActCompat/FormSpellTimers.cs) | Managed execution versus missing GUI management/result/persistence capabilities                                                                                      |
| [Registration views](../managed/ActCompat/RegistrationViews.cs)                                                                                                                                                                           | Plugin-defined categories and default column metadata                                                                                                                |

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

| Stage                 | Delivery                                                                                                    | Evidence required                                                                                                                                                      |
| --------------------- | ----------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Platform feasibility  | Minimal Dioxus window using Host directly; choose/lock GUI dependencies at implementation                   | Windows native message/control operation while the GUI remains responsive; Linux WebView startup; hidden windows do not affect host progress                           |
| First usable workflow | Runtime/plugin selection, initialization, UTF-8 import, progress/cancel, encounters/details/attacks         | Fixture totals match direct queries; partial-import/error states and stale selection handling                                                                          |
| ACT analysis workflow | Full hierarchy, individual actions, action-linked nonmodal log windows and contextual tools described above | Compare against pinned ACT with a known corpus; verify double-click/keyboard activation, parent navigation, same-timestamp log linkage and independent window contexts |
| Live/history          | Existing file-following results, bounded refresh, end combat, explicit history checkpoints and ACT XML/gzip | Live results match host queries; restart restores the last checkpoint                                                                                                  |
| Distribution          | Reproducible GUI packaging and prerequisites                                                                | Packaged launches, keyboard/focus/scaling checks and platform-specific plugin evidence                                                                                 |

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

| Candidate                                   | Example / intended outcome                                                                                                        |
| ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Evidence-linked analysis agent              | Explain a death and open its contributing actions/source logs; distinguish observations from inferred causes and unavailable data |
| Cross-attempt comparison                    | Align comparable encounter segments and explain changes in actions, targets and survival time with inspectable evidence           |
| Trigger authoring assistant                 | Draft a rule from selected logs, inspect matches against historical encounters, then review before applying                       |
| Configuration and troubleshooting assistant | Inspect plugin state, dependencies and diagnostics; propose reviewable changes with a clear recovery path                         |
| Reusable analysis tasks                     | Save a chosen filter/comparison/report workflow for repeated use after encounters                                                 |
| External agent integration                  | Consider exposing the same bounded application operations to external agents through MCP                                          |
| Supporting analysis UX                      | Evaluate segment selection, bookmarks/notes, configuration history/undo and restoring or sharing analysis context                 |

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

### Running plugin toggle boundary (2026-10-27)

The ACT 3.8.5.288 `PluginListingPanel.cbEnabled_CheckedChanged` enable branch
creates a new plugin object and tab, then calls `InitPlugin`. Disable removes the
tab, calls `DeInitPlugin`, drops the object and disposes the tab without restarting
ACT. The GUI now exposes that individual-instance lifecycle in the existing
worker; it does not discard the host's combat graph or automatically restart after
a failed toggle. Stop remains available during a pending toggle and invalidates
its eventual UI result. Structural edits (path/type/order) remain idle-only.

Assemblies remain process-owned and cached by path/hash in aftercast, so replacing
a loaded DLL requires an explicit host stop/start. Fresh instances do not promise
a fresh assembly/static state or repair arbitrary plugin side effects. Dependencies
remain the plugin/operator's responsibility, as with an individual ACT toggle;
this does not add an automatic dependency cascade. A failed initialization remains
terminal and a retry constructs another instance. A throwing DeInit still leaves
a terminal, cleaned instance; its error stays visible while enablement reflects
its queried status. Save persists the resulting configured selection.

Linux tests exercise repeated observer enable/disable alongside an initialized
parser, unchanged worker session and existing encounter IDs, source-log identity,
no observer callbacks while disabled, failed initialization/retry and throwing
cleanup. Windows WebView2 test scenarios include navigation during enable and
saved disablement after restart, but actual clicks, WinForms focus/input and
Windows execution remain unverified on ser7.

### Independent encounter log windows (2026-10-27)

| Boundary         | Implemented behavior                                                                                                                                                                                                                                                                                   | Validation boundary                                                                                                                                                                                                                                                        |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Original context | Open Log Window captures the originating HostHandle, EncounterId, current source selection and page. Each open creates another nonmodal Dioxus window with its own browser state; main selection and other windows do not rebind it                                                                    | Synthetic and Windows scenarios distinguish two source lines at the same timestamp; repeated open/close and different-encounter selections are covered by the Windows test definition                                                                                      |
| Existing UI      | Local Rust/UI-derived Button and Accordion, Catppuccin styles and the existing action/log browser are reused. Window locale/theme are captured at creation                                                                                                                                             | No WinForms bridge or new UI dependency is introduced                                                                                                                                                                                                                      |
| Retention        | Existing paged encounter_actions/encounter_logs APIs provide owned results, exact source identity, empty-state and omission counts. Refresh reads the same encounter's currently retained data; this is not an immutable full-log snapshot                                                             | Existing managed/Rust tests cover count/text bounds and restored source identity; this does not prove vendor disk recording is lossless                                                                                                                                    |
| Stale references | Stop and restore invalidate all previously opened window contexts before proceeding; pending replies cannot repopulate cleared selections. Missing encounters or known worker failures clear stale state. Fresh windows must be opened from Analysis; old windows never attach to the new worker/graph | Restore failure conservatively leaves old windows invalidated; completion also invalidates a window queued during restore. Main-window Open is disabled while the host is busy. Window selection changes remain local; a history checkpoint alone does not invalidate them |
| Ownership        | Closing a secondary window drops only its local UI/query tasks, never the Host owner. Main app quit closes all secondary roots through a plain shared lifetime token, including a window queued before quit but mounted afterward                                                                      | Creation is awaited outside the WebView callback. Failure is reported in the originating browser; unmount cancels only that error handler                                                                                                                                  |
| Remaining QA     | Linux unit/build/contract checks and supported app startup are separate from native interaction                                                                                                                                                                                                        | ser7 CUA still exposes apps:[]; actual secondary-window clicks, focus, close/reopen and Windows WebView2 execution are not claimed as passed                                                                                                                               |

### Linux continuation inventory after writer boundary work

| Next scope                     | Linux-feasible work                                                                                                                                                                                                                           | External validation or decision                                                                                                                                                      |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Vendor recording               | Private I/O diagnostics and incomplete stop are now explicit; known live threads poison worker reuse. Improve recording/recovery only with an explicit acknowledgement and producer-quiescence contract; current observer never repairs files | A durable journal, replay/deduplication, flush policy and resource/rotation bounds need a separate design. Process/OS/disk-crash losslessness is not established                     |
| Custom triggers                | Rust list/edit/delete/validate/save operations and existing UI components can expose managed triggers; preserve key replacement/collision semantics, import restrictions and restart persistence                                              | Native audio/input QA remains platform-specific                                                                                                                                      |
| Trigger results                | Add bounded timestamped result records and source context without importing live audio/timer effects                                                                                                                                          | Windows presentation/input testing follows implementation                                                                                                                            |
| Timer definitions              | Expose definition list/edit/save/load separately from running countdown frames; use synthetic timing/round-trip tests                                                                                                                         | Native spell-window behavior and actual audio remain Windows validation                                                                                                              |
| History discovery              | Query saved checkpoint metadata and select restores explicitly instead of interpreting saved aggregates as source-log reparse                                                                                                                 | Native file dialogs/window interactions remain manual platform QA                                                                                                                    |
| Plugin columns / notifications | Adapt existing category/default-column metadata; determine the inspectable notification and optional Show callback contract                                                                                                                   | Notification Show behavior and retention require a concrete supported contract, not an implicit claim of full ACT notification equivalence                                           |
| Existing UI and native plugins | Keep Rust/unit/contract/bundle checks runnable on Linux                                                                                                                                                                                       | ser7 apps:[]/missing accessibility bus blocks actual secondary-window click/focus QA. Real WinForms/CEF input, Windows capture/reconnection and FF Logs acceptance remain unverified |
| Partial WinForms bridge        | Planning only                                                                                                                                                                                                                                 | Separate explicit implementation request is required; no Mono/Wine introduction is authorized                                                                                        |

These are remaining scopes, not a claim that all Linux ACT parity work is complete.

### Follow-up policy decisions after trigger backend work

| Topic              | ACT-grounded recommendation                                                                                                                                                                                                                                             | Product choice or technical work                                                                                                                                                                                                                                       |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Recording baseline | Keep the reused vendor behavior plus explicit I/O/incomplete-stop reporting and unsafe-worker rejection. Do not silently introduce a journal, replay or stronger durability default                                                                                     | This is the current approved compatibility scope                                                                                                                                                                                                                       |
| Stronger recording | A journal must sit at record admission before the vendor can dequeue; a Host observer that sees already written/followed lines cannot recover upstream loss. Recovery needs acknowledged commit points, checksums and partial-write/deduplication handling              | Admission/recovery mechanisms are technical work. Allowed loss window, producer latency, disk budget and backpressure policy are actual product choices for a separately requested stronger guarantee                                                                  |
| Notification Show  | ACT stores title/message, callback and sender; only the user's Show click invokes the callback on the original UI owner thread. Disposed Control targets disable Show; callback exceptions are logged and disable the action. Successful callbacks may be clicked again | This contract is sufficiently specified by ACT and can be implemented on Linux without waiting for a product decision: session-owned IDs, bounded inspectable records, owner-thread dispatch and stale-ID rejection. Native callback effects remain platform-dependent |
| Next Linux scopes  | History discovery, plugin columns and remaining notification history controls; native trigger/notification UI validation remains on Windows                                                                                                                             | Partial WinForms rendering remains planning only, pending an explicit implementation request                                                                                                                                                                           |

Evidence: ACT 3.8.5.288 `FormActMain.NotificationAdd` and
`NotificationHistoryItem.btnShow_Click`; these recommendations do not claim native
Windows interaction or crash-lossless recording has been validated.
