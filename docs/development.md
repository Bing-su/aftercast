# Development and validation

Build and test commands are defined in [mise.toml](../mise.toml).
Consumer behavior is described in [usage](usage.md), with source ownership in
[architecture](architecture.md).

## Requirements

| Role                 | Requirements                                                                                      |
| -------------------- | ------------------------------------------------------------------------------------------------- |
| Consumer             | Rust toolchain and installed .NET 10 runtime; no SDK                                              |
| Maintainer           | Rust, mise and .NET 10 SDK; global.json permits the latest installed stable 10.0 feature band     |
| Real CLR tests       | DOTNET_ROOT points to the trusted .NET 10 installation                                            |
| Original comparisons | Explicit local proprietary ACT/FFXIV artifacts matching the pinned hashes; excluded from packages |

Cargo.toml and clippy.toml define the Rust lint policy; verification treats
warnings as errors. The license is undecided and workspace publication is disabled.

## Everyday verification

```sh
# Use the actual installed runtime root so the fixture integration tests exercise it.
DOTNET_ROOT=/usr/lib/dotnet mise run check
```

On Windows, use PowerShell with the installed x64 Desktop Runtime:

```powershell
$env:DOTNET_ROOT = 'C:\Program Files\dotnet'
mise run check
```

The repository's `NuGet.Config` selects nuget.org independently of machine-wide
offline feeds. Windows tests initialize their owner threads as STA and run against
the native Desktop Runtime; independent WinForms model contracts remain separate.

| Included check              | Scope                                                                                                                              |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Managed build               | Locked restore, Release publish and refreshed embedded Cargo payload                                                               |
| WinForms contracts          | Independent control/chart identity, ownership, dispatch and explicit unsupported behavior                                          |
| Managed bridge              | Native header validation, sessions, end combat, import/log/snapshot requests and reentrancy                                        |
| Managed synthetic contracts | Log following, initial snapshot, combat graph/actions, import restoration, owned output and lifetime checks; no original artifacts |
| Real CLR hosting            | Startup errors, managed diagnostics, active ownership, stop/drop/restart                                                           |
| Fixture plugins             | Construction, initialization failures, cleanup, import, logs and snapshots through Rust                                            |
| Rust                        | Format, strict Clippy, all workspace tests and doctests                                                                            |

`cargo test --workspace` remains the runtime-independent subset. Seven integration
scenarios are ignored there and run explicitly through the unified check.
Rust tests use [rstest](https://docs.rs/rstest/0.28.0/rstest/) cases and fixtures.
The deployment fixture keeps temporary files alive across host restart while
each scenario owns its Host. Exhaustive truncated-prefix checks test every prefix.

## Targeted commands

| Command                                                 | Purpose / inputs                                                                       |
| ------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `mise run managed:restore`                              | Intentionally refresh NuGet lock state after dependency changes                        |
| `mise run managed:build`                                | Rebuild WinForms and publish the portable bridge into Cargo payload files              |
| `mise run winforms:build`                               | Rebuild the independent forms/chart payload                                            |
| `mise run managed:test`                                 | Bridge and synthetic ACT contracts                                                     |
| `mise run test:winforms`                                | Independent WinForms and chart contracts                                               |
| `mise run test:hosting`                                 | Real CLR lifecycle and diagnostic tests; requires DOTNET_ROOT                          |
| `mise run test:plugins`                                 | Managed plugin requests and Rust fixture integration; requires DOTNET_ROOT             |
| `mise run test:log-load`                                | Synthetic buffer bounds, exact subscriber loss and latency/allocation observations     |
| `mise run package:check`                                | Build both offline Cargo packages, using the extracted unpublished WinForms package    |
| `mise run smoke -- /usr/lib/dotnet /private/new-bridge` | Extract, start, probe and stop; the destination's parent exists and destination is new |

The managed runner's `bridge` and `contracts` modes require no original ACT
artifact. Its `act` mode additionally runs the separately compiled original
reference probes. Genuine ACT assertions and expected calculations are independent
of the replacement implementation.

## Original-artifact verification

| Command                                                               | Required input and evidence                                                                                     |
| --------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| `mise run test:original`                                              | Check reference binding, original constructors/values/actions and shared contracts against both implementations |
| `mise run test:external-plugin -- <assembly> <type> <state>` | Check explicit trusted plugin construction, lifecycle and host diagnostics; see [additional plugin results](additional-plugin-validation.md) |
| `mise run test:text-exports` | Compare unaligned exports against genuine ACT on Windows; replacement probe only elsewhere |
| `mise run test:external-export -- <assembly> <state>` | Exercise Export_All through two combat ends and native clipboard output; restores prior clipboard |
| `mise run test:linux-plugin-compat`                                   | Verify pinned memory/writer/resource corrections against local FFXIV SDK files                                  |
| `mise run test:original-import -- <plugin-copy> <new-state>`          | Original synthetic import, failure recovery and private diagnostics; use a disposable trusted plugin copy       |
| `mise run test:original-corpus -- <plugin-copy> <new-state> <corpus>` | Three pinned Zeromus imports with submitted-action totals and exact subscriber loss                             |
| `mise run test:original-log -- <plugin-copy> <new-state> <log>` | Three local-log imports; verify actual line counts, action-derived totals, snapshots and subscriber loss; save private replay transcripts |
| `mise run test:recorded-actions -- <transcript>`                      | Compare captured actions, actor totals, allies and encounter totals in both ACT implementations                 |
| `mise run test:original-writer -- <sdk-deployment>`                   | Watchdog-protected original writer child processes; characterize flush/cancellation behavior                    |

See [compatibility](compatibility.md) for reference inputs, Linux corrections and
comparison limits.

## Deployment and package ownership

| Owner               | Files / rule                                                                                                                   |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| aftercast           | Prebuilt bridge, ACT compatibility, runtime configs and official legacy permission/extension dependencies with bundled notices |
| aftercast-winforms  | Forms/chart source, independent tests and prebuilt assemblies; Linux-only Rust dependency                                      |
| Cargo extraction    | Select the platform runtime config and Windows extension image; write only into a new private destination                      |
| Managed native seam | Standard ComponentEntryPoint, private versioned packets, bounded writes and immediate managed error capture                    |
| Maintainer          | Rebuild checked-in DLLs after managed changes; consumers never invoke dotnet during Cargo build                                |

WinForms source builds from its extracted Cargo package without root managed
props. Package verification uses that extracted WinForms package as the host's
dependency. The temporary patch path in package:check contains version 0.1.0;
the host archive excludes the generated lockfile for that patch.

Existing extraction directories are never rewritten. Deploy a rebuilt payload to
a fresh directory and retain its files for the process CLR lifetime. There is no
build.rs, automatic runtime download, native UI substitute or consumer NuGet restore.

## Verification limits

The unified check passes on Windows x64, including CLR lifecycle, fixture-plugin
initialization, native callback dispatch, import, logs and snapshots.
See the [Windows validation results](windows-validation.md) for commands and limits.
Original Windows plugin import/lifecycle results are recorded in the
[original-artifact report](windows-original-validation.md) and
[local-log report](local-log-validation.md). Visible UI rendering, broader encounter families, native Linux capture, sustained live
throughput, full recording durability/rotation and FF Logs acceptance remain
unverified. Tests of subscriber loss do not establish lossless recording.
