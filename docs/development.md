# Development and validation

The prebuilt F# lifecycle bridge runs inside a Rust process. M1 adds the first
[ACT contract-binding slice](m1-contract-binding.md), but does not yet initialize
the original FFXIV plugin successfully, parse logs, aggregate combat, stream batches, or install
.NET. See the [blueprint](blueprint.md) for later
milestones and the original-artifact research reports for compatibility work.

## Requirements

| Role | Requirements |
| --- | --- |
| Rust consumer | Rust toolchain plus installed .NET 10 runtime; no .NET SDK |
| Maintainer rebuilding managed assets | .NET SDK 10.0.113 (global.json allows latest patch), Cargo and mise |
| Local verified environment | Ubuntu 26.04 x86-64, Rust 1.99.1, .NET SDK 10.0.113, runtime 10.0.13, mise 2026.10.14 |

The workspace and every Rust member inherit the user-provided Clippy lint policy
from Cargo.toml and clippy.toml. Warnings are errors in validation tasks. New code
must not silence these with allow attributes. The project license is undecided,
so workspace publication is disabled rather than inventing a license.

## Commands

| Command | Purpose |
| --- | --- |
| `mise run managed:restore` | Intentionally refresh maintainer NuGet restore/lock state when changing managed dependencies |
| `mise run managed:build` | Locked restore and portable Release publish into the Cargo payload |
| `mise run winforms:build` | Independently rebuild the prebuilt DLL owned by bxivact-winforms |
| `mise run test:winforms` | Check WinForms signature identity and explicit UI failures without ACT or the bridge |
| `mise run managed:test` | Build, stage both crates' DLLs, then verify ABI validation, session ownership and the independent WinForms contract |
| `mise run test:act-binding` | Check the pinned local ACT hash, compile the legacy-reference probe, and run it against the replacement; requires the original local ACT artifact |
| `mise run test:linux-plugin-compat` | Check the pinned FFXIV memory IL adjustment, unknown-build rejection and original-file preservation; requires supplied FFXIV artifacts |
| `DOTNET_ROOT=/usr/lib/dotnet mise run test:plugins` | Build fixtures; check plugin request bounds and construct instances through the Rust API, including dependency and constructor failures |
| `mise run check` | Rust formatting, Clippy with warnings denied, ordinary tests |
| `DOTNET_ROOT=/usr/lib/dotnet mise run test:hosting` | Run the explicitly ignored real CLR integration test on this Linux installation |
| `mise run smoke -- /usr/lib/dotnet /private/new-bridge-directory` | Extract the bundled bridge, start/call/stop it; the parent must exist |
| `mise run package:check` | Verify both isolated Cargo packages offline, resolving the unpublished WinForms dependency from its extracted package |
| `cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings` | Cross-check Windows Rust code; does not link/run a Windows program |

Use your actual .NET root and application-controlled deployment directory. The
smoke example deliberately leaves the extracted component on disk; a loaded CLR
can still need component files after an ACT session stops. It refuses existing
directories. Reuse deployed files by passing their directory directly to HostConfig,
or extract into another new versioned directory for another process.
After a payload update, extract the new payload into a new directory; M0 deployment
directories do not contain the renamed ACT assembly or the ReadError entry point.

The default test suite skips real CLR integration because consumers may not have
a runtime installed at test time. Running test:hosting is required to claim real
hosting validation. The standard tests do not run original ACT or the original
plugin; the separate [FFXIV constructor experiment](m1-plugin-construction.md)
executes the plugin constructor only.

## Consumer interface and trust

| API | Behavior |
| --- | --- |
| extract_bridge(directory) | Creates a new private Unix directory, writes embedded assemblies/config/license, rolls back on failure, refuses to overwrite an existing directory |
| HostConfig::builder() | Requires explicit dotnet_root, bridge_directory and private writable data_directory paths |
| Host::start(config) | Acquires exclusive host ownership, selects the highest stable .NET 10 hostfxr in that installation, loads the standard managed delegate, starts session |
| Host::session_id() | Makes a real managed call to validate the current component-local session identity |
| Host::create_plugin(PluginConfig) | Selects and constructs a trusted IActPluginV1, returns an opaque session-scoped PluginId; never calls InitPlugin |
| Host::initialize_plugin(PluginId) | Calls InitPlugin once, returns bounded status text; rejects exceptions reported to the host during the callback |
| Host::deinitialize_plugin(PluginId) | Calls DeInitPlugin once for a successfully initialized instance; unregisters and disposes owned controls even on failure |
| Host::stop() / Drop | Ends session; hostfxr and CLR remain process-owned |

Public paths use camino 1.2.6: configuration fields and path-bearing errors contain
`Utf8PathBuf`, and `extract_bridge` accepts `AsRef<Utf8Path>`. String callers remain
supported. Existing `std::path::PathBuf` callers must convert explicitly with
`camino::Utf8PathBuf::try_from(path_buf)`; borrowed paths can use
`camino::Utf8Path::from_path(path)`. Neither conversion replaces invalid bytes.
Canonicalization uses `canonicalize_utf8`, so a UTF-8 symlink resolving to a
non-UTF-8 target returns `Error::Io` with `InvalidData` and the requested UTF-8 path.
Runtime discovery still skips unrelated non-UTF-8 directory entry names.
Only the OS/native interop edges use OsStr/std paths; .NET receives its existing
platform-native path representation, and plugin requests use `Utf8Path::as_str()`.
See the [camino documentation](https://docs.rs/camino/1.2.6/camino/struct.Utf8Path.html#method.canonicalize_utf8).

Use trusted runtime/component files and an application-controlled parent directory.
The library executes native/managed code; it is not a sandbox for hostile files.
The host is !Send/!Sync to preserve its creation thread. Runtime selection is fixed
once hostfxr is loaded; switching the installation requires a new process.
An unrelated incompatible preloaded CLR is a hosting failure, not a reason to load
a fallback runtime. That external-host case has not been exercised locally.

## ABI v1 lifecycle header

| Offset | Rust / managed representation | Meaning |
| --- | --- | --- |
| 0 | u32 / int32 bit pattern | Header size, exactly 24 |
| 4 | u32 / int32 bit pattern | ABI version, 1 |
| 8 | u32 / int32 bit pattern | Command: start=1, probe=2, stop=3 |
| 12 | u32 / int32 bit pattern | Managed output: runtime major |
| 16 | u64 / int64 bit pattern | Component-local session identity |

Rust checks size/offset at compile time. F# uses Marshal reads/writes only after
pointer/length/version checks; it does not retain the pointer. Returned int status
is 0 for success, 1 invalid buffer, 2 ABI mismatch, 3 already active, 4 inactive,
5 wrong session, 6 unknown command, -1 caught managed exception. Rust converts
failures to typed errors and keeps all dynamic-loader handles alive during calls.
This header is private implementation detail, not a stable public C ABI or a
batch/event protocol.

ReadError is a second ComponentEntryPoint delegate. It returns the required UTF-8
byte count and writes nothing when capacity is insufficient. Diagnostics persist
until the next Invoke/CreatePlugin and are read immediately on the owning thread. Rust uses an
initialized 8192-byte BytesMut, validates length/UTF-8, and includes the text with
command/status in Error::Managed. Exception summaries are capped at 2048 UTF-16
code units; this is diagnostic text, not a serialized exception object.

## Managed payload

| File | Purpose |
| --- | --- |
| Bxivact.Bridge.dll | ComponentEntryPoint implementation |
| Bxivact.Bridge.runtimeconfig.json | .NET 10 LatestPatch requirement |
| Bxivact.Bridge.runtimeconfig.windows.json | Cargo extraction uses this Desktop Runtime configuration on Windows |
| Bxivact.Bridge.deps.json | Dependency resolver input |
| Advanced Combat Tracker.dll | Public-signed ACT 3.8.5.288 identity; session scaffold and initial Dnum contract slice |
| FSharp.Core.dll | Locked NuGet 10.0.113 runtime dependency |
| FSharp.Core.LICENSE | Upstream MIT notice, also extracted with the payload |
| System.Security.Permissions.dll | Official 10.0.13 legacy permission type support for older serializers |
| System.Windows.Extensions.dll / System.Windows.Extensions.windows.dll | Transitive 10.0.13 dependency; extraction chooses the platform asset and writes System.Windows.Extensions.dll |
| Dotnet.Compatibility*.LICENSE | Upstream .NET runtime MIT notice and identical third-party notices for both compatibility packages |
| System.Windows.Forms.dll | Linux-only headless control hierarchy, status text and disposal; native handles/rendering unsupported; never extracted on Windows |

The WinForms DLL, F# source and independent contract tests belong to
`crates/bxivact-winforms`. Its only Rust export is `ASSEMBLY: &[u8]`; it has no Rust
dependencies. bxivact depends on it only on Linux, combines its bytes with the
host payload during extraction, and retains managed context selection in the bridge.
Direct F# bridge tests use `target/managed/test-bridge`, staged from both crates
by `managed:test-payload`; the host crate no longer owns a second WinForms DLL.

The bridge preloads its two legacy compatibility assemblies into the default
assembly context: Costura's byte-loaded dependencies do not bind through the
plugin context. They remain loaded after host shutdown. This adds neither CAS
enforcement nor Linux implementations of Windows identity APIs. The publish target
copies the Windows asset to a flat alternative filename and retains upstream
notices; consumers need neither NuGet restore nor the SDK.

The WinForms F# project declares its own build settings and can be rebuilt from
its extracted Cargo package without ACT projects or the root managed props.
Consumers still use the prebuilt DLL and do not run a compiler. The host supplies
the .NET runtime and FSharp.Core; the WinForms crate does not start a runtime.

Because publication is disabled, package verification first packages bxivact-winforms,
then supplies its extracted package with a temporary command-line crates.io patch
when packaging bxivact. This checks the packaged bytes rather than depending on
the source checkout. `package:check` currently names version 0.1.0 explicitly;
update that path when changing the workspace version. The host archive excludes
its generated lockfile so the temporary registry patch does not alter the workspace
lockfile; Cargo still extracts and verifies both packages. Future registry publication
must publish the WinForms dependency before the host. The path-plus-version
dependency follows the [Cargo documentation](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#multiple-locations),
checked through Context7.

Locale satellite assemblies are excluded. F# library diagnostics fall back to
English; FFXIV regional resources are unrelated. Cargo include patterns restrict
the payload to flat DLL/config/license files, even if a maintainer has old locale
folders locally. Managed bin/obj output and original proprietary artifacts are
ignored. There is no build.rs, nethost-download, automatic runtime fetch, or
consumer-side dotnet invocation. bytes is included as requested for the planned
batch milestone and bounds the new plugin request buffer; there is no event-stream API yet.

## Review and validation record — 2026-10-20

| Review pass | Confirmed problem | Repair and regression |
| --- | --- | --- |
| Lifecycle | First start worked, restart after stop failed with hostfxr InvalidArgFailure | Keep hostfxr in a process-lifetime OnceLock; real integration checks stop/restart/drop/restart |
| Failure cleanup | Host ownership and extraction rollback were manually managed | scopeguard owns the active lease and incomplete-directory rollback; tests cover failed startup recovery and no overwrite |
| Packaging | Initial publish emitted translated FSharp.Core satellites | Restrict satellite language and Cargo payload; remove obsolete generated folders |
| Cross-platform | DirBuilder mutation was unused in Windows code | Apply Unix-private permissions in a cfg-gated shadow binding; Windows Clippy rechecked |
| Reproducible payload | SDK assembly metadata appended the current Git revision, making checked-in assemblies change after a commit | Set managed version to 0.1.0 and disable revision suffixes in informational versions |
| WinForms ownership | Moving the DLL left direct F# bridge tests without their colocated adapter | Stage both crates' payloads in target/managed/test-bridge; independent managed tests and Rust plugin construction pass |
| Independent packaging | An unpublished workspace dependency cannot resolve from crates.io during host packaging | Verify the WinForms package first and patch its extracted copy into host verification; the packaged F# project rebuilds without ACT props, and the packaged host constructs the original FFXIV plugin with the SDK CLI blocked |
| UTF-8 paths | PathBuf configuration deferred UTF-8 validation until plugin construction | Use camino across host paths and errors; check Unicode runtime discovery, Unicode bridge/plugin deployment, non-UTF-8 symlink rejection, unrelated OS entry skipping and native NUL rejection |
| Plugin lifecycle | Callback return alone can hide a caught initialization exception; a failing shutdown can strand other plugins | Capture bounded WriteExceptionLog diagnostics during InitPlugin; stop attempts all initialized callbacks and closes the session before reporting aggregate failure |

The [lifecycle slice](m1-plugin-lifecycle.md) records host state, startup contracts,
the maintainer-only public-field publishing step, and the remaining original-plugin
blocker. Use a new bridge extraction after updating the payload. Raw `dotnet build`
does not publish the two original static fields; `mise run managed:build` does.

Context7 was used for netcorehost's standard delegate flow and scopeguard's
Always/into_inner semantics, and compared with the resolved crate source.

Tests cover stable .NET version selection; extraction without overwriting;
missing/invalid runtime configuration recovery; one active host; explicit stop;
Drop cleanup; restart; runtime-installation switch rejection; and managed null,
size/version, unknown command, inactive/stale session checks. Package verification
builds from the extracted .crate rather than relying on managed source projects.
The packaged smoke example also ran offline with a failing dotnet CLI shim first
on PATH: extraction, startup, probe, and stop succeeded without invoking the SDK.

Windows checks compile Rust only. Actual Windows CLR/plugin/WinForms execution,
original ACT comparisons, native Linux FFXIV capture, live throughput, disk
recording behavior, FF Logs acceptance, and runtime-download remain unchecked.
