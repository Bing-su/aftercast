# Local log validation — 2026-10-21

Local logs are optional maintainer inputs. The repository does not assume an ACT
installation path, search another host's logs automatically, or include personal
logs/transcripts in Git or Cargo packages. `mise run check` needs no personal log.

## Run on another host

Provide a trusted disposable plugin deployment, a private state directory and
an existing stable log file:

```sh
mise run test:original-log -- "<plugin-copy>" "<new-state>" "<log>"
```

Paths are arguments, including paths with spaces. The argument-taking import and
replay tasks use mise's [Windows command override](https://mise.jdx.dev/tasks/task-configuration.html#run-windows)
to match the default Windows shell; Unix retains its existing shell commands.

The normal Rust/mise/.NET 10 SDK prerequisites still apply. Windows plugin runs
need Desktop Runtime, and the supplied FFXIV build also needs its chart dependency
in the disposable deployment. No installed GAC path is hardcoded in this task.
The exact plugin/parser hash guards remain in force: Windows additionally admits
the verified 3.0.3.4 pair; Linux corrections remain restricted to 3.0.3.3.

The task derives line counts from the provided input, records its SHA-256 and
checks three imports against their committed actions, actor/ally totals, latest
snapshot, restored live state and exact bounded-subscriber loss. Unlike the
pinned Zeromus fixture, it assumes no fixed encounter count or duration. It
requires combat data and at least two nonempty input records; zero healing or
zero damage is valid when it matches the committed actions. Startup/private
diagnostics remain strict; a successful numeric phase does not override a later
diagnostic failure. Historical type-251 diagnostic records are matched by exact
timestamp, text and bounded occurrence count across the three imports, then
reported separately. An unrecorded message still fails validation.
Timestamp matching uses the plugin's own host-local timestamp parser, and
diagnostic text retains embedded pipes, so a recording's source timezone and
message separators do not depend on the validation host.

Import diagnostics are captured from the original parser callback, preserving
its normal UI callback, because the plugin's UI list stops retaining messages
after about 100 entries. Startup capture fails if that list is already full.
Windows does not require startup notices or a version-first notice order;
Linux keeps its existing startup assertions. The simulated-damage warning
allowance remains restricted to the pinned corpus, not arbitrary local logs.

Each run writes `recorded-actions-1.json` through `recorded-actions-3.json` below
the supplied state directory. These contain private names/actions. To compare
against both ACT implementations, supply genuine ACT at the documented reference
location, then run:

```sh
mise run test:recorded-actions -- "<state>/recorded-actions-1.json"
```

Repeat for the other two transcripts. Existing transcript limits still apply:
250,000 actions and 128 MiB per transcript. Select a stable archived log that fits
those bounds; this task does not validate a growing live file as a recording.

## Observed run on this host

| Input                   | Value                                                              |
| ----------------------- | ------------------------------------------------------------------ |
| Selected filename       | `Network_30303_20261016.log`                                       |
| Size                    | 56,562,713 bytes                                                   |
| SHA-256                 | `db3d0b055f4f958e1a0c44e6d0d7a4e01bd970f18389072f1a669010a8f48c8c` |
| Recorded plugin version | 3.0.3.3                                                            |
| Importing plugin        | 3.0.3.4, disposable deployment with local chart DLL                |
| Source integrity        | SHA-256 unchanged after all reads                                  |

| Check                                                  | Result                                                                                                  |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------- |
| Three numeric import checks                            | Pass; identical totals across these runs                                                                |
| Records per run                                        | 315,219 read and dispatched                                                                             |
| Encounters / committed actions per run                 | 19 / 71,290                                                                                             |
| Sum of per-encounter combatants                        | 865; not a unique-player count                                                                          |
| Allied damage / healing per run                        | 32,216,001 / 31,937,359                                                                                 |
| Exact subscription loss per run                        | 315,217 records with capacity 2                                                                         |
| Action-derived totals, snapshots and restored state    | Pass                                                                                                    |
| Replay against genuine ACT and aftercast               | All three transcripts pass in both implementations                                                      |
| Native shutdown                                        | Completed                                                                                               |
| Full import command after echo fix                     | Pass, including strict private-message checks and shutdown                                              |
| Historical diagnostics captured throughout all imports | 9,003 matched to input type-251 records; 12 informational notices; no new warning or exception          |
| Boundary regressions                                   | Damage-only input passes; two records produce zero loss; an exception after 100 UI messages still fails |

The initial `ReportCombatData.EchoCommand` failure came from ordinary
`Hojoring>[EX] ...` echo text. Genuine ACT ignores unmatched echo text. The host
now does the same, while known but unimplemented UI/chat commands still throw.
A separate .NET Framework process checked ten unmatched echo cases against the
supplied genuine ACT; host regression checks also verify unchanged combat state
and snapshots. Static ACT command inventory: REA Evidence
`ev_ffaa0f88b306fc55a43c3f4af36d6f90101ddb8afe6e6b9570bcb6c9672217ce`,
method `0x060002cc`, against the ACT SHA-256 recorded in the Windows validation.

Process/signature notices and unknown-effect warnings remaining in the message
sink already exist in the archived input. They are counted as historical input,
not silently allowed by a broad warning prefix. This validates hosting and
consistency; it does not establish that the original parser understands every
effect type in the recording.

The earlier count of 96 historical messages described only the capped UI list.
The callback capture supersedes that check; private diagnostic assertions run
before shutdown, and native shutdown completion is checked separately.

Private evidence remains only under `target/windows-local-log-20261016/` on the
validation host. That directory and its absolute input path are observations, not
prerequisites or defaults for other hosts. No other host or live capture was tested.
