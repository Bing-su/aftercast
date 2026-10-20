# ACT, SDK, and OverlayPlugin event delivery

Research date: 2026-10-20. Static binary/source observations, not measured
throughput, loss rates, or successful original-plugin execution. The three event
boundaries below have different ordering and overload behavior.

## SDK public subscriptions

Common IDataSubscription includes add/remove for NetworkReceived/Sent,
CombatantAdded/Removed, PrimaryPlayerChanged, ZoneChanged, PlayerStatsChanged,
PartyListChanged, LogLine, ParsedLogLine, ProcessChanged. The selected interface
does not include CombatData or semantic combat-action events. Other APIs may.

| Delegate | Common Invoke token | Exact parameter metadata | Signature blob |
| --- | --- | --- | --- |
| NetworkReceived | 0x06000056 | void(string connection, int64 epoch, byte[] message) | 2003010e0a1d05 |
| NetworkSent | 0x0600005a | Same | 2003010e0a1d05 |
| LogLine | 0x06000076 | void(uint32 EventType, uint32 Seconds, string logline) | 20030109090e |
| ParsedLogLine | 0x0600007a | void(uint32 sequence, int32 messagetype, string message) | 20030109080e |

The byte array is a supplied SDK network message, not established as a complete
TCP packet/segment capture. Framing and epoch units remain to be checked against
the producer. EpochToDateTime/ToLocalTime and DateTime.AddMilliseconds anchors
exist, but parameter names/types alone do not define the full conversion. The
ParsedLogLine first argument is a sequence, not a timestamp.

| Delegate signatures | BeginInvoke token/blob | EndInvoke token/blob |
| --- | --- | --- |
| NetworkReceived | 0x06000057 / 2005127d0e0a1d051280811c | 0x06000058 / 200101127d |
| NetworkSent | 0x0600005b / same | 0x0600005c / same |
| LogLine | 0x06000077 / 2005127d09090e1280811c | 0x06000078 / same |
| ParsedLogLine | 0x0600007b / 2005127d09080e1280811c | 0x0600007c / same |

BeginInvoke adds AsyncCallback/object and returns IAsyncResult. Class operands
are metadata coordinates for this exact assembly. PEReader signature hashes for
all 12 entries matched retained REA evidence.

Memory DataSubscription.OnNetworkReceived (0x0600004f), OnNetworkSent (0x52),
OnLogLine (0x67), and OnParsedLogLine (0x6a) each have 71-byte/37-instruction bodies.
They iterate GetInvocationList and call each handler's BeginInvoke with a callback,
then discard the result. They do not wait for callback completion, impose a
per-consumer queue cap, track dropped records, ACK, replay, or order completion.
No exception region appears in these four bodies. Do not infer globally lossless
capture or unlimited runtime resources from this bounded observation.

Network arrays are passed directly in these bodies without Clone/copy. Producer
reuse/mutation and array lifetime are not thereby proven safe for retention.
Numbering callbacks at entry records observation order, not restored game order.

DelegateCallback (0x0600006e, 103 bytes/33 instructions) obtains AsyncResult's
delegate at IL6, reflects EndInvoke at IL18/23 and invokes it at IL39. Try IL12-46
and catch(Exception) IL47-101 trace caller/plugin-author diagnostics. Conversion
and get_AsyncDelegate precede the try. No retry/unsubscribe/error counter is in
this body. remove_NetworkReceived/LogLine/ParsedLogLine use Delegate.Remove and
Interlocked.CompareExchange (0x4e/66/69); they do not cancel already captured or
scheduled handlers or wait for a drain.

## Public subscriptions are distinct from parser input

| Boundary | Static observation |
| --- | --- |
| Network.ScanPackets.NetworkMessageReceived | Processing and public notification are separate call paths |
| Memory.DataEvent.OnNetworkReceived/Sent | Direct handler Invoke at 0x06000018/19 |
| DataEvent.OnLogLine/OnParsedLogLine | Direct Invoke at 0x06000021/22 |
| DataEventProcessor.WriteLineToLog | ILogOutput.WriteLine followed by OnParsedLogLine (0x06000030) |
| DataEventProcessor.OnParsedLogLine | Excludes types 249-254; increments uint32 sequence, then public notification (0x06000031) |
| Main ConfigureUI | Registers ACT BeforeLogLineRead handler (0x0600004c) |
| Main parser callback closure | Calls ParseMediator.BeforeLogLineRead (0x060000d6) |

The sequence field at 0x04000023 is incremented by load/add/store (IL51/58) and
passed at IL70. No lock/Interlocked appears in this selected body. Reset/wrap and
concurrent-producer conditions are not fully restored. Public async delivery is
not shown to be mandatory for ordinary parser initialization. Modern .NET support
for this original BeginInvoke/Remoting path is unverified; M0 does not use it.

## ACT core delivery

Target: ACT 3.8.5.288; SHA-256
934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657;
MVID 94f61cd2-1f88-4a7c-98d1-0646897b1fb4.

| Method/token | Observed behavior | Slow consumer consequence |
| --- | --- | --- |
| ParseRawLogLine 0x060002c7 | rawLogLineLock; direct BeforeLogLineRead; consume modifications; direct OnLogLineRead | Blocks parsing caller and later handlers/lines |
| ReadLog 0x060002c2 | Read file, buffer List<string>, sequential ParseRawLogLine | Backlog can grow; no selected per-subscriber cap/drop/lag contract |
| AddCombatAction 0x060002b6 | InCombat validation; direct Before hook; modification/cancellation; locked queue Add | Before callback blocks producer; no selected queue capacity branch |
| ThreadAfterCombatAction 0x060002b8 | ToArray/Clear under queue lock; release; aggregate each action then direct After hook | Slow After blocks worker, while producer can enqueue more |
| EndCombat 0x060002cf | Clear InCombat; wait for queue and active array; finish encounter/event | Stuck worker can delay combat end |
| ImportFile 0x060002c5 | Calls ParseRawLogLine per record | Same synchronous log-handler delay |

Before hooks change/cancel parser input and combat actions. Making them asynchronous
observers would change semantics. The debugHandlers path catches individual handler
errors; it is not a timeout or slow-consumer removal mechanism.

The tmrTick_Tick body (0x06000107) increments parsingBusyCount when logLinesQueue
is nonempty. When it exceeds 10 and other conditions hold, popup lambda 0x06000310
uses trayText-parsingBusy/trayTitle-parsingBusy. No queue clearing/drop was found
in that path. This warns about parsing delay, not a subscriber loss range. Timer
period was not established. Selected FormActMain events have logs/combat/actions,
not FFXIV byte[] packets; those exist in the game-specific SDK instead.

## OverlayPlugin and transport

Sources pinned to OverlayPlugin commit 75ac96f9013f15e3acf90797087a47e7a4ec6db4.

| Path | Source observation |
| --- | --- |
| CombatData | Periodic aggregate snapshot; default UpdateInterval=1 second. [Producer](https://github.com/OverlayPlugin/OverlayPlugin/blob/75ac96f9013f15e3acf90797087a47e7a4ec6db4/OverlayPlugin.Core/EventSources/MiniParseEventSource.cs#L182-L225), [config](https://github.com/OverlayPlugin/OverlayPlugin/blob/75ac96f9013f15e3acf90797087a47e7a4ec6db4/OverlayPlugin.Core/EventSources/BuiltinEventConfig.cs#L158-L164) |
| LogLine | ACT Before hook; rawLine is original text, not network bytes. [Hook](https://github.com/OverlayPlugin/OverlayPlugin/blob/75ac96f9013f15e3acf90797087a47e7a4ec6db4/OverlayPlugin.Core/EventSources/FFXIVOptionalEventSource.cs#L26-L46), [output](https://github.com/OverlayPlugin/OverlayPlugin/blob/75ac96f9013f15e3acf90797087a47e7a4ec6db4/OverlayPlugin.Core/EventSources/FFXIVOptionalEventSource.cs#L163-L175) |
| ImportedLogLines | Separate accumulated strings and timer batch. [Collection](https://github.com/OverlayPlugin/OverlayPlugin/blob/75ac96f9013f15e3acf90797087a47e7a4ec6db4/OverlayPlugin.Core/EventSources/MiniParseEventSource.cs#L141-L151) |
| SDK NetworkReceived | Internal parser creates selected semantic events such as OnlineStatusChanged; no general byte[] WS stream found in inspected built-ins. [Parser](https://github.com/OverlayPlugin/OverlayPlugin/blob/75ac96f9013f15e3acf90797087a47e7a4ec6db4/OverlayPlugin.Core/NetworkProcessors/NetworkParser.cs#L50-L71) |
| EventDispatcher | Direct sequential HandleEvent under event-type receiver lock; catches/logs errors. [Source](https://github.com/OverlayPlugin/OverlayPlugin/blob/75ac96f9013f15e3acf90797087a47e7a4ec6db4/OverlayPlugin.Core/EventDispatcher.cs#L120-L141) |
| Timer overlap | updateRunning skips overlapping tick; not every tick retained. [Source](https://github.com/OverlayPlugin/OverlayPlugin/blob/75ac96f9013f15e3acf90797087a47e7a4ec6db4/OverlayPlugin.Core/EventSourceBase.cs#L40-L60) |
| Modern /ws | JSON serialization and Send; returned Task ignored. No wrapper-level bound/ACK/replay. [Source](https://github.com/OverlayPlugin/OverlayPlugin/blob/75ac96f9013f15e3acf90797087a47e7a4ec6db4/OverlayPlugin.Core/WebSocket/WSServer.cs#L290-L333) |
| WS error/close | Unsubscribe/remove connection. [Source](https://github.com/OverlayPlugin/OverlayPlugin/blob/75ac96f9013f15e3acf90797087a47e7a4ec6db4/OverlayPlugin.Core/WebSocket/WSServer.cs#L260-L282) |

Fleck 1.2.0 source is pinned to 45672e0781974bb04dbad1b94320756a33c60a6d.
The [NuGet record](https://api.nuget.org/v3/catalog0/data/2021.04.22.03.55.01/fleck.1.2.0.json)
points to the official repository but lacks a commit; byte equality with the
packaged DLL is unproven. Do not apply TLS behavior to plain WS.

| Fleck path | Observation |
| --- | --- |
| Send | Frames bytes and returns Task; not an application receipt ACK. [Source](https://github.com/statianzo/Fleck/blob/45672e0781974bb04dbad1b94320756a33c60a6d/src/Fleck/WebSocketConnection.cs#L60-L96) |
| Plain ws | NetworkStream, BeginWrite per send; no Fleck QueuedStream here. OS/.NET buffers unexamined. [Source](https://github.com/statianzo/Fleck/blob/45672e0781974bb04dbad1b94320756a33c60a6d/src/Fleck/SocketWrapper.cs#L168-L188) |
| TLS wss | SslStream wrapped in QueuedStream. [Configuration](https://github.com/statianzo/Fleck/blob/45672e0781974bb04dbad1b94320756a33c60a6d/src/Fleck/SocketWrapper.cs#L72-L84) |
| TLS queue | Unbounded Queue<WriteData>; pending writes retain frame arrays. [Enqueue](https://github.com/statianzo/Fleck/blob/45672e0781974bb04dbad1b94320756a33c60a6d/src/Fleck/QueuedStream.cs#L75-L86) |
| TLS order | EndWrite then FIFO Dequeue; enqueue order, not game/client-processing order. [Source](https://github.com/statianzo/Fleck/blob/45672e0781974bb04dbad1b94320756a33c60a6d/src/Fleck/QueuedStream.cs#L137-L174) |
| Write failure | Callback logs/closes socket; no selected event replay/ACK. [Close path](https://github.com/statianzo/Fleck/blob/45672e0781974bb04dbad1b94320756a33c60a6d/src/Fleck/WebSocketConnection.cs#L203-L228) |

## Approved decisions

| Decision | Outcome |
| --- | --- |
| Q12 | Native ABI and Rust-owned batches |
| Q14 | Original logs/results first; raw SDK network API deferred |
| Q15 | Preserve original files; core continues; live Rust logs drop oldest on bounded overflow and report loss; full snapshots coalesce to newest |

The initially proposed overflow-disconnect policy was rejected in favor of
continued delivery. Recording failures and subscriber loss are distinct. Loss
status must remain observable outside a full event queue. A lossy subscriber must
not produce FF Logs upload files. See the [blueprint](blueprint.md) for policy.

## Evidence and verification boundaries

| Scope | REA parent ID |
| --- | --- |
| Common contracts | ev_1bd122a43c24894f3a3f64c7de67bfc677124a1b5cb7a2be2ca9b2d4b6864f9f |
| Memory | ev_68fe92da14645a6c2f020b3dbcd5d18801566779d2900bdbbb32f62dc1a3e05d |
| Network | ev_cdd0f54ae55adfc19e7b88cbb71c51e691ad3ce6fe30a2fb2011a2a18f578945 |
| Main members | ev_ad7641feec72d06f1e82ffc7dfa63bc104ff2f04fa1754d4e4ec55d74681ab87 |
| Main init/UI selection | ev_e34271e902daeebb7244c5c6d537fed8b0a7269754cfb0ff3ddd714e90bca67c |
| ACT host types | ev_e10bfceb62a78ed060b028138f7963d250457608d5f6551af9718f399d5894c9 |
| ACT events/zone types | ev_2fb5619a651ba480afdbaba659cf3ae298a1269cfe5108d9fcfa5e6a37bfa03d |

SDK hashes are in [SDK analysis](ffxiv-act-sdk-analysis.md). Common MVID:
13e4b6d8-29da-4f12-812b-85a9ceccdd85; Memory: 89b13c66-df57-43bf-8217-c84f5c77f5f6;
Network: 76681374-3fff-4800-a863-ec91c4641fdc; main: a06abaef-4808-48e0-ab76-f2608b6b9408.

Ignored binaries/analysis contains act-event-delivery-2026-10-20-evidence.json,
act-event-delivery-selected-il.txt (six ACT methods, 3,025 instructions checked
against REA IL hash/count), act-event-delivery-backlog-il.txt (timer/popup),
act-event-sdk-signatures.fsx/.txt (12 signature hashes), and
act-event-sdk-selected-il.txt (nine Memory IL hash/count checks). SDK bundles use
the sdk-3.0.3.3-core/network prefixes. PEReader helpers read bytes without loading
the original targets. Retained evidence is local, not included with Git docs.

No original ACT/SDK execution, delegate async compatibility test, actual network
stress measurement, or Linux plugin-host execution was performed. Queue absence
claims are limited to selected methods/source; they establish no global lossless
or unlimited-resource contract.
