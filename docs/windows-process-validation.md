# Windows process-isolation validation — 2026-10-26

Native Windows execution of the current async, process-isolated host passes after
the two corrections below. This supersedes the execution-model limits in the
[2026-10-21 report](windows-validation.md), without establishing full plugin or gameplay compatibility.

## Environment and corrections

| Component                  | Observed value                                           |
| -------------------------- | -------------------------------------------------------- |
| Source baseline            | `fe034a8`, plus these corrections                        |
| Windows / Rust             | x64 build 26300 / Rust 1.99.1, `x86_64-pc-windows-msvc`  |
| .NET SDK / Desktop Runtime | 10.0.403 / 10.0.14, x64, under `C:\Program Files\dotnet` |

| Reproduced failure                              | Correction and regression coverage                                                                                                                                                                                                                                                       |
| ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Startup socket error 10035 (`WouldBlock`)       | Explicitly reset accepted sockets to blocking mode: Windows inherits the nonblocking listener mode. Hosting and archive tests failed before the correction and pass afterward.                                                                                                           |
| History restart fails with `UriFormatException` | Open XML files as streams, preserving DTD rejection and size limits. Canonical paths such as `\\?\C:\state\history\index.xml` must not be parsed as URIs. Apply the same correction to trigger and data-correction persistence; managed tests now explicitly use extended Windows paths. |

The managed Cargo payload was rebuilt. The workspace was clean before validation.

## Results

| Check                                                                  | Result / scope                                                                                                                                                                                       |
| ---------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mise run check`                                                       | Pass: managed bridge/contracts, independent WinForms model, CLR hosting, fixture integration, format, strict Clippy, Rust tests and doctests                                                         |
| Process-isolation integration                                          | 13 passed: same-executable launch, missing entry hook, startup timeout, authentication rejection, exclusive state ownership, six stuck callback/disposal cases, Drop cleanup and parent-exit cleanup |
| Async fixture integration                                              | Pass: import cancellation/restoration, lifecycle, logs, snapshots, archive restart/round-trip and child-owned injected audio cleanup/error delivery                                                  |
| Extended-path managed regressions                                      | Pass: triggers, data corrections and history, including malformed XML / DTD rejection                                                                                                                |
| `mise run test:original`                                               | Pass: genuine ACT constructors, values, configured actions and shared contracts                                                                                                                      |
| `cargo build --workspace --release --locked`                           | Pass                                                                                                                                                                                                 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Pass, including the host's native audio feature                                                                                                                                                      |
| Three offline Cargo packages                                           | Pass: package and compile all three crates using extracted local dependencies and `--allow-dirty` for the corrections; existing license/metadata warnings remain                                     |
| Release smoke example                                                  | Pass: fresh Unicode deployment path, same-executable child launch, session probe and shutdown                                                                                                        |
| Native audio                                                           | Two tests passed: SAPI completion/cancellation/recovery and fixture PCM completion/cancellation; Edge service excluded                                                                               |

## Original FFXIV through the public async API

The `initialize` Rust example ran FFXIV 3.0.3.4 from a disposable copy under
`target/windows-validation-20261026/plugin`, including the installed Framework
chart DLL required by the [earlier deployment](windows-original-validation.md).
Each input used a fresh Unicode bridge/state directory and a separate worker process.

| Input                                                      | Observed result                                                                                                                              |
| ---------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Checked-in synthetic fixture                               | 7 records, 1 encounter, 2 combatants, 400 damage, 0 healing; 7 subscribed records delivered, none omitted                                    |
| `D:\repos\cactbot\docs\logs\TheAbyssalFractureExtreme.log` | 104,744 records, 3 encounters, 35 combatants, 122,118,606 damage, 64,058,080 healing; 4,096 subscribed records delivered and 100,648 omitted |
| Both runs                                                  | Successful initialization, returned snapshots, zero host diagnostic errors, successful shutdown and exit                                     |

Corpus totals are observations from one run, not fixed expectations or a new
per-action comparison against genuine ACT. The bounded subscriber overflow
accounts for every record: the example drains after import and is not a recorder.

## Evidence and limits

| Local evidence under `target/`                                                                       | Contents                                                                     |
| ---------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `windows-check-20261026.log`, `windows-check-fixed-20261026.log`                                     | Initial failures and successful unified verification                         |
| `windows-managed-original-20261026.log`                                                              | Managed contracts with explicit extended-path regressions                    |
| `windows-original-20261026.log`                                                                      | Genuine ACT comparisons                                                      |
| `windows-release-20261026.log`, `windows-all-features-20261026.log`, `windows-packages-20261026.log` | Release, features and packages                                               |
| `windows-native-audio-20261026.log`                                                                  | SAPI/PCM tests                                                               |
| `windows-validation-20261026/`                                                                       | Release smoke, original-plugin async imports and disposable deployment/state |

The [OverlayPlugin/cactbot follow-up](overlay-cactbot-process-validation.md)
subsequently reran the browser stack and corrected legacy CEF subprocess paths.
Live game capture, desktop overlay composition/chart rendering,
Edge network synthesis and Linux regressions were not retested.
Audio tests check device/backend completion, not perceived sound quality.
