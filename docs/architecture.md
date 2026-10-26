# Architecture

Rust handles deployment, supervision and owned output. A dedicated child owns CLR,
plugins, audio and the mutable ACT combat graph. `HostHandle` sends bounded commands
to a library IPC thread and awaits owned responses.
Windows STA and native/managed pumping are internal. Domain terms are in the
[glossary](../GLOSSARY.md).

The [Dioxus desktop plan](gui-plan.md) describes a future GUI consumer of the
asynchronous library, not current GUI support.

## Execution and cancellation

| Area              | Implementation and contract                                                                                                                                                      |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Commands          | A 32-entry bounded queue; at most eight commands within a 5 ms scheduling budget per turn; overload returns Busy                                                                 |
| Scheduling        | Host pumps callbacks/native messages, advances long operations and timers, and dispatches audio even without incoming commands                                                   |
| Control           | Shutdown and import cancellation use atomic flags plus an unpark, independent of queue capacity                                                                                  |
| Imports           | Async file reads feed four bounded batches (64 records or about 128 Ki UTF-16 characters plus one maximum record each); owner processing admits at most 64 records/5 ms per step |
| Import completion | Input disposal and accepted actions settle before finalization; partial history survives failure/cancel and live parser context is restored                                      |
| Live timers       | Use the saved live monotonic clock throughout imports, independently of imported timestamps                                                                                      |
| Archives          | Host captures/applies graph state; worker tasks perform disk I/O, hashing and compression; owner-context continuations commit results                                            |
| Conflicts         | Imports/archives exclude lifecycle changes and other mutations; diagnostics and graph/log queries can run between steps                                                          |
| Audio             | Separate child-owned speech/effect workers; factories run inside the process boundary; bounded playback-error receiver belongs to parent Host                                    |
| Failure           | Publish terminal failure before releasing queued response senders; pump failure closes admission and runs cleanup                                                                |
| Drop              | Requests shutdown without joining; independent supervision enforces the same graceful deadline as explicit shutdown                                                              |

No scheduling budget preempts an executing callback, graph serialization or native
call. The compatibility layer retains synchronous ACT events and its sequential
combat worker. Native calls borrow Rust buffers only for each call; no task keeps
a native pointer across a yield. No async ABI or arbitrary closure executor is
public. Futures need an executor but do not require a Tokio runtime.

One optional operation enum represents import, archive or audio retirement; these
states cannot coexist. Audio cleanup futures are polled between owner pump requests,
and shutdown retires audio before disposing the managed owner context. Shutdown
rejects queued requests before draining the active operation.
Audio backend objects cannot capture a parent HostHandle: their factories run in
the child. A stuck factory, playback worker or destructor is covered by the same
process shutdown deadline as a stuck plugin callback.

Host resolves the current application executable and starts it with only the
reserved `--aftercast-child` argument. Application main calls `run_child_if_requested`
before UI/runtime creation, returning on true or error; false enters normal startup.
No dedicated worker executable is shipped. The optional application-executable
override supports embedding test harnesses/launchers. Custom audio factories use
`run_child_if_requested_with_audio` at the same entry point.
Starting another Host from the reserved child mode is rejected before spawning,
so a missing entry hook cannot create a chain of processes.

The child authenticates a loopback TCP connection using a fresh 256-bit token
delivered over its inherited stdin pipe. JSON control metadata and binary ACT
packets use separate length-prefixed frames capped at 128 MiB; stdout/stderr remain
independent plugin output. The parent never loads CLR or executes plugin callbacks.
A supervisor owns the Child handle, checks deadlines independently of IPC and
always waits for process exit after killing. Default startup/shutdown deadlines
are 30/5 seconds. Startup expiry returns StartupTimeout even after authentication;
one supervisor clock covers authentication and CLR initialization. Its thread's
completion also signals exit, without a separate exit flag. Shutdown expiry
returns ShutdownTimeout after reaping; requests
blocked on IPC then fail and a new Host can start. Cancellation of startup and
dropping a Host also request supervised cleanup. A child-held OS file lock prevents
two workers from writing the same state directory; killing the child releases it.
Before plugin execution, Linux workers install a parent-death kill signal and
check for the parent-exit race. On Windows a non-inherited kill-on-close Job Object
is assigned before the child receives its launch configuration. Parent process
exit therefore cannot leave a stuck plugin worker holding that state lock.
Session identities are random across process restarts. No command or plugin
initialization is automatically replayed, and in-memory state can be lost on kill.

The owner parks only when no command, callback or import progress occurred in a
turn. Active input does not pay a fixed delay per batch. Import records that
cross an idle combat boundary remain pending until prior actions settle; their
timestamps are parsed once and input order is preserved. The synchronous managed
adapter retains pumping but uses a bounded wait only when no progress occurred.

Archive continuations use a private owner-context queue independent of the
256-entry plugin callback admission limit. Only one archive operation runs at a
time; its sequential awaits bound the number of pending internal continuations.
Internal completions run before plugin backlog within the same pump budget, so
plugin overload cannot strand core cleanup or fail .NET await scheduling.

## Module map

Speech lives in the independent [aftercast-tts crate](../crates/aftercast-tts/README.md).
[The host adapter](../crates/aftercast/src/speech.rs) drains bounded ACT speech and effect requests
from managed code during Pump. Direct playback backends and inference-only
Synthesizer implementations share worker ownership and cancellation; Playback
adapts CPU PCM/MP3 output without exposing model/tensor types to ACT.
`SoundPlayer` shares worker admission/cancellation with `Speaker`, with separate
queues so effects can play during speech. Timer callbacks only enqueue requests;
device I/O stays on Rust workers. See [audio contracts](audio-validation.md).

The private `aftercast-tts::worker` module owns queue admission, cancellation and
completion handles; speech and effects reuse it through their existing public
types. The host adapter shares FIFO completion, bounded draining and response
validation while keeping speech and effect queues and errors independent. Each
drain reuses one request packet for up to 32 submissions.

| Location                                                                                                                                                                                       | Responsibility                                                                                                                                            |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Host](../crates/aftercast/src/host.rs)                                                                                                                                                        | Public lifecycle operations                                                                                                                               |
| [Runtime types](../crates/aftercast/src/runtime.rs)                                                                                                                                            | Process configuration and IPC request/packet types                                                                                                        |
| [Process](../crates/aftercast/src/process.rs)                                                                                                                                                  | Authenticated IPC, process supervision, deadlines, kill/reap and worker entry points                                                                      |
| [Local runtime](../crates/aftercast/src/local_runtime.rs)                                                                                                                                      | Child-only native delegates, loader handles, CLR lifetime and state lock                                                                                  |
| [Plugins](../crates/aftercast/src/plugins.rs)                                                                                                                                                  | Plugin configuration, session-scoped IDs and lifecycle requests                                                                                           |
| [Import](../crates/aftercast/src/import.rs), [logs](../crates/aftercast/src/logs.rs), [snapshots](../crates/aftercast/src/snapshots.rs), [diagnostics](../crates/aftercast/src/diagnostics.rs) | Host methods, request validation, decoding and owned result types for each feature                                                                        |
| [Payload extraction](../crates/aftercast/src/bridge.rs)                                                                                                                                        | Platform asset selection and rollback on extraction failure                                                                                               |
| [EntryPoint](../managed/Bridge/EntryPoint.cs) / [PluginSession](../managed/Bridge/PluginSession.cs)                                                                                            | Native command validation, initialized plugins, registration and shutdown                                                                                 |
| [PluginLoader](../managed/Bridge/PluginLoader.cs) / [OwnerContext](../managed/Bridge/OwnerContext.cs)                                                                                          | Assembly ownership and caller-thread dispatch                                                                                                             |
| [LogBuffer](../managed/Bridge/LogBuffer.cs) / [CombatSnapshotEncoding](../managed/Bridge/CombatSnapshotEncoding.cs)                                                                            | Bounded original-log observations and snapshot encoding                                                                                                   |
| [ACT objects](../managed/ActCompat)                                                                                                                                                            | ZoneData, EncounterData, CombatantData, DamageTypeData, AttackType and MasterSwing own their state, calculations, cache invalidation and typed registries |
| [LogFollower](../managed/ActCompat/LogFollower.cs)                                                                                                                                             | Open file identity, initial position, zone scan, complete-record following and reader shutdown                                                            |
| [ImportOperation](../managed/ActCompat/RootImport.cs)                                                                                                                                          | Import admission, temporary parser context, restoration and partial failure                                                                               |
| [Control](../crates/aftercast-winforms/managed/Control.cs)                                                                                                                                     | Headless control ownership, caller-context dispatch and disposal                                                                                          |

Host feature methods pass owned request buffers directly to the parent RemoteSession.
The child-only ChildHost owns its local ManagedSession and audio; there is no
shared local/remote enum or parent audio state. Native calls borrow buffers only
for the duration of the call; delegates and loader handles remain private.
Returned strings, records and snapshots own their data.

[Combat GUI queries](../crates/aftercast/src/combat.rs) read live and retained
encounters through [the bridge encoder](../managed/Bridge/CombatQueries.cs) and
[graph queries](../managed/ActCompat/CombatQueries.cs). Reads copy values under
the combat lock without joining action producers. Weak encounter identities
remain stable during the session without retaining removed graphs. Import
completion marks every added encounter's status independently of the latest
ended snapshot; [query contracts](snapshots.md#gui-combat-queries) describe paging.
Attack paging counts skipped categories without enumerating their attacks. The
bridge writes text through the standard character-span encoder and returns an
owned array segment, avoiding per-string byte arrays and a final payload copy.
See [refactoring measurements](refactoring-performance.md) for the synthetic workload.

## Lifecycle invariants

Spell-timer definitions, frames and recast modifications live in
[FormSpellTimers](../managed/ActCompat/FormSpellTimers.cs). Host pumping advances
warning/expiry/removal events outside its state gate. Live accepted combat actions
notify timers after BeforeCombatAction edits; imports bypass that notification.
The log time estimate adds monotonic elapsed time, and import restoration retains
the live clock baseline. The host owns and disposes its timer form.

| Transition                | Ordering                                                                                                              |
| ------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Host startup failure      | Release the active lease; close an admitted managed session before dropping loader handles                            |
| Host stop / Drop          | Request managed shutdown; kill/reap after deadline; subsequent Host starts a fresh process                            |
| Log selection             | Open and identify one handle; scan that image; apply zone before activating live delivery                             |
| Same-path log replacement | Drain the old complete tail, then read the new identity from its beginning                                            |
| Initial partial record    | Its continuation is discarded before new live records are delivered                                                   |
| Import admission          | Reserve the operation, suspend live delivery, drain admitted work, then capture parser context                        |
| Import completion/failure | Drain actions, retain partial history, restore live context, notify restoration, then finalize import snapshot status |
| Reentrant callbacks       | Reject waits that would join their own producer; pumped owner callbacks remain foreign import producers               |

Initial-zone scanning checks cancellation outside the reader gate, so it never
acquires the root gate while holding the reader gate. Snapshot bytes are read
positionally from the captured handle without moving its live stream position.

An import operation exists until restoration notifications and snapshot status
have completed. Its first callback failure is retained, even after input reading
ends. A newer live snapshot published by restoration callbacks remains newer
than the imported result. Import and live framing retain their distinct contracts.
`LiveParserContext` captures and restores the live parser state under the combat
gate before restoration subscribers run, including the monotonic clock baseline.

## Binary compatibility

| Contract                                                                              | Reason                                                                                      |
| ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| Original names, inheritance, nested delegates/collections and separate chart assembly | Existing compiled plugin MemberRefs require their exact identities                          |
| Writable ACT collections, registration insertion order and numeric oddities           | Genuine ACT probes verify these behaviors; normalization would change results               |
| Per-artifact guarded Linux corrections                                                | Exact hashes, IL and exception regions reject unrecognized versions safely                  |
| Process-owned loader cache                                                            | Loaded assemblies outlive individual host sessions                                          |
| Independent original-reference processes                                              | Reference probes execute against genuine ACT without loading the replacement implementation |
| Independent WinForms package build settings                                           | Extracted Cargo packages build without the repository's managed props                       |

See [compatibility](compatibility.md) for supported artifacts and comparison limits,
and [development](development.md) for build and test commands.
