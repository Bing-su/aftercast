# Windows validation — 2026-10-21

The unified check, offline Cargo packages, release build and smoke execution pass
on Windows x64. Fixture tests exercise native WinForms handles and callback dispatch;
Full original ACT/FFXIV compatibility and visible rendering remain unverified.
See the later [supplied-artifact results](windows-original-validation.md) for
native initialization, numeric comparisons and remaining diagnostic failures.

## Environment

| Component                      | Observed value                                                 |
| ------------------------------ | -------------------------------------------------------------- |
| Rust target                    | `x86_64-pc-windows-msvc`                                       |
| Rust compiler                  | `1.99.1`                                                       |
| .NET SDK                       | `10.0.402`                                                     |
| .NET / Windows Desktop Runtime | `10.0.13`, x64                                                 |
| Runtime root                   | `C:\Program Files\dotnet`                                      |
| SDK selection                  | `global.json`: stable .NET 10.0, latest installed feature band |

The original SDK pin (`10.0.113`, `latestPatch`) rejected the installed SDK.
The updated policy selects `10.0.402` without permitting .NET 11.
Managed restores use locked dependency versions and the repository's
`NuGet.Config`, which selects nuget.org instead of machine-wide offline feeds.
Existing build directories required write access beyond the restricted runner;
fresh Rust output avoided the original `target/debug` access failure.

## Results

| Check                                            | Result | Evidence / limit                                                                                                           |
| ------------------------------------------------ | ------ | -------------------------------------------------------------------------------------------------------------------------- |
| `cargo fmt --all --check`                        | Pass   | Workspace formatting                                                                                                       |
| `mise run check`                                 | Pass   | Managed builds/contracts, independent model, all CLR/fixture scenarios, formatting, strict Clippy, Rust tests and doctests |
| `cargo test --workspace`                         | Pass   | 26 ordinary tests and 2 doctests; unified check also executes all 7 scenarios ignored by this command alone                |
| `cargo build --workspace --release --locked`     | Pass   | Native MSVC release build                                                                                                  |
| Managed bridge Release publish                   | Pass   | Locked restore; no compiler warnings or errors; embedded payload rebuilt                                                   |
| Managed runner and plugin fixture Release builds | Pass   | No compiler warnings or errors                                                                                             |
| Independent WinForms/chart contracts             | Pass   | Linux headless model executed on Windows; does not establish native WinForms behavior                                      |
| Ignored CLR diagnostic and hosting scenarios     | Pass   | Error propagation, invalid configuration, exclusive ownership, stop/drop/restart and Unicode deployment paths              |
| Ignored plugin construction scenario             | Pass   | Type selection, dependency failures, recovery and cached assemblies across restart                                         |
| Ignored import/lifecycle/log/snapshot scenarios  | Pass   | STA fixtures; Windows canonical paths; native owner callbacks also run in lifecycle tests                                  |
| Managed runner `bridge`, `contracts`, `plugins`  | Pass   | Desktop Runtime, STA owner threads, real control handles, end combat, shutdown and reentrant Stop rejection                |
| Offline Cargo package checks                     | Pass   | Both packages extracted and compiled; `--allow-dirty` permits checking the current uncommitted changes                     |
| Release smoke example                            | Pass   | Extracted rebuilt payload to a fresh Unicode path; started, probed and stopped the CLR host                                |

## Corrections

| Failure                                             | Correction                                                                                                                                                                           |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| SDK feature band unavailable                        | Stable .NET 10 SDK feature-band roll-forward                                                                                                                                         |
| Dependencies absent from offline feeds              | Repository-owned NuGet source configuration                                                                                                                                          |
| Plugin initialization on non-STA threads            | Balanced COM STA guards in Rust fixtures; STA managed owner threads                                                                                                                  |
| Managed WinForms binding missing                    | Windows test runtime config uses the real Desktop Runtime                                                                                                                            |
| Native Invoke cannot dispatch                       | The host creates root/tab/status handles before InitPlugin; isolated test controls also create handles; the owner context pumps native messages and guards reentrant lifecycle waits |
| Message pumping changes the synchronization context | Native pumping runs inside the context's save/restore scope                                                                                                                          |
| Windows extended-length path differs                | Subscription assertion compares the canonical source path                                                                                                                            |
| Chart DLL absent from Desktop Runtime               | Separate Linux-model chart check from platform-independent fixture lifecycle checks                                                                                                  |
| Cargo path dependency cannot be packaged            | Explicit version on the workspace WinForms dependency                                                                                                                                |

`Host.pump()` now dispatches Windows native messages as well as queued callbacks
on the caller's STA thread. Native messages are not included in its returned
queue count or the queue's 64-item/5-ms scheduling budget. No hidden UI thread is created.
Native chart rendering is not covered; the independent headless chart contracts
still execute in their separate suite, and through the plugin fixture on Linux.

Review removed fixture-created plugin handles and reproduced a lifecycle failure
(`explicit pump did not release worker`). After preparing handles in the host,
the fixture invokes the root, tab and status control directly from its worker;
the unified check passes without plugin-side handle preparation.

## Reproduce verification

Run from the repository root:

```powershell
$env:DOTNET_ROOT = 'C:\Program Files\dotnet'
$env:CARGO_TARGET_DIR = "$PWD/target/windows-validation/build"
mise run check
cargo build --workspace --release --locked
cargo package -p aftercast-winforms --offline --allow-dirty
cargo package -p aftercast --offline --allow-dirty --exclude-lockfile --config 'patch.crates-io.aftercast-winforms.path="target/windows-validation/build/package/aftercast-winforms-0.1.0"'
```

`mise run check` rebuilds embedded managed DLLs. Package verification retains the
existing warning about undecided licensing and package metadata; publication
remains disabled. No original ACT or FFXIV artifacts are included in the packages.

The later [original-artifact validation](windows-original-validation.md) uses the
supplied 3.0.3.4 SDK and matching corpus found in the neighboring cactbot checkout.
Gameplay/native capture, visible UI interaction and Linux regression execution
remain unverified in this Windows session.
