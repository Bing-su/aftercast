# Windows OverlayPlugin/cactbot follow-up — 2026-10-26

Both the managed host and the public Rust process-host path pass the complete
CEF/Edge rendering integration on the current implementation. The Rust-owned
worker repeated the rendered checks across two complete lifetimes after the
Windows canonical-path correction.

## Inputs and coverage

| Input                   | Observed version                                                       |
| ----------------------- | ---------------------------------------------------------------------- |
| FFXIV ACT plugin        | 3.0.3.4, disposable copy with the installed Framework chart dependency |
| OverlayPlugin / cactbot | 0.19.111.0 / 0.37.5.0, existing unchanged local release files          |
| CEF / Edge              | CefSharp 95.7.14 x64 / Edge 156.0.4314.42                              |
| Runtime                 | Windows x64, .NET Desktop Runtime 10.0.14                              |

OverlayPlugin, cactbot and HtmlRenderer DLL SHA-256 values still match the
[pinned inputs](overlay-cactbot-validation.md#pinned-inputs). The host retains its
existing hash-pinned in-memory HtmlRenderer correction; no vendor DLL or JavaScript
was rewritten. CEF was extracted into fresh private state directories.

| Path / check                                   | Result and boundary                                                                                                                                                                                                                    |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mise run test:overlay-integration`            | Pass: original FFXIV, OverlayPlugin phase two and actual cactbot event source, run through the managed test host                                                                                                                       |
| Native CEF and real Edge → original WS         | Pass: unchanged raidboss pages display alerts `1`, `2`, `3`; timeline starts at `Angry Dummy`, jumps to `Three`, and resets to empty                                                                                                   |
| Actual handlers and raw logs                   | Pass: save/load round trip returns 42, real version response, public player event and at least five original LogLine events; no uncaught Edge page errors                                                                              |
| Native renderer                                | Real CEF bitmap captured and inspected; Edge screenshot inspected independently                                                                                                                                                        |
| Native browser audio                           | CEF plays `BigWigs/Alarm.webm` unmuted through completion; no recorded media error. Edge is intentionally muted in this harness                                                                                                        |
| ACT speech/effect routing                      | Original CEF and WS `cactbotSay` requests deliver both expected texts; both `playSound` requests reach ACT. This checks routing, not Rust audio-device playback                                                                        |
| Managed shutdown                               | Pass: ACT globals cleared and cactbot log hook removed                                                                                                                                                                                 |
| Rust public async API → same-executable worker | Pass twice: prerequisite plugins constructed/initialized in order, actual cactbot version response and save/load round trip, zero host diagnostics, successful shutdown, same state directory and WS port reused with a new worker PID |
| Rust worker → CEF/Edge rendered full flow      | Pass twice: native bitmap, alerts, timeline start/jump/reset, real raw logs, original WS handlers and browser audio/routing assertions, followed by Rust-owned shutdown and a new worker using the same state                          |
| Rust regressions / package                     | Pass after correction: unified check, canonical-path regression, Release build and offline package verification                                                                                                                        |

The earlier WS-only worker boundary is superseded by the rendered worker checks
below. An open port or `InitPlugin` return alone is not a readiness check.

## Rendered end-to-end verification inside the Rust worker

The standalone Rust app calls `run_child_if_requested` before creating its runtime,
starts `Host`, and constructs/initializes the original FFXIV, OverlayPlugin and
cactbot plugins through the public API. It then constructs the test-only
`OverlayProcessProbe` plugin in that same worker. The probe reuses the embedded
bridge's existing session and calls the unchanged CEF/Edge assertions in
`OverlayIntegrationChecks`; it does not create a second managed host. No vendor
DLL, browser bundle or private vendor callback was modified.

The test helper now has a mode that leaves session shutdown to its Rust owner.
Its default managed-host entry point retains its existing shutdown behavior.
The probe runs in the ordinary constructor call path because native dispatched
callbacks and plugin deinitialization are unsuitable places to drive live-log
tests. During this synchronous constructor probe, the reused test helper calls
the actual session's `Pump` to drive messages; it does not test sustained idle
scheduling under the outer Rust pump loop. Normal library lifecycle and scheduling
regressions are covered separately by `mise run check`. The production API is unchanged.

| Evidence                                           | First lifetime                                  | Second lifetime  |
| -------------------------------------------------- | ----------------------------------------------- | ---------------- |
| Rust parent PID                                    | 32400                                           | 32400            |
| Actual worker PID                                  | 47772                                           | 9888             |
| Managed owner thread / apartment                   | 2 / STA                                         | 2 / STA          |
| CEF DOM snapshots                                  | Six passed                                      | Six passed       |
| CEF popup text                                     | `1`, `2`, `3`                                   | `1`, `2`, `3`    |
| Timeline start                                     | `Angry Dummy` and four following bars           | Same             |
| Timeline jump                                      | `Two`, `Three`, `Four`, `Six`, `Ten`, `Fifteen` | Same             |
| Timeline reset                                     | Zero bars                                       | Zero bars        |
| Edge LogLine events / page errors                  | 5 / 0                                           | 5 / 0            |
| Host diagnostic errors                             | 0                                               | 0                |
| `Host.shutdown().await` / cactbot log-hook removal | Passed / removed                                | Passed / removed |
| Worker process after shutdown                      | Absent                                          | Absent           |

The CEF and Edge screenshots were inspected; both show the actual alerts and
timeline. The parent asserts that the probe PID equals `Host.process_id()` and
differs from its own PID, checks the STA marker, waits for graceful shutdown, and
preserves both runs' artifacts before reusing the same state directory. The
existing managed contracts also pass after adding this test seam. No further
production-runtime correction was needed for the rendered worker run.

## Reproduced failures and correction

| Failure                                                           | Evidence and correction                                                                                                                                                                                                                                                                                                  |
| ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Constructing all three plugins before initializing prerequisites  | cactbot type selection cannot find `OverlayPlugin.Common`. The passing sequence constructs/initializes FFXIV, then OverlayPlugin, then cactbot; OverlayPlugin initialization installs its resolver. Corrected the Rust API and startup documentation                                                                     |
| Rust worker's CEF GPU helpers repeatedly exit, then kill the host | Windows event log reports CLR 4 `FileNotFoundException`; direct invocation of the identical CefSharp helper with `\\?\...` fails to load `CefSharp.dll`, while its ordinary path avoids the exception                                                                                                                    |
| Canonical Windows paths inherited by legacy helpers               | Simplify canonical paths only when the installed `dunce` implementation considers it safe; retain extended paths when Win32 semantics differ. `dunce` 1.0.5 was already present in Cargo.lock and is now a direct Windows dependency. The regression covers Unicode filenames and retaining a trailing-dot extended path |

Inputs requiring extended paths remain unsupported by this legacy CefSharp helper;
they are preserved rather than silently redirected to another file. Deploy the
CEF/plugin/state directories under conventional Windows paths.

## Reproduction and evidence

| Local path under `target/overlay-cactbot-20261026/`                             | Contents                                                                                                                    |
| ------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `integration.log`, `stack/`                                                     | Full managed integration log, original CEF/Edge screenshots and JSON assertion reports                                      |
| `app/`, `process-probe.cjs`                                                     | Standalone disposable Rust app and Node WS assertions; app calls `run_child_if_requested` before creating its runtime       |
| `process.log`, `process-ready.log`                                              | Batch-construction failure and pre-correction CEF/helper failure                                                            |
| `cef-normal.err`, `cef-extended.err`                                            | Direct helper comparison; ordinary `--help` returns -1, extended path fails with CLR exception exit -532462766              |
| `process-fixed.log`, `process-fixed/probe-1.json`, `process-fixed/probe-2.json` | Successful two-lifetime Rust worker run and actual cactbot handler responses                                                |
| `unified-check.log`, `release.log`, `package.log`                               | Post-correction verification                                                                                                |
| `render-app/`, `child-render.log`, `child-render-summary.json`                  | Initial disposable render harness, successful complete run and independent summary of both lifetimes                        |
| `child-render/run-1/`, `child-render/run-2/`                                    | Separate native CEF and Edge screenshots, six CEF snapshots, browser reports, PID/STA markers, routing and teardown reports |
| `process-render-build.log`, `child-render-contracts.log`                        | Managed test build with zero warnings/errors and passing managed contract suite                                             |
| `review-render.log`, `review-render/run-1/`, `review-render/run-2/`             | Repeat using the checked-in `overlay-process` example, preserving plugin configuration across worker lifetimes              |
| `review-check.log`                                                              | Unified regression verification after review                                                                                |
| `review-packages.log`, `review-settings-preserved.log`                          | All three offline Cargo packages verified; existing OverlayPlugin configuration refused and preserved                       |

The managed command is the existing [reproduction procedure](overlay-cactbot-validation.md#reproduction-and-evidence)
with state `target/overlay-cactbot-20261026/stack` and the current FFXIV copy under
`target/windows-validation-20261026/plugin`. Playwright came from the bundled Node
dependencies; no project npm dependency was added. The Rust app uses a fresh
directory containing `state/OverlayPluginCef/x64` and an available loopback port.

The reviewed runner is checked in as
[`examples/overlay-process.rs`](../crates/aftercast/examples/overlay-process.rs).
It stages only `Aftercast.ManagedTests.dll` in `probe/` so the test resolver binds
to the embedded worker bridge, and refuses to replace existing OverlayPlugin
configuration. Prepare CEF and `NODE_PATH` as above, then run from the repository root:

```powershell
$run = 'target/overlay-cactbot-20261026/new-render-run'
7z x 'binaries/overlay-cactbot-20261021/CefSharp-95.7.14-x64.7z' "-o$run/state/OverlayPluginCef/x64" -y
Set-Content "$run/state/OverlayPluginCef/x64/version.txt" '95.7.14' -NoNewline
mise run test:overlay-process -- 'C:/Program Files/dotnet' $run 'target/windows-validation-20261026/plugin/FFXIV_ACT_Plugin.dll' 'binaries/overlay-cactbot-20261021/OverlayPlugin/OverlayPlugin/OverlayPlugin.dll' 'binaries/overlay-cactbot-20261021/cactbot/cactbot/cactbot/CactbotOverlay.dll'
```

The runtime root, run directory and three trusted plugin assembly paths are
explicit inputs. Substitute your local deployment paths; proprietary inputs are
not checked in. The bridge extraction and configuration must be fresh; choose
another run path when repeating the command. The app performs both worker
lifetimes itself, reloading saved plugin settings on the second startup.

Review found that the original reproduction command depended on an ignored
`target/` project. The checked-in example and mise task remove that dependency;
all runtime/deployment/plugin paths are explicit. Both rendered lifetimes passed
again through this example. Review also verified the refusal to overwrite existing
settings, all-features strict Clippy, the Release build, the unified checks, and
offline packaging of all three crates. Embedded DLL hashes match the current
managed Release build. No unresolved correctness finding remains in the reviewed changes.

Live game memory/packet capture, desktop overlay composition/transparency,
long-running reconnect and Linux execution remain unverified. The full-flow
test supplies player identity via the vendor public event API and follows controlled
raw logs through the real FFXIV plugin. It permits only cactbot's previously
documented false `.NET 4.6` version warning, not other integration errors.
