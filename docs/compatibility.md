# Compatibility

The host implements the ACT contracts used by its fixtures and the pinned FFXIV
plugin. Reference tests compare selected behavior with genuine ACT; they do not
establish complete ACT or Windows parity.

## Reference artifacts

| Input           | Version / location                                                   | Validation                                                                                                                                                                  |
| --------------- | -------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Genuine ACT     | 3.8.5.288; `binaries/act/Advanced Combat Tracker.exe`                | [OriginalReference](../managed/tests/OriginalReference/Program.cs) checks the full executable hash before loading                                                           |
| FFXIV plugin    | 3.0.3.3; local SDK deployment or disposable plugin copy              | [Plugin compatibility](../managed/Bridge/LinuxPluginCompatibility.cs) and the [integration runner](../managed/tests/ManagedTests/Program.cs) pin main and dependency hashes |
| Synthetic input | [ffxiv-synthetic.log](../managed/tests/fixtures/ffxiv-synthetic.log) | Seven genuine-writer records with territory/player/actor/party state and two action effects totaling 400 damage                                                             |
| Recorded input  | `TheAbyssalFractureExtreme.log` from cactbot documentation           | [OriginalCorpusChecks](../managed/tests/ManagedTests/OriginalCorpusChecks.cs) checks its hash, 104,744 records and three encounters                                         |

Original ACT and FFXIV binaries are local test inputs, excluded from Git and Cargo
packages. The ACT replacement preserves the assembly identity
`Advanced Combat Tracker, Version=3.8.5.288, Culture=neutral, PublicKeyToken=a946b61e93d97868`
using public signing; it does not contain the original private signing key.

The recorded fixture is available in the neighboring cactbot checkout at
`cactbot2/vendor/cactbot/docs/logs/TheAbyssalFractureExtreme.log`. Its SHA-256 is
`a5992e53bef80424ba29d0db4db00d0359c727e00532de0873ce49c67732f240`.
It is a Zeromus recording made with plugin version 2.6.9.4, not a recording of the
currently pinned parser. The file is not copied into this repository.

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
in the [WinForms crate](../crates/bxivact-winforms/README.md).

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

## Original writer

| Property        | Behavior                                                                                                           |
| --------------- | ------------------------------------------------------------------------------------------------------------------ |
| Queue           | The vendor's initial capacity of 1,000 is not a bound; sustained disk lag can grow memory                          |
| Output          | The writer formats timestamps, sanitizes payload NULs and calculates checksums using its private sequence          |
| Write failure   | A dequeued line is not requeued after a write or flush exception                                                   |
| I/O diagnostics | Some failures go only to the plugin-private message sink, so host diagnostics alone do not establish writer health |
| Flush           | Stream flush is not a durable disk synchronization                                                                 |
| Shutdown        | Disposal flushes stream buffers, not the pending queue; the Linux correction fixes cancellation exit only          |
| Rotation        | Healthy delivery does not poll for midnight, size or folder changes                                                |

Original-log subscriptions are bounded observations of hook input. Their loss
accounting does not describe the vendor's writer queue or recover a recording.
Native game capture, durable recording, rotation guarantees and FF Logs acceptance
remain unverified.
