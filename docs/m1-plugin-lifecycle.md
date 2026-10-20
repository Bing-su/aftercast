# M1: startup contracts and plugin lifecycle

Checked on Linux x86-64 / .NET 10.0.13 on 2026-10-20. Fixture initialization and
cleanup work. Original FFXIV initialization passes the missing UserControl and
legacy permission-assembly barriers, then fails with "Windows Principal
functionality is not supported on this platform." This is not completion of M1
or parser readiness.

## Implemented slice

| Contract | Behavior |
| --- | --- |
| HostConfig.data_directory | Required UTF-8 path; Rust creates the directory and passes its canonical path to the managed session; existing contents are preserved |
| ActGlobals.oFormActMain / charName | Actual public static CLI fields; root is established lazily before the first InitPlugin attempt, character initially empty, both cleared on stop |
| FormActMain | Form-derived startup object with AppDataFolder: DirectoryInfo and PluginGetSelfData(IActPluginV1) returning the registered instance by reference identity or null |
| ActPluginData | Original public instance fields pluginFile: FileInfo, pluginObj: IActPluginV1, tpPluginSpace: TabPage, lblPluginStatus: Label |
| WriteExceptionLog(Exception,string) | Bounded in-memory diagnostic capture with separate owner-thread initialization counts; no ACT file logging or Rust log delivery |
| RestartACT(bool,string) -> void | Throws PlatformNotSupportedException; an embedded library must not restart the application |
| Linux controls | Control -> ScrollableControl -> Panel -> TabPage; Control -> Label; ScrollableControl -> ContainerControl -> Form / UserControl; Component disposal, Text, Name, IsDisposed |
| Legacy permission binding | Official System.Security.Permissions 10.0.13 plus transitive System.Windows.Extensions 10.0.13; shared with plugin and byte-loaded dependency contexts |
| Unsupported UI | Handle and CreateControl throw; control trees, layout, drawing, timers, handles and message dispatch are absent |

Host globals are unavailable to constructors before the first initialization
attempt. This remains a limitation for other plugins whose constructors use ACT
state. The control hierarchy is a tested headless subset, not complete WinForms
inheritance/interface compatibility. Windows keeps real WinForms and requires a
caller-owned STA thread and UI loop; this implementation never starts a hidden UI
thread. Windows execution is untested. Context7's [thread-affinity documentation](https://github.com/dotnet/docs/blob/main/docs/standard/parallel-programming/potential-pitfalls-in-data-and-task-parallelism.md)
was checked while choosing this ownership rule.

## Public static fields without C#

F# rejects public static val declarations (FS0881) and emits the private source
declarations as internal CLI fields named oFormActMain@ and charName@. Properties
would not satisfy original ldsfld/stsfld instructions. The maintainer-only
PublishFields.fsx reads our published PE metadata and changes just these two access
flags and trailing name suffixes. It checks the owning implementation, the exact
two-field layout, original flags and patched names/flags without executing code.
The step is idempotent and runs after publish in `mise run managed:build`.

This modifies our compatibility DLL, never the original ACT or plugin. It needs
no IL assembler, C# source, extra NuGet dependency or consumer-side compiler.
F#'s source/reference metadata remains private; the published binary contract is
for existing plugin references, not an F# authoring interface to those fields.
Public signing still establishes only tested modern-runtime reference identity,
not an authentic original strong-name signature.

The F# compiler also needs the matching .NET Framework System.Drawing reference
when deriving FormActMain from the original WinForms Form. A compile-only target
replaces the Core drawing facade for that project. No Windows reference assembly
is copied to the payload and no System.Drawing.Common runtime package is added.

The original-reference HostProbe compiles against the supplied ACT metadata and
emits actual field reads/writes and method calls. Running it against the published
replacement verifies both globals, AppDataFolder, void RestartACT binding and
global cleanup. This catches field/property and generic-method/void mismatches.

## Initialization and cleanup policy

| Transition | Result |
| --- | --- |
| Constructed -> initialize | Create owned tab/label, register plugin before callback, call InitPlugin on the host's thread |
| Callback returns without reported exception | Mark initialized and return status text, bounded to 2048 UTF-16 code units |
| Callback throws or calls WriteExceptionLog | Mark failed, unregister, dispose owned controls, return a managed error; never retry that instance |
| Initialized -> deinitialize | Mark stopped before DeInitPlugin; always unregister/dispose; a thrown callback is reported and never retried |
| Constructed / failed -> deinitialize | Reject; do not call DeInitPlugin on a never-successfully-initialized instance |
| Host stop / Drop | Attempt remaining initialized plugins in reverse initialization order, then clear registration/globals even when a callback fails |
| Shutdown callback fails | Report aggregate failure after closing the managed session; release Rust ownership so another session can start |

Treating WriteExceptionLog during InitPlugin as failure is a conservative host
policy. It is not a claim that every original ACT exception log is fatal. Background
thread logs remain in bounded helper diagnostics and are not attributed to the synchronous initializer. Logs outside that
callback do not retroactively change lifecycle state. A returned
status string is not proof that a plugin has started its parser: plugins may
silently decline initialization without throwing or reporting an exception.

Cleanup covers resources owned by this host. It cannot roll back arbitrary
plugin-created threads, global handlers, subscriptions, native state or files
after partial initialization. Assemblies remain process-owned. Fresh instances
can be constructed after failures, but process restart may be needed for a plugin
whose own partial startup left side effects. No combat/capture/FF Logs behavior is
implemented by these startup contracts.

## Private lifecycle ABI

ConfigureHost reuses the bounded 40-byte UTF-8 request layout from construction:
the path is the existing absolute data directory and the type name is empty.
The session allows one configuration before initialization.

PluginCommand requires exactly 8224 initialized bytes, reserving output capacity
before executing a callback. Header integers are little-endian.

| Offset | Size | Meaning |
| --- | --- | --- |
| 0 / 4 | 4 each | Total length 8224 / ABI version 1 |
| 8 | 8 | Active session ID |
| 16 | 8 | Instance ID |
| 24 | 4 | Initialize=1, deinitialize=2 |
| 28 | 4 | Output UTF-8 status byte length |
| 32 | 8192 | Output capacity; no pointer is retained |

Status 0 means the command returned, -1 means a managed error retrievable through
ReadError. Rust checks session ownership before crossing the ABI and validates
output ranges/UTF-8. Error::Managed command identifiers are 5 for configuration
and 6 for plugin commands. The existing lifecycle and construction ABI remain.

## Original-artifact evidence

| Artifact / operation | Evidence |
| --- | --- |
| ACT 3.8.5.288 | SHA-256 934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657; MVID 94f61cd2-1f88-4a7c-98d1-0646897b1fb4 |
| ACT fields/startup methods | REA ev_72669f70e59260819a63d972ecf988dbec62fb1e8988f295387aaca7468d183f |
| Main plugin 3.0.3.3 | SHA-256 af1031cdba8eeba8f53dd9d618e58d62d4919bfa77db9b31c568bd04cb359093; MVID a06abaef-4808-48e0-ab76-f2608b6b9408 |
| Plugin Init/UI selection | REA ev_466be12b273f1fc54ab01403f2b8ce5b55167a1d1edbcfb918365900b948b130 |
| Local retained bundle | binaries/analysis/init-lifecycle-evidence.json; full metadata retained after the large ACT response exceeded the transport budget |
| Supplemental IL | binaries/analysis/init-lifecycle-il.txt, decoded using the existing non-executing PEReader helper |

The selected metadata is complete within its types; user-string coverage has
explicit malformed-terminal-flag issues preserved by REA. Static call order is
not proof that all calls execute on every path.

| Original method | Observed static sequence |
| --- | --- |
| InitPlugin 0x06000105 | 64-bit check, VerifyAssemblyVersions, ConfigureIOC, region/resources, data interfaces, update check, UI/settings, Deucalion, memory, loaded status |
| InitPlugin catch | Calls FormActMain.WriteExceptionLog, sets failure status and returns normally |
| DeInitPlugin 0x06000106 | Clear PluginStarted; stop/update/UI/data/IoC cleanup; unloaded status |
| ConfigureUI 0x0600004c | Find ACT control tree, attach Clear/log callbacks, replace date parser and load settings control |

InitPlugin has 297 IL bytes / 90 instructions, SHA-256
be1e64438308bdf20ed8b4654516a2474b4de34e29d9539d8b88ccd1d1ac0ea1.
Its exception handler covers offsets 32..254 and returns at offset 296. The Linux
runtime attempts report caught failures through WriteExceptionLog; Rust therefore
returns an initialization error instead of treating this return as success.
No successful original initialization, game capture or original DeInitPlugin run
is claimed.

## UserControl follow-up

| Runtime probe, same original plugin hash | Observed outcome |
| --- | --- |
| Add UserControl : ContainerControl | No missing base-type error; InitPlugin reports Definitions.Astrologian.json deserialization failing because System.Security.Permissions cannot load |
| Bundle permissions but resolve only through PluginContext | Same missing assembly: Costura's byte-loaded Newtonsoft.Json binds outside that context |
| Preload official compatibility dependencies into Default | Previous missing assembly is no longer reported; InitPlugin reports unsupported Windows Principal functionality |

The permission package was selected from the official
[10.0.13 NuGet release](https://www.nuget.org/packages/System.Security.Permissions/10.0.13).
Context7's [.NET dependency migration guidance](https://github.com/dotnet/docs/blob/main/docs/core/porting/third-party-deps.md)
was checked; package compatibility still requires runtime validation. The packages
are process-owned and do not restore .NET Framework CAS enforcement or Windows
identity support on Linux. No principal check is bypassed or made to return a
fabricated administrator identity. The exact original Windows principal call site
and the remaining settings/control-tree dependencies still need investigation.

The independent control test checks UserControl inheritance, text, disposal and
native-handle rejection. The lifecycle fixture compiles its UserControl constructor
against the genuine WinForms reference. It also exercises permission construction
and Demand through both file-loaded and byte-loaded dependency assemblies, without
the original plugin. This validates binding, not a security boundary.

Both Cargo packages were verified offline. The extracted host package initialized
and stopped ReadyPlugin with the dotnet CLI replaced by a failing shim, then
reproduced the original plugin's Windows Principal failure in a separate process.
Rust formatting, workspace tests, strict Linux and Windows-target Clippy, managed
ABI/control tests and real CLR lifecycle/construction tests passed. Windows runtime
execution and successful original parser startup remain unverified.

## Reproduce and checks

| Command | Checks |
| --- | --- |
| mise run managed:test | Independent headless-control/disposal tests and bridge ownership/header checks |
| DOTNET_ROOT=/usr/lib/dotnet mise run test:plugins | Construction plus initialization, caught-error reporting, registration, bounded status, failed-init disposal, duplicate rejection, failed-stop continuation and restart |
| mise run test:act-binding | Original-reference Dnum, static fields, host method binding and disposal |
| cargo run -p bxivact --example initialize -- /usr/lib/dotnet /private/new-bridge plugin.dll Namespace.Plugin | Explicitly initialize a trusted plugin; prints status only on successful callback completion |

The initializer example uses a private state directory below its new bridge
directory. Library callers supply their own data_directory explicitly. Original
binary probes require supplied local artifacts; they are not packaged. Rust tests
exercise Windows code by cross-check only; Windows runtime execution remains open.
The isolated Cargo package also initialized and shut down ReadyPlugin with a
failing dotnet CLI shim first on PATH, using only the explicit installed runtime.
