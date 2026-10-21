# Architecture

Rust handles deployment, CLR hosting and owned output. Managed code runs plugins
and maintains the mutable ACT combat graph. A Host represents one session on its
creation thread. Domain terms are in the [glossary](../GLOSSARY.md).

## Module map

| Location                                                                                                                                                                                       | Responsibility                                                                                                                                            |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [Host](../crates/aftercast/src/host.rs)                                                                                                                                                        | Public lifecycle operations                                                                                                                               |
| [ManagedSession](../crates/aftercast/src/runtime.rs)                                                                                                                                           | Native delegates, loader handles, process hostfxr, active-session lease and managed error retrieval                                                       |
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

Host feature methods pass owned request buffers to ManagedSession. Native calls
borrow these buffers only for the duration of the call; delegates and loader
handles remain private. Returned strings, records and snapshots own their data.

## Lifecycle invariants

| Transition                | Ordering                                                                                                              |
| ------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Host startup failure      | Release the active lease; close an admitted managed session before dropping loader handles                            |
| Host stop / Drop          | Attempt managed shutdown once; retain process CLR/hostfxr for restart                                                 |
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
