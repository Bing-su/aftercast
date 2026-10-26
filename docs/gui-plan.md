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

| Stage                 | Delivery                                                                                                    | Evidence required                                                                                                                            |
| --------------------- | ----------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Platform feasibility  | Minimal Dioxus window using Host directly; choose/lock GUI dependencies at implementation                   | Windows native message/control operation while the GUI remains responsive; Linux WebView startup; hidden windows do not affect host progress |
| First usable workflow | Runtime/plugin selection, initialization, UTF-8 import, progress/cancel, encounters/details/attacks         | Fixture totals match direct queries; partial-import/error states and stale selection handling                                                |
| Live/history          | Existing file-following results, bounded refresh, end combat, explicit history checkpoints and ACT XML/gzip | Live results match host queries; restart restores the last checkpoint                                                                        |
| Distribution          | Reproducible GUI packaging and prerequisites                                                                | Packaged launches, keyboard/focus/scaling checks and platform-specific plugin evidence                                                       |

Raw-log viewing must retain subscription bounds and show loss counters. Native
capture, lossless recording, original plugin UI equivalence and upload acceptance
remain separate validation work.

| Reference                        | Purpose                                        |
| -------------------------------- | ---------------------------------------------- |
| [Architecture](architecture.md)  | Library-owned scheduling and lifecycle         |
| [Usage](usage.md)                | Async calls, audio errors and startup batching |
| [Imports](import.md)             | Progress, cancellation and partial history     |
| [Combat queries](snapshots.md)   | Owned results and paging                       |
| [History](encounter-archives.md) | Explicit checkpoints and restoration           |
