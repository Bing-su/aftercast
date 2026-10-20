# Implementation blueprint

Updated: 2026-10-20. Approved for implementation. This replaces the earlier
separate-process/JSON Lines proposal with in-process native hosting. Research
observations and implementation results remain separate.

## Scope and ownership

| Area | Decision |
| --- | --- |
| Product | A reusable Rust crate first; an application binary and UI come later |
| Existing software | Reuse the FFXIV ACT plugin and its parser; reconstruct the ACT host contract and aggregation |
| Languages | Rust API and hosting; C# compatibility core, WinForms model and bridge |
| Runtime | netcorehost, in-process .NET 10; no automatic .NET Framework fallback |
| Platforms | Windows and native Linux; replacing Windows dependencies on Linux is a research milestone, not a verified capability |
| Maintainer tasks | Cargo workspace and mise.toml; no cargo xtask |
| Consumer requirements | Prebuilt managed assemblies in the crate; no .NET SDK or managed compilation during cargo build |
| First API | Host lifecycle, then plugin registration, log import, original log subscriptions, combat result queries/subscriptions |
| Deferred API | Raw SDK NetworkReceived/Sent byte subscriptions; no mandatory packet round-trip through Rust |
| Runtime delivery | Explicit installed runtime path initially; optional runtime-download feature later; application runtime bundling later |
| License | Undecided; publishing disabled until resolved |

C# owns the mutable ACT object graph, hooks, action queue, aggregation, and plugin
lifecycle. Rust owns the public API, hosting, batches, owned output data, and
external subscriber status. Do not implement aggregation twice or expose mutable
.NET objects as Rust public API values. Missing ACT methods must fail visibly,
not return fabricated success or empty data.

## Workspace

```text
Cargo.toml                 # Keep the host and Linux WinForms compatibility in one workspace.
mise.toml                  # Run maintainer builds, validation, and packaging explicitly.
crates/bxivact/             # Provide the reusable API; no separate sys crate initially.
  managed/                 # Ship prebuilt portable C# assemblies for SDK-free consumers.
crates/bxivact-winforms/    # Supply the Linux WinForms DLL without ACT or hosting dependencies.
  managed/                 # Own the C# implementation, prebuilt DLL, and contract tests.
managed/ActCompat/          # Implement ACT contracts and state in their owning language.
managed/Bridge/             # Translate native commands and batches into managed operations.
managed/tests/              # Check the managed ABI independently of the original plugins.
docs/                      # Keep research and implementation evidence separate.
binaries/                  # Preserve local analysis artifacts outside Git.
```

A future application is a separate workspace member. No UI framework, database,
generic plugin framework, separate aggregation crate, or async executor is
selected initially. Libraries must not install a tracing subscriber.

## Dependencies

Versions were resolved on 2026-10-20. These are not an established minimum Rust
version; the current build uses Rust 1.99.1.

| Crate | Version | Purpose |
| --- | --- | --- |
| netcorehost | 0.22.0 | hostfxr hosting; default features off, net10_0 enabled |
| bon | 3.10.2 | Configuration builders |
| bytes | 1.12.1 | Owned batch buffers and shared byte slices in the batch milestone |
| camino | 1.2.6 | UTF-8 host/plugin paths and path-bearing errors |
| thiserror | 2.0.21 | Typed errors |
| tracing | 0.1.44 | Application-controlled diagnostics |
| scopeguard | 1.2.0 | Release host ownership and roll back incomplete extraction |
| tempfile (development only) | 3.27.0 | Isolated test directories |
| Standard library | — | Initial synchronization and bounded queues; no mandatory Tokio or crossbeam |

The optional installer remains unimplemented. Candidate dependencies are ureq
3.4.2, sha2 0.11.1, zip 9.0.5 (Windows), tar 0.4.46 and flate2 1.1.10 (Linux).
Recheck versions and narrow features when implementing it. It must be disabled by
default, act only on an explicit install request, verify official source/version/
architecture/hash, validate archive paths and links, and avoid exposing partial
installs. Enabling a feature alone must not download anything.

## Runtime and ABI

| Contract | Plan |
| --- | --- |
| Lifetime | One active host; managed sessions can stop/restart, while CLR and hostfxr remain process-owned |
| Runtime selection | Explicit trusted .NET root; report incompatible runtime state instead of silently switching |
| Threading | Keep the host on its creation thread until plugin/UI threading is designed and verified |
| Entry point | Standard ComponentEntryPoint delegate; retain the existing native ABI |
| Commands | Small ordered command surface; no callback per managed getter |
| Batches | DrainBatch into Rust-owned BytesMut storage; offset/length UTF-8 areas and fixed-width integers |
| Validation | ABI version, structure size, offset/length bounds, arithmetic overflow, and buffer capacity |
| Ownership | Native pointers are borrowed for the call only; no managed array/string/Span/bool directly crossing ABI |
| Failure | No managed exception or Rust panic unwinding across the boundary |
| Ordering | Preserve ACT's mutable synchronous hooks and action order internally; exporter order is not automatically game packet order |
| Snapshots | Read a consistent aggregation state rather than enumerate concurrently mutated dictionaries |

Capacity failure must not consume pending events. Determine batch sizes and time
budgets from measured load. bytes does not make borrowed CLR memory zero-copy;
pinning/release protocols or a shared ring require a demonstrated bottleneck.
M0 only implements a fixed lifecycle handshake, not DrainBatch.

## Recording, overload, and FF Logs

| Path | Approved behavior |
| --- | --- |
| Original log file | Prefer the original FFXIV writer; preserve its contents/order without deliberate dropping or sampling |
| Core parsing/aggregation | Continue independently of slow Rust subscribers; distinguish delay from lost input |
| Live Rust log subscription | Bound by item count and bytes; discard oldest pending items on overflow, report missing counts/ranges, and continue |
| Combat snapshots | Replace older pending snapshots with the latest full result |
| Command results/errors | Keep completion and failure observable independently of a full event queue |
| Disk failure | Report recording failure and mark the recording incomplete; do not disguise it as ordinary subscriber lag |

Store loss state outside the same queue that overflowed. External log loss does
not imply core aggregation loss, but external analysis built only from those
logs can be incomplete. Preserve file/session information for later import;
automatic replay and cursor subscriptions are not yet part of the API.

Finite memory, disk capacity, sustained storage speed, and process crashes prevent
an unconditional lossless guarantee. Determine actual writer backpressure and
flush behavior from the plugin rather than reuse the live subscriber drop policy.

For FF Logs, first validate the original file with the official uploader, including
whole-file upload, live logging, encoding, timestamps, required records, flush, and
rotation. Do not reconstruct upload logs from snapshots or a lossy subscription.
Direct upload and account/report integration are later product features; the
[public OAuth/GraphQL API](https://www.fflogs.com/api/docs) is not evidence of a
supported upload API. Sources: [file/live logging](https://fr.fflogs.com/help/start/),
[current FFXIV setup](https://www.archon.gg/ffxiv/articles/help/getting-started),
[official client transition](https://www.archon.gg/wow/articles/news/older-logging-apps-will-retire-june-29th).

## Milestones

| Stage | Deliverable and acceptance |
| --- | --- |
| M0 | Workspace, mise, prebuilt bridge packaging, hostfxr lifecycle; SDK-free Rust consumer startup/call/stop, duplicate prevention, failure recovery, restart |
| M1 | ACT identity/type/field contracts and original plugin initialization/shutdown; begin native Linux dependency replacement experiments |
| M2 | Import and aggregation compared against original ACT with fixed plugin/settings/input; action order, combat boundaries, integers, categories, getters/exports |
| M3 | Batch ABI and subscribers; validate ownership, bounds, shutdown, loss reporting, bounded memory, and core progress with slow consumers |
| M4 | Live capture, original recording, official FF Logs file compatibility; zone/reconnect, disk failure, rotation, sustained traffic |
| M5 | Clean consumer environments and optional installer; explicit download, verification/extraction/failure recovery, platform support matrix |
| Later | Application binary/UI/runtime bundle, OverlayPlugin and cactbot compatibility |

Preserve plugin version/hash, runtime, settings, input hash, and platform for every
comparison. Report latency, queue count/bytes, memory, recording throughput, and
shutdown duration. No throughput or ACT-equivalence claim follows from static
metadata inspection or M0 success. Native plugin crashes can affect the entire
consumer process.

M0 is implemented; see [development evidence](development.md). M1 has an initial
[binary-contract binding slice](m1-contract-binding.md): a public-signed ACT identity
and selected Dnum members used by an original-reference probe. The next slice adds
[plugin discovery and construction](m1-plugin-construction.md), including an original
FFXIV plugin constructor on Linux. Initialization, WinForms UI, native Linux capture,
aggregation, and licensing remain open work.

The [lifecycle slice](m1-plugin-lifecycle.md) adds host globals/registration,
headless startup controls and tested fixture InitPlugin/DeInitPlugin handling.
The original FFXIV InitPlugin returns `Started` on Linux after the
[controls, events and settings slice](m1-controls-and-settings.md) and
[registration/file-following slice](m1-registration-and-log-following.md).
The Rust host provides cooperative owner-thread dispatch through `Host::pump`;
applications must call it regularly. Original parsing,
aggregation, native capture and Windows execution remain open.
