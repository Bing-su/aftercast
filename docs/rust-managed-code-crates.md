# Rust and managed-code crate research

Research date: 2026-10-20. This records earlier alternatives for creating an ACT
compatibility assembly without C#/F# source. The approved design now uses F# and
netcorehost; the alternatives below are not implicit dependencies.

## Assembly generation and interop alternatives

| Candidate | Snapshot | Findings and limits |
| --- | --- | --- |
| [dotnetdll](https://docs.rs/dotnetdll/0.3.0/) | 0.3.0, 2026-06-24; Rust 1.85.1; GPL-3.0-or-later | Direct metadata/CIL/PE creation. Does not provide runtime hosting or ACT semantics. Framework/ACT output and licensing need evaluation |
| [dotscope](https://docs.rs/crate/dotscope/0.9.1) | 0.9.1, 2026-09-05; declared Rust 1.95; Apache-2.0 | ECMA-335 reading/editing and CIL encoding. Examined examples start with an existing assembly; independent new-PE creation unverified |
| [rustc_codegen_clr](https://github.com/FractalFir/rustc_codegen_clr) | Experimental rustc backend; MIT OR Apache-2.0 | Managed class generation is WIP; nightly/rustc-dev toolchain. Not a first product dependency |
| [dotbridge](https://github.com/devzolo/dotbridge-rs) | Upstream workspace 0.1.0, Rust 1.70, MIT | build.rs publishes DotBridgeBootstrap.csproj. C# bootstrap conflicts with strict no-C# implementation |
| [inline_csharp](https://docs.rs/inline_csharp/0.1.1/) | 0.1.1; .NET 8+ SDK | C# in a Rust macro remains C# and requires compilation |

Sources: [dotnetdll manifest](https://raw.githubusercontent.com/nickbclifford/dotnetdll/master/Cargo.toml),
[dotscope workspace](https://raw.githubusercontent.com/ATRAPSLLC/dotscope/main/Cargo.toml),
[dotscope package](https://raw.githubusercontent.com/ATRAPSLLC/dotscope/main/dotscope/Cargo.toml),
[dotbridge build](https://raw.githubusercontent.com/devzolo/dotbridge-rs/main/crates/dotbridge/build.rs).
The dotscope README also includes a Responsible Use Policy. Its analysis-oriented
emulation with BCL stubs does not establish execution support for a Windows plugin.

Generating an assembly still leaves exact identity, inheritance, mutable fields,
static/instance signatures, event accessors, valid IL, exception handling, GC
roots, callback lifetime, and thread affinity to implement. Recording a public key
in metadata is not producing a valid signature with the original private key.
A plugin writing LogLineEventArgs.logLine needs the actual managed field and its
changes reflected in downstream processing.

## Hosting candidates

| Crate | Stable snapshot / publication | License / declared Rust minimum | Role |
| --- | --- | --- | --- |
| [netcorehost](https://docs.rs/netcorehost/0.22.0/) | 0.22.0 / 2026-07-26 | MIT / unspecified | Preferred modern hostfxr wrapper |
| [hostfxr-sys](https://docs.rs/hostfxr-sys/0.15.0/) | 0.15.0 / 2026-07-26 | MIT / unspecified | Already used by netcorehost; no direct dependency needed |
| [nethost-sys](https://docs.rs/nethost-sys/0.8.0/) | 0.8.0 / 2026-03-22 | MIT / unspecified | hostfxr discovery; not a complete CLR distribution |
| [windows](https://docs.rs/windows/0.62.2/) | 0.62.2 / 2025-10-06 | MIT OR Apache-2.0 / 1.82 | Microsoft CLR hosting API for a Framework comparison experiment |
| [rustclr](https://docs.rs/rustclr/0.3.4/) | 0.3.4 / 2025-12-01 | Registry/manifest MIT / unspecified | Windows x64 CLR and COM reflection; not selected |
| [clroxide](https://docs.rs/clroxide/1.1.1/) | 1.1.1 / 2023-03-31 | MIT / unspecified | Windows CLR execution; older release and separate COM lifetime concerns |

The exact names clrox and dotnet-rs were not established as appropriate general
ACT interop crates in this investigation; this is not proof of absence. rustclr's
README and published license metadata differ, so inspect the actual package
before adopting it. Its Environment.Exit patch is not a normal plugin lifecycle
mechanism. RustClrEnv being !Send/!Sync is relevant to hosting boundaries.

## netcorehost implications

| Observation | Consequence |
| --- | --- |
| initialize_for_runtime_config and assembly delegate loading | A managed static entry point is still needed |
| Default features include nethost-download, net10_0, utils | Disable defaults to avoid the nethost NuGet build download |
| Hostfxr::load_from_path and explicit dotnet root | Use an installed trusted runtime without a nethost build dependency |
| net10_0 selects hosting API surface | Does not install .NET or establish Framework/WinForms compatibility |
| Standard ComponentEntryPoint(IntPtr, int) -> int supported | F# can use this without assuming UnmanagedCallersOnly support |
| Framework-dependent native hosting | Not equivalent to a self-contained publish |

Primary sources: [pinned manifest](https://github.com/OpenByteDev/netcorehost/blob/85358e26871765b029ef56a15d2fd0d249794b1d/Cargo.toml),
[pinned CI](https://github.com/OpenByteDev/netcorehost/blob/85358e26871765b029ef56a15d2fd0d249794b1d/.github/workflows/ci.yml),
[Microsoft native hosting](https://learn.microsoft.com/en-us/dotnet/core/tutorials/netcore-hosting).
Upstream CI configuration is not evidence of ACT support or successful local
Windows execution. The crate declares no MSRV; edition 2024 alone is insufficient.

Context7 was used during implementation for the default delegate flow and lifetime
APIs, then checked against the downloaded 0.22.0 source. Its general examples use
master documentation, so the local versioned API remains authoritative.

## Data buffers and selected support crates

| Choice | Verified behavior / implication |
| --- | --- |
| bon 3.10.2 | Builders; declared Rust 1.88 |
| bytes 1.12.1 | copy_from_slice copies; from_static requires truly static storage |
| Bytes::from_owner | Requires AsRef<[u8]> + Send + 'static ownership; a borrowed CLR pointer is insufficient |
| thiserror 2.0.21 | Typed failures without a public async runtime |
| tracing 0.1.44 | Emit diagnostics; consumer configures subscriber |
| scopeguard 1.2.0 | Context7 guard/into_inner documentation reviewed; used for rollback and active-host release. Destructors do not run after abort/exit |

Version records can be checked in the official sparse index:
[bon](https://index.crates.io/3/b/bon), [bytes](https://index.crates.io/by/te/bytes),
[netcorehost](https://index.crates.io/ne/tc/netcorehost), [thiserror](https://index.crates.io/th/is/thiserror),
[tracing](https://index.crates.io/tr/ac/tracing), [scopeguard](https://index.crates.io/sc/op/scopeguard).
See the [blueprint](blueprint.md) for the approved dependency set and deferred installer.
