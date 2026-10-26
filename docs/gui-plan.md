# Desktop GUI plan

The GUI is a future consumer of the asynchronous `aftercast` library. Host
ownership, bounded command admission, Windows STA, message pumping, import jobs
and shutdown now belong to the library; the GUI must not add another host thread
or duplicate its command queue.

| Owner                | Responsibility                                                               |
| -------------------- | ---------------------------------------------------------------------------- |
| Dioxus executor      | Components, selection, filters, pending state and visible errors             |
| `Host`               | Session lifetime, playback error receiver and explicit shutdown completion   |
| `HostHandle`         | Cloneable Send + Sync access to typed asynchronous operations                |
| Internal host thread | CLR/plugin lifecycle, native messages, ACT callbacks, timers and graph state |
| UI state             | Owned encounter summaries/details/pages, session and selection generations   |

Normal flow: UI action → await HostHandle method → owned result → Dioxus state.
Create the complete plugin batch before initialization so automatic startup
completion cannot precede construction of addons.

| UI behavior       | Contract                                                                                           |
| ----------------- | -------------------------------------------------------------------------------------------------- |
| Startup           | Report runtime/deployment/plugin failures before enabling host actions                             |
| Import            | Use start_import, show byte/record progress, request cancel explicitly and wait for restored state |
| Refresh           | At most one automatic refresh in flight; suppress conflicting work and display Busy errors         |
| Stale responses   | Discard responses belonging to an obsolete session or selection                                    |
| Playback failures | Consume Host::next_audio_error and display failures without pretending speech succeeded            |
| Shutdown          | Await Host::shutdown; close the window after cleanup or present an explicit waiting state          |
| Blocked plugin    | A callback can delay cancellation/shutdown indefinitely; no forced in-process termination          |
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
