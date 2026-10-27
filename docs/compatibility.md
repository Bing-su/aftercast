# Compatibility

The host implements the ACT contracts used by its fixtures and the pinned FFXIV
plugin. Reference tests compare selected behavior with genuine ACT; they do not
establish complete ACT or Windows parity.

The [ACT compatibility matrix](compatibility-matrix.md) tracks implementation,
verification evidence and remaining work by feature and API family.

[Additional plugin validation](additional-plugin-validation.md) records five
official downloads, the constructor-order correction and remaining blockers.

`ActCommands` implements the literal `end` command and ignores unmatched echo
text, as genuine ACT does (for example, `Hojoring>[EX] Beacon ...`). Recognized
UI/chat commands such as `clear`, `add `, `macro: ` and `timer ` remain unsupported
and throw rather than pretending to perform those operations.

## Reference artifacts

| Input                | Version / location                                                   | Validation                                                                                                                                                                  |
| -------------------- | -------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Genuine ACT          | 3.8.5.288; `binaries/act/Advanced Combat Tracker.exe`                | [OriginalReference](../managed/tests/OriginalReference/Program.cs) checks the full executable hash before loading                                                           |
| FFXIV plugin         | 3.0.3.3; local SDK deployment or disposable plugin copy              | [Plugin compatibility](../managed/Bridge/LinuxPluginCompatibility.cs) and the [integration runner](../managed/tests/ManagedTests/Program.cs) pin main and dependency hashes |
| Windows FFXIV plugin | 3.0.3.4; `binaries/FFXIV_ACT_Plugin_SDK_3.0.3.4`                     | [Windows original-artifact results](windows-original-validation.md) record numeric success, extra chart dependency and diagnostic failures                                  |
| Synthetic input      | [ffxiv-synthetic.log](../managed/tests/fixtures/ffxiv-synthetic.log) | Seven genuine-writer records with territory/player/actor/party state and two action effects totaling 400 damage                                                             |
| Recorded input       | `TheAbyssalFractureExtreme.log` from cactbot documentation           | [OriginalCorpusChecks](../managed/tests/ManagedTests/OriginalCorpusChecks.cs) checks its hash, 104,744 records and three encounters                                         |

Original ACT and FFXIV binaries are local test inputs, excluded from Git and Cargo
packages. The ACT replacement preserves the assembly identity
`Advanced Combat Tracker, Version=3.8.5.288, Culture=neutral, PublicKeyToken=a946b61e93d97868`
using public signing; it does not contain the original private signing key.

The recorded fixture is available in the neighboring cactbot checkout at
`cactbot2/vendor/cactbot/docs/logs/TheAbyssalFractureExtreme.log`. Its SHA-256 is
`a5992e53bef80424ba29d0db4db00d0359c727e00532de0873ce49c67732f240`.
It is a Zeromus recording made with plugin version 2.6.9.4, not a recording of the
currently pinned parser. The file is not copied into this repository. On this
Windows checkout, the same input is at `D:\repos\cactbot\docs\logs\TheAbyssalFractureExtreme.log`.

## Linux corrections

The recognized FFXIV 3.0.3.3 deployment receives three in-memory corrections.
Original files remain unchanged. Hash and method-body checks reject unexpected
dependencies within the correction path; other plugin builds receive no correction.

| Assembly | Correction                                                                       | Limit                                                                      |
| -------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Memory   | Administrator probing reports false while retaining the original getter/setter   | No Windows administrator identity is emulated                              |
| Logfile  | Post-flush cancellation exits through the existing disposal handlers             | Pending queue records are not drained                                      |
| Resource | The dictionary reader uses the original Windows CRLF delimiter behavior on Linux | Values are not trimmed or normalized; other plugin versions are unverified |

The implementation and guards are in [LinuxPluginCompatibility](../managed/Bridge/LinuxPluginCompatibility.cs)
and [LinuxResourceCompatibility](../managed/Bridge/LinuxResourceCompatibility.cs).
Writer and resource checks exercise exact rewritten behavior and verify that the
input images remain unchanged. Headless WinForms and chart support is documented
in the [WinForms crate](../crates/aftercast-winforms/README.md).

## Comparison tests

| Test                      | What it checks                                                                                                                                 | Limit                                                                                                 |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Original-reference probes | Genuine constructors, Dnum behavior, configured combat actions, ally graphs, duration and aggregate queries                                    | No ACT main form or game collection                                                                   |
| Shared contract probes    | The same compiled probe executes against replacement and genuine ACT in separate processes                                                     | Selected contracts only                                                                               |
| Original synthetic import | Repeated 400-damage imports, failure recovery, live-state restoration, owned snapshots and subscriber loss                                     | Controlled input with no representative DoT/HoT coverage                                              |
| Recorded corpus           | Each import's report, actors and latest snapshot match its own committed-action totals; observer loss is exact                                 | One encounter family and log revision                                                                 |
| Recorded-action replay    | Captured actions run through genuine and replacement ACT objects; four actor totals, automatic allies and allied encounter totals are compared | Parser output is frozen; original import orchestration, UI and complete finalization are not compared |

The original parser uses stochastic rounding and retains some private state
between imports. Repeated imports can produce different amounts and actor/action
counts. Corpus checks compare each run with its own committed actions rather
than fixed totals. A fresh host/plugin session provides isolation for independent
comparisons. Host live-state restoration does not reset all plugin-private state.

Corpus imports write `recorded-actions-1.json` through `recorded-actions-3.json`
in their private state directory. These files contain names and action text and
are test inputs, not a public export format. The replay task consumes these files.
Commands and required arguments are listed in [development](development.md#original-artifact-verification).

The corpus check inspects the plugin's private message sink as well as host
diagnostics. It accepts the pinned plugin's first-start Deucalion DLL rename only
for the exact supplied deployment paths. Known simulated-damage timeout warnings
are counted explicitly; unknown messages still fail with their original text.

## Original writer

| Property        | Behavior                                                                                                                                                                                                                  |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Queue           | The vendor's initial capacity of 1,000 is not a bound; sustained disk lag can grow memory                                                                                                                                 |
| Output          | The writer formats timestamps, sanitizes payload NULs and calculates checksums using its private sequence                                                                                                                 |
| Write failure   | A dequeued line is not requeued after a write or flush exception                                                                                                                                                          |
| I/O diagnostics | The pinned plugin observer forwards writer I/O callbacks to bounded Host diagnostics after attachment; earlier errors or a replaced callback may be missed                                                                |
| Flush           | Stream flush is not a durable disk synchronization                                                                                                                                                                        |
| Shutdown        | Disposal does not drain the pending queue. Host cleanup now reports observed I/O faults, pending records and active known threads; active threads permanently block new plugin construction/initialization in that worker |
| Rotation        | Healthy delivery does not poll for midnight, size or folder changes                                                                                                                                                       |

Original-log subscriptions are bounded observations of hook input. Their loss
accounting does not describe the vendor's writer queue or recover a recording.
Native game capture, durable recording, rotation guarantees and FF Logs acceptance
remain unverified.

### Writer failure and stop boundary (2026-10-27)

The observer applies only to the inspected FFXIV SDK 3.0.3.3 plugin hash. It
captures the actual writer, mediator and writer/scan/monitor threads after the
initializer returns, including the caught-initialization-failure cleanup path.
It chains the existing `ProcessException` callback and captures the owning Host,
so a late diagnostic cannot be redirected to a later `ActGlobals` session. Host
diagnostic failures cannot escape onto the writer thread. Observation begins at
attachment, not before plugin initialization; it is not a complete error ledger.

After DeInit and control/plugin/container disposal, a bounded owner-pumped wait
checks known threads and acquires the vendor queue lock within that same deadline.
A lock timeout reports an unknown pending count and poisons worker reuse. A
nonzero pending count, observed I/O failure or diagnostic failure makes cleanup
report an error. The pending count is only a point-in-time count of records still
in that queue; it excludes already dequeued, partially written or unacknowledged
records. No queue mutation, automatic append/replay or user-log repair is done.

If a known thread remains alive or lifecycle inspection fails, this process-owned
PluginSession becomes permanently unsafe to reuse: Configure, Construct and
Initialize reject further work. Stop is still permitted for cleanup. A later
clean observation or session Stop does not clear that guard; start a new worker
process. A stopped writer with pending records is an explicit recording failure,
but does not by itself imply that a producer remains alive.

| Fault model                                   | Evidence and limit                                                                                                                                                              |
| --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Healthy records and idle cancellation         | Genuine vendor bytes/order/NUL sanitation/checksums verified in temporary paths; does not establish a shutdown drain contract                                                   |
| Open failure and recovery                     | A synthetic regular-file folder blocker fails before dequeue; removing only that blocker lets queued records be written                                                         |
| Write failure                                 | Persistent injected stream Write failure loses the first dequeued record while later records are written; queue count can be zero despite loss                                  |
| Flush failure                                 | Injected exception after the underlying Flush leaves all three records readable, while the caller sees an I/O error; completion is uncertain and blind retry can duplicate data |
| Cancellation before start / after first Flush | Three / two records remain pending respectively in the corrected writer. The pristine writer's post-Flush cancellation spin remains a watchdog-reaped known failure             |
| Disposal and active producer                  | A synthetic post-stop disposal producer enqueues one record; inspection must follow disposal. A live synthetic producer poisons worker reuse even after it subsequently stops   |
| Restart                                       | A fresh vendor writer appends one record without replaying the old queue and restarts its private checksum index; this is not recovery of lost records                          |
| Process crash or forced kill                  | In-memory queue and stream buffers can be lost. No durable journal/replay or complete committed-record count is provided                                                        |
| OS crash, power loss or disk failure          | No durable-sync/transactional recovery guarantee. Vendor StreamWriter.Flush is not fsync; readable test bytes do not prove stable-media persistence                             |

`test:original-writer` executes the pristine and `--patched` variants in isolated,
watchdog-bounded child processes. Fault injection touches only that test's stream
and temporary files. `test:linux-plugin-compat` also runs a real pinned-type
PluginSession fixture without calling InitPlugin/StartMemory or starting game,
network, update or vendor writer workers; it checks pending cleanup errors and
sticky rejection of unsafe worker reuse. Synthetic producer threads are owned and
joined by the fixture.
