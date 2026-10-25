# Dioxus desktop implementation plan

Status: planned; no GUI or threaded host adapter is implemented by this document.
The target is a Windows/Linux desktop application built with Dioxus Desktop,
using the existing Rust host and managed compatibility layer.

## Decisions and scope

| Decision              | Plan and reason                                                                                                                                   |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| UI                    | Dioxus components, RSX and CSS; use the desktop system WebView renderer                                                                           |
| Application boundary  | Add an unpublished `crates/aftercast-gui` executable; keep GUI dependencies out of the existing library crates                                    |
| Host ownership        | Create, operate and stop `Host` inside one dedicated thread; initialize that thread as STA on Windows before plugin construction                  |
| GUI access            | An application-local, cloneable `HostHandle` sends bounded commands and awaits owned responses; design the handle to be `Send + Sync`             |
| Existing contracts    | Keep `Host` as `!Send + !Sync`, the single active-session lease, trusted plugin loading and process-owned CLR/assemblies                          |
| First usable workflow | Start host, initialize an explicitly selected trusted parser, import a log, inspect encounters and stop cleanly                                   |
| Deferred scope        | Plugin discovery/downloads, full ACT UI parity, chart dashboards, trigger/timer editors, automatic history retention and separate-process hosting |

The adapter initially belongs to the GUI crate. Promote it to a reusable library
only when another consumer needs it; do not mirror every public `Host` method.
This plan adds a consumer of the existing APIs, rather than a second combat model.

## Thread and data ownership

| Owner                     | Responsibility                                                               | Boundary                                                                            |
| ------------------------- | ---------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| Dioxus UI thread          | Components, selection, filters, pending operations and visible errors        | Sends commands through `HostHandle`; never blocks waiting for the host              |
| Host thread               | Deployment/startup, plugin lifecycle, `pump()`, import, queries and shutdown | Creates `Host` within the thread; never sends it or managed controls across threads |
| Command/response channels | Deliver typed operations and their results                                   | Bounded admission and asynchronous responses; no unbounded refresh backlog          |
| UI state                  | Retain returned encounter summaries, details and attack pages                | Use existing owned Rust result types; do not expose managed references              |

Normal flow: UI action → bounded command → host thread → existing `Host` API →
owned result → Dioxus state update.

| Adapter behavior          | Required contract                                                                                                                                                                                     |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Startup                   | Report deployment/runtime/STA/plugin failures to the UI; enable host actions only after successful startup                                                                                            |
| Scheduling                | Alternate bounded command processing with regular `pump()` calls; timed waiting must wake for pumping even when no commands arrive or the window is hidden                                            |
| Requests                  | Start with plugin initialization, import, end combat, encounter list/detail/attacks, diagnostics and stop; preserve existing validation and errors                                                    |
| Queue pressure            | Reject excess work visibly without blocking the UI; allow at most one pending automatic refresh and reserve shutdown admission independently of ordinary queue capacity                               |
| Long operations           | Serialize imports and conflicting actions, show an indeterminate busy state and suppress refreshes until completion; do not promise progress or immediate cancellation that the host does not provide |
| Responses                 | Complete each admitted request or report worker termination; dropping a response receiver does not cancel an admitted mutation                                                                        |
| Session/selection changes | Discard responses for an obsolete session or selection; keep existing session-scoped encounter ID validation                                                                                          |
| Pump failures             | Surface the error and leave the worker in a defined failed/stopping state; never silently continue as healthy                                                                                         |
| Shutdown                  | Stop admission, resolve queued work as closed, then stop and drop the host on its owning thread; await worker exit without blocking Dioxus rendering                                                  |
| Import during shutdown    | Wait for an already running synchronous import before stopping; never force-kill the host thread or claim a bounded shutdown time                                                                     |

Windows plugin controls remain on the host's STA thread. Dioxus does not convert
WinForms controls into components. Test how existing plugin settings windows are
opened and operated before promising access to them in the application.
Native plugin code still executes in-process; a host thread does not isolate
native crashes or arbitrary plugin failures.

## Delivery stages

| Stage                         | Implementation                                                                                                                                                              | Completion evidence                                                                                                                                                                                                                             |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1. Platform feasibility       | Minimal Dioxus window plus a dedicated host thread, startup handshake, pumping and shutdown; choose and lock the Dioxus version during implementation                       | Windows x64: STA fixture plugin initialization, native callback dispatch and cleanup while the Dioxus window remains usable. Linux: WebView launch, headless fixture workflow and cleanup. Both: hidden/minimized UI does not stop host pumping |
| 2. Host adapter               | Add only the typed commands needed by the first workflow, bounded admission, asynchronous replies, diagnostics and explicit shutdown                                        | Focused checks for queue saturation, pump scheduling under queued work, dropped receivers, startup/worker errors and shutdown completion; compile-time checks that `HostHandle` is `Send + Sync` and existing `Host` restrictions remain        |
| 3. First usable UI            | Runtime/deployment and trusted plugin selection, initialization status, UTF-8 log import, encounter list, participant detail and paged attack aggregates                    | Fixture import totals agree with direct host queries; import failure shows partial-history/error semantics; controls reflect busy/error states; changing selection cannot publish an older detail response                                      |
| 4. Live operation and history | Expose existing file-following workflow where supported, bounded live queries, end combat, explicit history save/restore and ACT XML/gzip import/export                     | Compare displayed live results with existing query APIs; restart, initialize matching parser categories and restore saved history; show that recovery reaches the last explicit checkpoint                                                      |
| 5. Distribution and usability | Add reproducible GUI build/package tasks and Windows/Linux setup documentation; verify keyboard navigation, focus, accessible labels, Korean input, scaling and long tables | Launch packaged builds on declared target environments; record WebView and .NET prerequisites, measured data volumes and known limitations                                                                                                      |

Stage 1 is the integration gate: resolve WebView/WinForms event-loop conflicts
before building the full UI. Check an explicitly selected original plugin after
the fixture passes, and record its exact version and workflow separately from
general compatibility claims.

Start with on-demand paged queries and one bounded live refresh while a live view
is open. Add a shared latest-snapshot store only if measured query pressure
justifies it. Raw-log viewing, when added, must retain the subscription's bounds
and show its loss counters rather than claim durable recording.

## Files and verification affected

| Location                                        | Planned change                                                                                                                                                    |
| ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/aftercast-gui/`                         | Executable manifest, Dioxus entry point, application-local worker/handle, components, assets and focused adapter tests                                            |
| Root Cargo workspace and lockfile               | Verify the existing `crates/*` membership picks up the executable; lock GUI dependencies without adding them to core libraries                                    |
| `mise.toml`                                     | GUI development/build and platform validation tasks; keep library/package checks usable without GUI system dependencies                                           |
| GUI package configuration                       | Include required application assets and existing managed deployment payload; preserve fresh private extraction directories and process-lifetime payload ownership |
| Existing Rust/managed tests                     | Reuse fixture plugins, host lifecycle, import, query and archive checks; add only missing integration coverage                                                    |
| Documentation index, usage and development docs | Record implemented startup/UI workflows, target prerequisites, commands and observed limits as stages land                                                        |
| Compatibility matrix                            | Update only the specific implemented/validated workflows; this plan does not change current support status                                                        |

Run the existing core checks for any core changes and add GUI-specific checks
separately. A working WebView is not evidence of native gameplay capture, lossless
recording, complete ACT compatibility or original plugin UI equivalence.

## References

| Reference                                                                          | Purpose                                                          |
| ---------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| [Host architecture](architecture.md)                                               | Ownership, callback dispatch and managed boundaries              |
| [Host API](../crates/aftercast/src/host.rs)                                        | Thread affinity, pumping and stop contract                       |
| [Combat queries](snapshots.md#gui-combat-queries)                                  | Owned session queries and pagination limits                      |
| [File import](import.md)                                                           | Synchronous import, partial history and live-context restoration |
| [Encounter archives](encounter-archives.md)                                        | Explicit checkpoints and restoration prerequisites               |
| [Dioxus Desktop guide](https://dioxuslabs.com/learn/0.7/guides/platforms/desktop/) | System WebView renderer and native Rust execution                |
