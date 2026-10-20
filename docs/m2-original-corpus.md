# Original-plugin recorded corpus integration

## Input and scope

| Item | Pinned value |
| --- | --- |
| Fixture | `cactbot2/vendor/cactbot/docs/logs/TheAbyssalFractureExtreme.log` in the neighboring checkout |
| Local source checkout | `Bing-su/cactbot2`, commit `f0960e1bf9fc2447b973ca6c7dcc5463930d4ef6` |
| Vendored project | `OverlayPlugin/cactbot`, as declared by its `package.json` |
| SHA-256 | `a5992e53bef80424ba29d0db4db00d0359c727e00532de0873ce49c67732f240` |
| Size | 14,971,097 bytes |
| Records | 104,744 |
| Recorded plugin version | `2.6.9.4 (50BCD605C50A749F)`, stated by the log's version records |
| Encounter family | The Abyssal Fracture (Extreme), Zeromus |

This is an existing vendored documentation fixture, rather than a synthetic action generator or a newly captured game session. The hash pins the actual bytes independently of the surrounding checkout. The large source file is not copied into this repository.

## Recorded observations

An initial import through the Rust API and the pinned original FFXIV plugin completed with zero host diagnostics. The values below describe that individual execution. They are integration results from the reconstructed host, rather than an independent comparison with the original ACT application's complete analysis, and numeric totals are not fixed regression expectations.

| Result | Observed value |
| --- | ---: |
| Lines read / raw records dispatched | 104,744 / 104,744 |
| Imported encounters | 3 |
| Sum of per-encounter combatant counts, initial execution | 35 |
| Total encounter damage | 122,118,650 |
| Total encounter healing | 64,058,062 |
| Latest completed snapshot damage | 40,741,359 |
| Latest completed snapshot healing | 21,311,986 |
| Latest snapshot duration | 5,808,330,000 ticks (580.833 seconds) |
| Latest snapshot actors | 13 |
| Initial raw subscription retention / loss | 4,096 retained / 100,648 dropped |

Raw subscription overflow is independent of combat parsing: the initial import still produced its complete host report and snapshot while dropping old observer records.

Later three-pass managed checks compared every run exactly with its own submitted actions. The first pass observed 35 combatants and 34,685 actions; a later pass in the same initialized plugin session observed 36 combatants and 34,782 actions. Encounter count remained three. This variation is recorded explicitly; neither actor counts nor full snapshots are asserted equal across repeated imports.

| Cause or observation | Evidence and interpretation |
| --- | --- |
| Numeric rounding | The shipped `DoTSimulator` uses `Random.NextDouble()` for stochastic rounding, including the default expected noncritical DoT path. Small integer differences between runs are therefore possible before actions reach ACT. Tests do not seed or replace the original RNG. |
| Repeated-session variation | The same pinned source produced different actor/action counts within one initialized parser session. Stochastic amounts alone do not explain the additional actors/actions. |
| Retained parser services | Genuine `EnvironmentState.Reset(true)` retains `_lastCleanTime`, `PartyCount` and the same `PartyMembers` list. Cleanup skips when replay time minus the retained cursor is below 5,000 ms. Reusing this graph is an observed compatibility boundary; the exact cause of each additional action remains unisolated. |
| Isolation recommendation | Use a newly initialized host/plugin session for each independently compared corpus import. An import's live-context restoration does not promise to reset all plugin-private simulation state. |

## Regression contract

`OriginalCorpusChecks.Run(PluginSession, string)` requires the pinned input hash and repeats import three times within one initialized original-plugin session. Its oracle captures the committed `MasterSwing` scalars from `AfterCombatAction`, clones actual outgoing/incoming category links at observation time, and groups actions by encounter identity and resolved combatant. It does not use mutable event-copy fields or the informational damage-swing lists to classify amounts. The helper does not treat raw callback delivery as parser acceptance.

| Check | Purpose |
| --- | --- |
| Exact per-actor outgoing damage/healing and incoming damage/healing | Sum actual positive submitted amounts using captured category routing. Negative `Dnum` sentinels and zero add no numeric amount. Repeated links and ACT's `All` aliases retain their routing multiplicity. |
| Exact per-encounter totals | Select final allies with the encounter's actual `GetAllies()` result, sum the independently derived actor totals, and compare both damage and healing with core getters. |
| Exact report totals and combatant counts | Compare the report with the new encounter/action-derived actor maps from the same run. Keep the known three encounter boundaries without replacing observed actor counts with a tolerance. |
| Exact latest snapshot | Compare its actors, ally flags, numeric amounts and metadata with the latest completed encounter and the same-run action oracle. The recorded duration remains 5,808,330,000 ticks. |
| Increasing snapshot revision and unchanged-revision empty query | Exercise publication semantics. |
| Positive action counts and totals; explicit repeated-import variation | Observe original submissions independently of raw transport, while recording count and numeric changes between runs instead of assuming deterministic parser output. |
| Two-record observer queue | Force loss while requiring unchanged combat reports and snapshots. Each import must lose exactly 104,742 source records. |
| Exact loss and retained sequence intervals | Verify continuous source numbering over all three imports. |
| Zero-capacity drain followed by successful drain | Ensure capacity failure acknowledges neither retained records nor loss. |
| Restored zone identity, zone name, file path, host time, read lock, and character | Verify import returns to the original live context. |
| Zero host diagnostics; strict classified parser messages | Reject host failures and unknown private parser messages. Report recognized simulator warnings explicitly rather than describing the parser as warning-free. |

`OriginalCorpusChecks.CheckMessages` validates the private sink's `[HH:mm:ss.fff] ` prefix and the first three startup message categories in order. Its corpus-only whitelist is deliberately separate from the fixed synthetic fixture's exact three-message contract.

| Private message category | Treatment |
| --- | --- |
| `FFXIV_ACT_Plugin Version:`, `Selected Language ID:`, `Selected Process ID:` | Information, including metadata notices originating from the recorded input. |
| `Detected Process ID:` | Information. |
| `SimulatedDamage Timeout with no ticks/GroundAoE: Removing status ` | Recognized original simulation warning; count and report it. |
| Any other body, malformed prefix, exception or unsupported-operation message | Fail the corpus check. |

The helper prints concise JSON counts for notices and warnings without dumping all private messages. A passing run can contain these recognized original warnings; it does not prove complete DoT simulation fidelity.

The completed three-pass check reported 42 notices, five simulator warnings and
zero host diagnostics. Each pass matched its own action-derived totals exactly
while reporting 104,742 omitted observer records. The source's literal `end`
echo commands also exercise the reconstructed ACT completion command; UI refresh
and command sound are intentionally absent in the embedded host.

The regression helper is maintained separately from the input fixture. Building the helper is not itself evidence that the three-import acceptance ran; execution results must be reported by the invoking test run.

## Limits

| Boundary | Remaining limitation |
| --- | --- |
| Original ACT oracle | Existing genuine-reference getter probes support individual ACT semantics; this corpus has not been compared end to end with a Windows ACT run. |
| Coverage | One encounter family and a legacy recorded plugin version do not establish compatibility with every expansion, log revision, locale, or plugin. |
| EOF | The host drains actions already submitted to ACT. It does not claim to flush the original parser's private deferred events by inventing a final record. |
| Repeated import | Restoring the host's live state does not reset every plugin-private service or guarantee the same actor/action stream. The oracle verifies each actual stream exactly, and cold sessions remain preferable for comparisons. |
| Linux resources | Original embedded resource parsing can retain CR suffixes; see [resource portability](m4-resource-portability.md). Stable values do not imply this portability gap is repaired. |
| Live capture | No native network capture, game process, or live encounter was exercised. |
| Recording | Import and raw observation do not establish disk-writer compatibility or durability. |

## Parser evidence

The ignored `binaries/analysis/synthetic-import-evidence.json` ledger retains the
pinned Parse assembly (`aabdeedb35f0e9fb2a833cc55e11cf9f7684ca06cddbaffa2a791754676afea9`,
MVID `6cbb93e7-5df5-4446-9389-d26bbcb62670`) observations.

| Finding | Exact reference |
| --- | --- |
| Stochastic integer rounding | `DoTSimulator.SimulateTicks`, token `0x06000018`, `Random.NextDouble` at IL 682; `ev_31616134a3c8d4f2cba65506503e3794c8406b8f54b3a214a732df3b263247db` |
| Retained reset/cleanup state | `EnvironmentState.Reset(true)` / `Clean`, tokens `0x06000135` / `0x06000136`; `ev_f2c911df3900fce20ecffd22904e2d49cda6184f0506562999216b4fe7e124eb`; genuine runtime reset probe preserved the cursor and party-list identity |
| Deferred processing units | `DeferredEventProcessor.Process`, token `0x06000009`, compares `TimeSpan.TotalMilliseconds` with 10; the threshold is milliseconds, not seconds |
| Literal end command | Genuine ACT `FormActMain.ActCommands`, token `0x060002cc`, exact `"end"` comparison then conditional `EndCombat(true)`; `ev_42f175b8eb11fc7bd3315f0459c5adb298e0a988999a765b6d143f9c1d5bab90` |
