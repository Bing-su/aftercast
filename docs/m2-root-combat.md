# Headless combat orchestration

Implemented and checked on Linux/.NET 10.0.13, 2026-10-21. This connects the
reconstructed ACT data graph to host combat methods and an asynchronous action
worker. Direct graph comparisons still use the genuine ACT assembly; root
orchestration follows inspected CIL and replacement regression tests, rather than
claiming a successful original main-form execution.

## Supported policy

| Area | Behavior |
| --- | --- |
| Parsing selection | Nonselective encounters; the selective data constructor still rejects action insertion |
| Initial zones | Import sentinel at index zero and an initial `Unknown Zone` live zone |
| Merged encounters | Enabled, matching the original shipped `cbZoneAllListing` default |
| Idle combat end | Parsed `LastKnownTime - LastHostileTime` strictly greater than six seconds, checked before live raw hooks; no wall-clock timer |
| Zone changes | Record new context without moving an ongoing encounter; the next combat can reuse or chronologically insert its zone |
| Root properties | Preserve public setters for `ZoneList`, `ActiveZone`, `InCombat`; `LastHostileTime` has a private setter |
| Combat counts | Damage/healing, hits, criticals, misses, blocked actions, kills/deaths, allied totals and numeric DPS getters use actual linked actions |
| Kill classification | Preserve original `Killing`/`All` fallback, Dnum equality, ally membership and literal-space filtering, including cache quirks |
| Commands | `ActCommands` explicitly rejects unsupported ACT UI/chat commands |
| Incoming hit rate | Install ACT's `DamageTypeData` `ToHit` base column so the original FFXIV `IncToHit` callback can chain it |

History databases, root settings UI, user rename/redirect rules, automatic exports,
silence-based segmentation, selective parsing and trigger commands are outside
this policy. The later [file-import slice](m2-file-import.md) uses the import
sentinel and adds a Rust API with separate semantic acceptance checks.

## Delivery and ordering

| Stage | Observable contract |
| --- | --- |
| Start | Prepare the real encounter, invoke `OnCombatStart` while `InCombat` is false and `LastHostileTime` retains its previous value, then commit both fields |
| Admission | Reject actions outside combat or after shutdown; count admitted producers before invoking any hook |
| Normalization | Normalize whitespace-only / `hit` / `hits` special text to the verified built-in `None`; trim attacker, victim and attack type on the original action |
| Before hooks | Run synchronously on the producer thread, share mutable event state, isolate subscriber exceptions and stop immediately on cancellation |
| Commit | Preserve original string interning and commit order; shared tag mutations remain visible, but replacing `event.tags` does not replace `MasterSwing.Tags` |
| Queue | Preserve enqueue order after Before hooks; producers already admitted before combat end or shutdown may finish and enqueue into that drain |
| Worker | Aggregate into the active zone before `AfterCombatAction`; run After hooks on the worker outside the graph monitor, isolate errors and continue |
| End | Clear `InCombat`, wait for admitted producers, queued actions and current After callbacks, finalize merged/individual segments, then invoke `OnCombatEnd` |

The host selects ACT's isolated-handler behavior. It does not expose ACT's grouped
multicast/debug-handler setting. After scalar edits are event snapshots and are
not committed to the action. Canceling a Before event prevents later Before
subscribers from undoing cancellation. String interning, like the hosted CLR,
remains process-owned across host restarts.

There is no universal total order between raw log callbacks and After callbacks.
Start/end event indices preserve ACT's historical last-zone-index quirk after
inserting or reusing an older zone; the event's encounter reference is the actual
active object. Reentrant start/zone changes during a transition fail visibly.

## Overload and teardown

| Condition | Result |
| --- | --- |
| Queue reaches 4,096 pending actions | Record one backlog warning and continue parsing; accepted actions are not discarded |
| Queue falls below 2,048 | Re-arm the warning for a later backlog episode |
| Backlog warning on the initialization thread | Retain it in `Host::diagnostics`; it does not count as an initializer failure |
| Individual plugin deinitialization | Pause file delivery and new combat admission, drain existing transitions/actions, execute teardown, resume in `finally` |
| New action during deinitialization | Reject before Before hooks; this is a lifecycle admission failure, not a silently discarded queued action |
| Whole-host stop | Close new admission before joining readers, drain existing transitions/actions while controls/globals are valid, stop/join the action worker, then deinitialize plugins and release globals |
| Worker synchronously invokes owner control | Owner waits cooperatively pump callbacks; a callback cannot wait for its own producer/transition |
| Action callback tries to join a log reader | Reject before detaching it, preventing a cycle when the reader is itself waiting for the action drain |

The action queue intentionally has no discard threshold. Warnings do not provide
a memory bound: the queue and retained ACT action history still require enough
memory for their workload. Plugins must finish their own producers during
`DeInitPlugin`; new background requests during its admission pause fail visibly.
Callbacks cannot be preempted by the host.

Lifecycle commands stay on the host's creation thread, as required by the Rust
`Host` API. Background plugin threads may submit actions; they do not own host
deinitialization or disposal.

## Verification and evidence

| Check | Scope |
| --- | --- |
| Shared original-reference probes | Value/graph/actions, automatic allies, finalization boundaries, segmented durations, kill/death totals and numeric rates pass in separate genuine/replacement processes |
| Root state tests | Defaults, setter visibility, transition phase, chronological zone reuse and event-index behavior |
| Root pipeline tests | Before mutation/cancellation, tag identity, interning, subscriber failure isolation, aggregation before After, final results and strict idle boundary |
| Concurrency regressions | Admitted producer during End, 4,097-action backlog continuation, pumped action/start/end shutdown, teardown admission and failure recovery, cross-pipeline reader-join rejection |
| Original FFXIV observation | Copied 3.0.3.3 deployment initializes as `Started`, reports no host errors and shuts down without a game; this does not yet establish semantic parsing |

ACT root/action evidence is retained under
`ev_e10bfceb62a78ed060b028138f7963d250457608d5f6551af9718f399d5894c9`.
Original initialization/default-option observations are in
`binaries/analysis/automatic-allies-evidence.json`; data comparison boundaries are
documented in [the M2 research ledger](m2-import-research.md).

## Hit-rate column boundary

| Contract | Verified behavior |
| --- | --- |
| `AttackType.ToHit` / `DamageTypeData.ToHit` | Single-precision `Hits / Swings * 100`; empty data yields NaN; an exception while reading malformed caller-owned data yields zero |
| Existing hit caches | Changing `blockIsHit` alone retains cached counts until explicit invalidation |
| Base cell / SQL | Cell uses `#,0.00` or `0.00` according to `mainTableShowCommas`, current culture, no percent suffix; SQL uses the float's ordinary current-culture string |
| Registration | Host acquisition installs only the required `DamageTypeData.ColumnDefs["ToHit"]`; disposal removes it and the next host reinstalls it |
| Unknown/failed columns | Unknown key remains empty; null keys and callback exceptions retain their original failure behavior |
| Original FFXIV callback | Synthetic imports verify `Fixture Enemy`'s incoming hit rate through the plugin's actual `IncToHit` callback, alongside the action/totals assertions |

This is a bounded base-column slice, not the complete ACT table/export registry.
The shared `HitRateProbe` passes against genuine ACT and the replacement; separate
host tests cover metadata, registration lifetime and en-US/fr-FR formatting.
Getter evidence is `ev_84bbd8e83aab244280224e1acccfac462086eb786f3068a131ab2e11cc570918`;
the original base callbacks and plugin dependency are retained in
`binaries/analysis/synthetic-import-evidence.json` as
`ev_608e7e5aed8b8c71188705ebf9264a6f6f41a55fcb9c468027a7305ddbdaf275`
and `ev_3143f16310494368a2903f087b1d67b7dd966a7a347cc15c249d7fa14aeb2692`.
