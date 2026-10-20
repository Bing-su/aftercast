# M4: original recording research

M4 is not complete. This inspection establishes the pinned writer's control flow,
not gameplay capture, storage durability, or FF Logs acceptance. The Rust log
subscription is a bounded observation channel and cannot replace this writer.

## Artifact identity and evidence

All tokens below are local to these exact SDK 3.0.3.3 files. The ignored ledger
`binaries/analysis/m4-original-recording-evidence.json` retains the REA metadata,
method-body hashes, exception regions, call edges, and string literals. Inspection
did not load or execute the target assemblies.

| File | SHA-256 | MVID | REA member evidence |
| --- | --- | --- | --- |
| `FFXIV_ACT_Plugin.dll` | `af1031cdba8eeba8f53dd9d618e58d62d4919bfa77db9b31c568bd04cb359093` | `a06abaef-4808-48e0-ab76-f2608b6b9408` | `ev_843864af514dab029377d8a584afd7c6b444b9a42c2e43fc2561dcf95b1c60a2` |
| `SDK\FFXIV_ACT_Plugin.Logfile.dll` | `2f4faa819df144ca70a17c17e79e45c9cdecf413c98c12fc42b51d73af10b5e7` | `ff6af951-0fd5-413c-b114-d048ca6b23b4` | `ev_162b9a2d7e7c0cb0ef9ee2b7fe53b32a389a0e9729ad6802bc10413b88a5a6fb` |
| `SDK\FFXIV_ACT_Plugin.Config.dll` | `f965992b4f520ae9aabc2158f925ae4044be647113bc1e7770abb9a6e3b20b2e` | `5431b9a8-f842-47a1-a471-618b65cf6833` | `ev_ec140b0e0174414c43af7e78486326e35af9d07ee2d30e5aa6b0321dc0fded2b` |

Additional `DataCollection` evidence is
`ev_1ae65100901bfa9543d0417642228d969263696ae48a5cdffcfb26673817a694`.
The read-only PEReader disassemblies are retained beside the ledger as
`m4-original-writer-il.txt`, `m4-settings-mediator-il.txt`,
`m4-main-recording-settings-il.txt`, and
`m4-data-collection-lifecycle-il.txt`. They show constants and branches omitted
from the compact REA instruction anchors.

## Writer behavior

| Surface | Observed control flow | Consequence |
| --- | --- | --- |
| Queue | `LogOutput::.ctor`, `0x06000046`, creates `Queue<string>(1000)`; `WriteLine`, `0x06000049`, enqueues under `_LogQueueLock` | 1,000 is initial capacity, not a limit. No item/byte bound, producer wait, or drop policy appears in this queue path. Sustained disk lag can grow memory. |
| Record construction | `WriteLine` formats a padded decimal type, round-trip `O` timestamp and payload, then appends a hash computed with an internal sequence; payload NUL becomes a space | The sequence participates in hashing but is not a separate output field. Preserve the original writer's bytes for upload testing; do not reconstruct them from mutable ACT hooks or combat snapshots. |
| Hash fallback | `ConfigureLogFile`, `0x06000047`, tests SHA-256; an exception sets `_disableHash` and traces `ERROR: Windows feature is missing, disabling support for FFLogs Uploads.` | A started writer does not prove upload-compatible hashing. On the fallback path the hash function returns `0`. The message text is the vendor's wording, not a verified Windows dependency. |
| Naming | `ConfigureLogFile` combines `DataCollectionSettings.LogFileFolder` with `Network_{0}{1}{2}0{3}_{4:yyyyMMdd}.log`, using assembly version components and `DateTime.Today` | The file date is local wall-clock date at configuration, not each record's timestamp. |
| Create/follow order | The same method creates the directory and, if absent, an empty file with `AppendAllText`; it then sets ACT `LogFilePath` and calls `OpenLog(true, false)` when the path differs or the file was newly created | ACT can follow the file before the long-lived writer stream opens. A visible path or successful `OpenLog` does not prove the writer opened successfully. |
| Stream | `Run`, `0x06000048`, IL 83–86 opens `FileStream(path, FileMode.Append, FileAccess.Write, FileShare.ReadWrite)`, then `StreamWriter(Stream)` | Readers and other writers may open the file. There is no explicit encoding argument or delete sharing; the runtime's StreamWriter default determines encoding. Actual bytes still need a fixture test. |
| Write/flush | Closure `0x06000054` dequeues one line; `0x06000055` writes it; `0x06000052` calls `TextWriter.Flush`, in that order for each record | Flush reaches the underlying stream but is not an explicit durable disk synchronization. A line is already removed if write or flush throws. There is no retry/requeue of that line in this path. |
| Empty queue | `Run` waits up to 50 ms on cancellation when no line is available | This is polling, not a queue notification or acknowledged write protocol. |
| Rotation | IL 318 compares the settings folder with a value captured once before the outer loop, but normal noncancelled delivery branches directly from IL 313 to dequeue at IL 125 | The folder check runs at initial stream entry and after cancellation, not on each healthy iteration. Ordinary active folder changes and midnight/date/size rotation are not polled. Reopening after a failure recomputes the date; a folder change racing initial configuration can repeatedly reopen because the captured comparison value is not updated. |
| Cancellation | The empty-queue wait at IL 285–301 exits through disposal when cancelled. However, IL 306 branches to dequeue only while cancellation is false; on cancellation it falls through IL 318, whose unchanged-folder branch returns to IL 306 | Cancellation after a write/flush can enter a tight cancellation/folder-check loop. This path needs an isolated runtime regression. Neither path drains `_LogQueue`; disposal flushes stream buffers, not queued records. Shutdown is not a lossless recording boundary. |

## Errors and lifecycle

| Case | Exact branch | Observable limit |
| --- | --- | --- |
| `IOException` | `LogOutput.Run` handler IL 383–418 calls `ISettingsMediator.AddExceptionMessage(DateTime.Now, "FFXIV_ACT_Plugin.LogOutput Error writing to log file. " + exception.ToString())` | The plugin-private parser-message sink receives each caught I/O error. It does not call ACT `WriteExceptionLog` on this branch; host diagnostics alone cannot establish recording health. The outer loop retries after its 50 ms cancellation wait. |
| Other exception | Handler IL 420–463 calls `IActWrapper.WriteErrorLog` with `FFXIV_ACT_Plugin.LogOutput exception occurred.` once, then retries with an additional 100 ms wait | The suppression flag is local to this `Run` invocation. A generic host error count is not an exact writer-failure count. Exceptions thrown by the error-reporting callbacks themselves are outside these sibling catch handlers. |
| `ThreadAbortException` | Handler IL 380–382 exits | This is vendor-era behavior, not a portable cooperative stop API. |
| Start | Main `InitPlugin`, `0x06000105`, calls `DataCollection.StartMemory` at IL 225, before reporting `Started`; `StartMemory`, `0x060000f9`, starts writer, memory, and monitor workers with the shared cancellation token | The public initialization call starts background workers before returning. Constructor retention in the host does not provide a public pre-start settings injection point. |
| Stop | `DataCollection.StopMemory`, `0x060000fa`, clears the restart callback, cancels the token, stops packet scanning, polls writer/memory liveness 25 times with 10 ms sleeps, then calls `Thread.Abort` on survivors | Slow/stuck writer shutdown must be tested in an isolated process. Existing successful no-game teardown does not prove a loaded writer drains or stops within this window. |
| Settings replacement | `SettingsMediator.set_DataCollectionSettings`, `0x06000013`, replaces its stored reference; changes to folder, process ID, or region invoke `RestartDataCollection` when prior settings exist | The attached DataCollection callback `0x060000fd` stops/starts packet scanning. It does not cancel/recreate the writer. The settings object exposes mutable fields, so direct in-place edits also bypass setter comparison. |

## Public configuration boundary

| Available public surface | What it actually provides |
| --- | --- |
| Main plugin `DataRepository`, `DataSubscription`, `PluginStarted` | Repository/game information and data event subscriptions, plus startup state; no public writer-folder, flush, queue, or recording-status endpoint |
| Config assembly `ISettingsMediator.DataCollectionSettings` | A public setter on a separately constructed mediator; the main plugin's actual mediator is not exposed by its public SDK |
| `SettingsPropertyPage.GetDataCollectionSettings`, `0x06000130` | Reads controls into settings; it is not an accessible main-plugin configuration service |
| ACT `LogFilePath`/`LogFileChanged` | Current file-follow input, also affected by imports and arbitrary plugins; it cannot honestly be labeled original recording status |

The main instance holds a private IOC container and private DataCollection. A
production adapter must not treat private reflection, guessed control names, or
an independent mediator as a supported writer configuration API. The generic
host currently has no public SDK route to owner-safe original writer settings.

## Next bounded validation slice

Use a separate original-writer harness, not a production configuration API. It
can construct the public SDK `LogOutput` with controlled interface adapters,
start its public `ScanThread` with a caller-owned cancellation token, and record
actual filesystem effects. Reflection may inspect queue state in this test
process only; it must not alter production behavior.

| Locally testable without a game | Requires additional environment/evidence |
| --- | --- |
| Actual record bytes, encoding/BOM/newlines/hash, append order, queue growth, per-record flush visibility, cancellation with pending records | Official FF Logs uploader acceptance and live logging; no conclusion follows from syntactically plausible records |
| New/existing file follow callbacks, separate-folder selection, mid-run folder change/reopen, cancellation and final thread join | Windows file sharing/locking behavior and real plugin UI configuration |
| Deterministic open failure and controlled write/flush failure; exact private versus host error sinks; already-dequeued record loss | Disk-full/crash durability on representative filesystems and production storage |
| Synthetic traffic throughput, peak queue depth/bytes, shutdown duration, independent parser/subscriber progress | Actual Linux process/memory/network capture, game reconnect/zone changes, privileged capture, sustained real gameplay |

Run every failure/shutdown case in a child process with a parent timeout, copied
pinned dependencies, and a temporary output directory. Do not attach to an
existing plugin instance, start memory/network workers, upload files, or write
into the pinned deployment. All started threads must be cancelled and joined;
an unjoined worker is a failed case, with the child terminated by its parent.
These checks can characterize existing loss/failure behavior. They cannot turn
an unbounded vendor writer into a bounded, durable recording service.

## Isolated harness result

The dependency-free console harness lives in `managed/tests/OriginalWriter`.
Run `mise run test:original-writer -- /absolute/pinned-sdk-deployment` to reproduce
the controlled cases. All three copied dependencies are hash-checked before
execution; the harness does not reference the replacement core. Controlled runtime output is retained in
`binaries/analysis/m4-original-writer-runtime.jsonl`; this is a separate runtime
observation, not part of the static REA evidence claim above.

| Scenario | Observed on Linux / .NET 10 |
| --- | --- |
| Ordered writes and flush visibility | Three records were readable before disposal, with strict UTF-8/no BOM, expected timestamp/payload/hash, Unicode preserved and NUL replaced; empty-queue cancellation joined the worker |
| Output directory blocked by a regular file | One genuine I/O failure reached the private settings sink; zero ACT host errors; cancellation joined the worker |
| Cancellation immediately after genuine Flush | Original worker did not stop; the parent watchdog killed and reaped the child process. Reported as `known-failure`, not a passing shutdown |

The harness copied only the pinned Logfile/Common/Config assemblies to a
temporary deployment and constructed the writer with test interface adapters.
It did not initialize the main plugin or start memory/network workers. Each
child had a five-second timeout; the parent deleted its temporary deployment
and files afterward. Folder-change rotation, write/flush I/O failure after
dequeue, throughput, Windows behavior and uploader acceptance remain untested.
The command succeeds when the expected behavior is characterized, including the
explicit `known-failure` cancellation result. Its exit status must not be read as
proof of safe original-writer shutdown. Successful cases must join their workers;
the known-failure child must time out, be killed and be reaped.

## Pinned Linux cancellation correction

The bridge now corrects only this cancellation loop when the main plugin hash
matches the inspected build and the host runs on Linux. `PluginContext` resolves
the embedded Logfile assembly through `LinuxPluginCompatibility.LoadLogfile`,
which bounds decompression to 26,112 bytes, validates the pinned file and method,
and loads a rewritten byte-array clone. The original deployment and embedded
resource remain unchanged. Unknown and already-rewritten images are rejected.
Windows and other plugin versions do not receive this adjustment.

| Guard/result | Exact value |
| --- | --- |
| Method | `LogOutput.Run(object) : void`, token `0x06000048`, signature blob `20 01 01 1c` |
| Original Run IL SHA-256 | `253503df897ee91a7aaebaee04e3009cf55691a703c6aacb946e3c1125440972` |
| Corrected Run IL SHA-256 | `c37628922371e34b053e65a3f21ea456c9c7e502bc879d139332b6f68dae8ce8` |
| Corrected assembly SHA-256 | `2530772a8582b7fe5d180964f42eebd49440fbc8dbc9d4f10cd9d859dc9364fa` |
| Method shape | RVA 12872, IL file offset 5204, 485 bytes, max stack 5, initialized locals `0x1100000a` |
| Preserved exception regions | Writer finally: try `[120,344)`, handler `[344,366)`; stream finally: try `[92,368)`, handler `[368,378)`; three catches retain try `[77,380)` and their original handlers |

| Original CFG | Corrected CFG |
| --- | --- |
| Successful write branch `283→306`; initial folder branch `340→306` | Both reach the relocated cancellation test at 303 |
| Empty-queue wait false branch `299→306` | False branch reaches 303 |
| Empty-queue cancellation `leave301→484` | Same-size window reuses shorter `leave.s301→366` and the existing disposal chain |
| Cancellation test at 306; false jumps to dequeue 125; true enters folder check and can spin | Test moves to 303; false still jumps to 125; true executes `leave.s315→366` |

The replacement occupies the same 19-byte window `[299,318)`, with two additional
short-branch operand changes at 284 and 341. No method-header, metadata, section
or exception-table bytes change. Each new leave starts with an empty evaluation
stack inside the writer try and runs the existing writer-disposal handler,
followed by stream disposal through `366→378→465`. The cancellation-aware wait
at 465 then exits. Initial branch `120→318` and folder block `[318,344)` are
unchanged; healthy noncancelled delivery still dequeues at 125.

`mise run test:original-writer -- /absolute/pinned-sdk-deployment --patched`
links the actual production rewriter into the isolated harness. The three
controlled cases passed with that exact implementation: ordered bytes and flush
visibility, private I/O-error reporting, and cancellation immediately after a
genuine flush. The last case joined after one record while **two records remained
queued**. This deliberately preserves the vendor's cancellation policy; the
correction adds no drain, retry, queue bound, rotation or durability guarantee.
The unmodified harness still reports the original cancellation as a known
failure. Original-plugin integration also checks that the actual plugin context
loaded the stream-backed Logfile assembly with this exact corrected Run IL hash.
Live gameplay capture and loaded recording behavior still require their own
validation; these checks do not complete M4.
