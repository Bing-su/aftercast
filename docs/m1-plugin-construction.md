# M1: plugin discovery and construction

Checked on Linux x86-64 / .NET 10.0.13 on 2026-10-20. This extends the
[ACT contract binding slice](m1-contract-binding.md); M1 initialization is still incomplete.

## Implemented boundary

| Capability | Behavior |
| --- | --- |
| Original plugin interface | Advanced_Combat_Tracker.IActPluginV1: InitPlugin(System.Windows.Forms.TabPage, System.Windows.Forms.Label) and DeInitPlugin() |
| Rust entry | Host::create_plugin(PluginConfig::builder().path(path).type_name("Namespace.Plugin").build()) returns opaque PluginId |
| Automatic selection | Omit type_name only when exactly one public, concrete, nongeneric IActPluginV1 has a public parameterless constructor |
| Explicit selection | Resolve only the requested type; avoids inspecting unrelated UI types with unavailable dependencies |
| Dependencies | Dedicated AssemblyLoadContext per main assembly path, AssemblyDependencyResolver and sibling DLL fallback; native dependency resolver supported but not exercised |
| Shared contracts | ACT and WinForms come from the bridge's actual context, which may be outside the default context |
| ACT identity | Require the tested ACT 3.8.5.288 full reference identity; ignore a private ACT copy in the plugin directory |
| Failure | Preserve load/select/construct stage, plugin path and underlying exception message; retain command/status and keep the session usable |
| Lifetime | Host retains constructed instances until stop; no InitPlugin, DeInitPlugin or IDisposable call on these uninitialized instances |

The resolver follows the [official .NET plugin tutorial](https://github.com/dotnet/docs/blob/main/docs/core/tutorials/creating-app-with-plugin-support.md),
queried through Context7. Returning null when no file resolves lets framework
binding and the plugin's Costura AssemblyResolve handler run. This is not an
isolation or security boundary. Constructors can execute arbitrary code, register
global handlers, start threads or crash the process.

Assemblies and their load contexts are cached for the bridge's process lifetime;
host restart creates fresh instances from those assemblies. A changed main DLL
at the same canonical path is rejected using SHA-256. Keep the whole plugin
deployment fixed while running: dependency changes are not monitored. Stop does
not undo constructor side effects or guarantee assembly unloading. See .NET's
[unloading limitations](https://github.com/dotnet/docs/blob/main/docs/standard/assembly/unloadability.md).

## Linux and Windows

| Platform | Runtime and UI binding |
| --- | --- |
| Linux | .NET 10 Core runtime plus our unsigned System.Windows.Forms adapter, version 4.0.0.0; only TabPage and Label exist, and both constructors throw PlatformNotSupportedException |
| Windows | .NET 10 Desktop Runtime requested by the extracted runtime configuration; use real WinForms, never extract the Linux adapter |

This table describes the original constructor experiment. The subsequent
[lifecycle slice](m1-plugin-lifecycle.md) replaces throwing control constructors
with headless status/disposal support and adds explicit initialization/cleanup.

The Linux adapter is owned by the independent
[bxivact-winforms crate](../crates/bxivact-winforms/README.md), including its C#
source, prebuilt DLL and contract tests. bxivact consumes its bytes on Linux;
runtime hosting and AssemblyLoadContext selection remain the host's responsibility.
`mise run test:winforms` checks this managed contract without ACT or a plugin.

The adapter does **not** reproduce the original WinForms strong-name token,
inheritance hierarchy, controls, message loop or UI behavior. Tested modern .NET
resolution binds the original signatures to the shared adapter despite its unsigned
identity. F# could not public-sign the original 16-byte ECMA pseudo-key; no original
private key or genuine reference DLL is shipped as a runtime implementation.
Plugins accessing more WinForms APIs fail visibly. Windows execution remains untested.

The maintainer build references Microsoft.NETFramework.ReferenceAssemblies.net48
1.0.3 only to emit the genuine UI signatures. This compile-only package is private;
its reference DLLs are never copied into the consumer payload.

## Construction ABI

CreatePlugin is a private ComponentEntryPoint delegate, separate from the existing
24-byte lifecycle packet. Integers are little-endian and the whole request is
bounded to 65536 bytes. Rust owns the buffer only for the duration of the call.

| Offset | Size | Meaning |
| --- | --- | --- |
| 0 | 4 | Total request length, at least 40 |
| 4 | 4 | ABI version 1 |
| 8 | 8 | Active session identity |
| 16 / 20 | 4 each | Absolute path UTF-8 offset / length |
| 24 / 28 | 4 each | Type name UTF-8 offset / length; zero length selects automatically |
| 32 | 8 | Output instance identity; valid only on success |
| 40 onward | variable | Strict UTF-8 text without NUL; Rust limits type name to 4096 bytes |

Offsets must be outside the header and within the allocation; lengths are checked
by subtraction to avoid overflow. Status 0 means construction succeeded, -1 means
a caught managed error available through ReadError. No managed object reference
crosses the boundary. PluginId pairs the session with the returned instance ID.

## Evidence and reproduction

| Check | Result |
| --- | --- |
| Good C# fixture | Multiple instances and fresh instance after host restart |
| Invalid selection | Ambiguous automatic selection, missing type, open generic, missing default constructor rejected |
| Constructor exceptions | Underlying fixture exception reported; later construction succeeds |
| Dependency failure | Missing sibling DLL reported; copying the DLL restores construction without stopping the host |
| Contract sharing | Invalid private Advanced Combat Tracker.dll ignored; fixture still binds to the host contract |
| Changed main DLL | Atomic replacement on Linux is rejected before using the cached assembly |
| Linux UI attempt | TabPage construction fails explicitly |
| ABI checks | Null/short/oversized headers, version/session, overflowed ranges, negative length, header overlap, NUL and invalid UTF-8 rejected; success clears diagnostics |
| Original FFXIV plugin | Explicit root type selected and constructed on Linux using the bundled replacement ACT and WinForms adapter |

Run `DOTNET_ROOT=/usr/lib/dotnet mise run test:plugins` for the fixture and boundary
checks. The C# dependency fixture prevents JIT inlining of its value getter so
construction genuinely requires the separate dependency assembly in Release.

The original constructor experiment uses these supplied, ignored artifacts:

| Artifact / member | Pinned value |
| --- | --- |
| FFXIV_ACT_Plugin.dll version | 3.0.3.3 |
| SHA-256 | af1031cdba8eeba8f53dd9d618e58d62d4919bfa77db9b31c568bd04cb359093 |
| Type | FFXIV_ACT_Plugin.FFXIV_ACT_Plugin, TypeDef 0x0200001e |
| Constructor | 0x06000104; 12-byte IL, Object constructor then Costura.AssemblyLoader.Attach |
| Constructor IL hash | f07558e1844d16b55dc499d36143cef1de841589cd84351e174a0db0ec4e4e90 |
| Costura Attach | 0x06000147; registers AppDomain.AssemblyResolve once through an Interlocked guard |
| Retained static evidence | binaries/analysis/sdk-3.0.3.3-core-evidence.json, selected by main DLL hash and method token |

The constructor was inspected before execution. It registers a global resolver;
InitPlugin remains the boundary for later host/UI/capture work. Reproduce with a
new extraction directory whose parent already exists:

```sh
sha256sum binaries/FFXIV_ACT_Plugin_SDK_3.0.3.3/FFXIV_ACT_Plugin.dll
cargo run -p bxivact --example plugin -- /usr/lib/dotnet /private/new-bridge \
  binaries/FFXIV_ACT_Plugin_SDK_3.0.3.3/FFXIV_ACT_Plugin.dll \
  FFXIV_ACT_Plugin.FFXIV_ACT_Plugin
```

Successful output reports a constructed PluginId and explicitly says InitPlugin
was not called. No file renaming or flattening of the supplied SDK was needed
for this root constructor. Proprietary binaries are neither committed nor packaged.
The isolated, offline Cargo package also constructed this original plugin with a
failing dotnet CLI shim first on PATH; the runtime path was supplied explicitly.
This verifies the new consumer path without a managed SDK invocation.

## Remaining M1 work

| Area | Still required |
| --- | --- |
| Host environment | The lifecycle slice adds selected globals, registration and data paths; most ACT callbacks remain absent |
| Linux UI replacement | Research actual accessed members and semantics; signature binding is insufficient |
| Lifecycle | Fixture InitPlugin/DeInitPlugin work in the lifecycle slice; original FFXIV initialization and plugin-owned partial cleanup remain incomplete |
| Runtime checks | Windows execution and original ACT differential behavior |
| Later milestones | Import, capture, aggregation, high-volume streams, persistence and FF Logs acceptance |
