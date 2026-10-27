# aftercast desktop

This first executable implements the aggregate workflow from the
[GUI plan](../../docs/gui-plan.md). It does not yet implement every planned ACT tab.

| Surface            | Current behavior                                                                                                                            |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Top navigation     | Analysis, plugins, import/export and settings; no application logo                                                                          |
| Analysis sidebar   | Search the current encounter page, expand zones/participants, select parser-owned categories and incoming/outgoing direction                |
| Analysis workspace | Sortable participant/attack aggregates, localized pagination and a collapsible damage-contribution chart below the table                    |
| Accordion          | Shared Rust/UI-style disclosure with native button keyboard behavior, ARIA state and reduced-motion support; used by the explorer and chart |
| Import/export      | Real log-file import with progress/cancellation and explicit history checkpoint save/restore; export UI is not implemented                  |
| Triggers/history   | Dedicated navigation entries remain planned; private history checkpoint actions are available under Import/Export                           |

Attack pages are read asynchronously in chunks of 256 and discarded when the
session or encounter selection changes. Parser categories and ACT `All` rows
remain separate: summing overlapping categories would double-count actions.
Imports belong to the application and continue across tab navigation; returning
to the import pane exposes the current progress and cancellation action.
The graph remains a contribution overview; it does not invent a damage time series
from aggregate data. The analysis pane now pages canonical individual actions and retained encounter logs,
links actions by source sorter (never timestamp alone), and displays capacity omissions.
Missing or ambiguous sources remain explicit. Read-only queries can be abandoned;
late results are ignored, while changing encounters or restoring history clears the
source selection. Checkpoint save/restore is available in Import/Export and remains
an application-owned operation across tab navigation. Separate nonmodal log windows,
timer/mini-window controls and a full history search UI remain future work.

| Command                    | Purpose                                                                                 |
| -------------------------- | --------------------------------------------------------------------------------------- |
| `mise run gui:css`         | Install with Bun's frozen lockfile; rebuild the embedded Tailwind/Catppuccin stylesheet |
| `mise run gui:run`         | Launch the desktop app                                                                  |
| `mise run test:gui`        | Run app unit/CLI checks and real Windows WebView2 workflows                             |
| `mise run gui:bundle`      | Build the release executable with its pinned .NET runtime embedded                      |
| `mise run test:gui-bundle` | Test verified downloads, offline cache use and runtime extraction                       |

Use Bun 1.4.4 for frontend tools. `bun.lock` is the frontend lockfile;
Tailwind executes on Bun. `bun run test` invokes Playwright's Node-based runner
because direct execution on Bun currently fails to load this suite.

| Cargo target family      | Embedded .NET frameworks | Native prerequisites                                      |
| ------------------------ | ------------------------ | --------------------------------------------------------- |
| Windows MSVC (x64/arm64) | Core and Windows Desktop | WebView2                                                  |
| Linux GNU (x64/arm64)    | Core                     | GTK3, WebKitGTK 4.1 and the runtime's system dependencies |

`build.rs` uses Cargo's `TARGET`, not the build host, to select the official
archives pinned in `runtime-lock.toml`. It downloads through `ureq`, checks SHA-512,
and caches complete archives under its `OUT_DIR/runtime-cache`. Failed or
interrupted downloads are not published as cache entries. No installed runtime or
Nu packaging script is needed. Cargo builds use the checked-in managed payload;
the release task rebuilds that payload and still requires the .NET SDK.
[`netcorehost/nethost-download`](https://docs.rs/crate/netcorehost/0.22.0/features)
does not supply these runtime archives.

The resulting executable contains the compressed archives and runtime licenses.
Starting the host with the default `dotnet` setting extracts them beside the
executable into `dotnet/<version>-<RID>-<content-id>/`. Extraction runs off the UI
executor, rejects traversal/links and oversized payloads, preserves Unix modes,
and publishes a complete directory while holding a process-shared file lock.
Subsequent starts check only the completion identity and essential runtime files,
then reuse the cache. Archive SHA-512 verification happens during the build;
startup neither rehashes the embedded archives nor scans every extracted DLL.
Older completion markers remain usable. Missing essential files or an invalid
marker are reported without overwriting the cache; close the app and remove only
the reported version directory to rebuild it. Damage within an existing file is
left to .NET loading errors. Other versions, external runtimes and settings remain
intact. Interrupted unpublished extraction directories are not reused.

For an offline build, first populate the matching Cargo build-output cache, then
set `AFTERCAST_RUNTIME_OFFLINE=true` (or `CARGO_NET_OFFLINE=true`). For example in
Nu: `with-env {AFTERCAST_RUNTIME_OFFLINE: "true"} { cargo build -p aftercast-app --offline }`.
Cargo's `--offline` alone does not configure the custom HTTP client. An empty or
corrupt cache fails explicitly with downloads disabled. `cargo clean` removes
these cached archives. Runtime updates require a reviewed lock-file update and
rebuilding the executable; application startup never downloads a runtime.

Development can use an installed runtime by setting `[host].dotnet_root` in
`config.toml` beside `target/debug/aftercast.exe`, for example
`dotnet_root = 'C:/Program Files/dotnet'`. The exact default value `dotnet` selects
the embedded runtime; other values resolve to an external runtime directory.
Relative plugin and log paths also resolve from the executable, independently of
the working directory. Invalid configuration is reported and never silently reset.

| Source                                              | Responsibility and extension boundary                                                               |
| --------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `src/main.rs`                                       | Common CLI parsing, authenticated child dispatch before GUI startup, logging and window setup       |
| `src/app.rs`                                        | Shared async session state and host operations; rejects stale session/selection responses           |
| `src/app/shell.rs`                                  | Navigation, encounters and application actions                                                      |
| `src/app/analysis.rs`                               | Participant/attack aggregates, Chorale tables, collapsible Dioxus chart and log-import pane         |
| `src/app/plugins.rs`                                | Plugin order, type overrides and status; future native settings/overlay actions belong here         |
| `src/app/preferences.rs`                            | Fluent locale, Catppuccin flavour and runtime preferences                                           |
| `src/config.rs`                                     | Validated, comment-preserving TOML and atomic replacement                                           |
| `src/session.rs`                                    | Private bridge deployment and asynchronous host lifetime                                            |
| `src/components/`                                   | Rust/UI button, input, label, separator, outline badge and progress; provenance in `THIRD_PARTY.md` |
| `build.rs`, `build_support.rs`, `runtime-lock.toml` | Target-specific, pinned runtime download, validation and embedding                                  |
| `src/runtime_bundle.rs`                             | Atomic executable-adjacent extraction, completion/essential-file checks and reuse                   |

The chart displays up to 12 positive-damage participants using approximate floating
point values. The table retains exact integer totals. Progress and cancellation are
polled while awaiting the same import job; no additional host thread or queue is added.

The Playwright suite starts the real desktop binary via WebView2 CDP, uses synthetic
trusted plugins, verifies configuration persistence and imports expected totals.
The suite uses a nonexistent global `DOTNET_ROOT`, exercises embedded extraction
and verifies cache reuse across host restarts. Test-only JavaScript is not an application bridge. Captures
are written into `target/`; user plugins and live game integrations need separate
compatibility testing before release.

Bundle checks cover offline build-cache hits, corrupt downloads/build-cache files,
unsupported targets, old completion markers, missing essential runtime files,
archive traversal/links, Unix modes, atomic publication, concurrent
extraction and executing the embedded runtime without a global installation.
The production download/extraction code has also passed its seven payload tests
in a headless WSL Ubuntu harness. Linux GUI startup, ARM64 execution and release
bundles remain separate validation work.
