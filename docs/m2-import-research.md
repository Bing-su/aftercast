# Offline import and combat reconstruction

Research date: 2026-10-20. These are static observations from the supplied
artifacts, followed by a proposed implementation sequence. Later sections record
bounded original-runtime constructor and aggregation probes. No original log
import, game connection or FF Logs upload was executed for this report.

## Fixed inputs

| Artifact | SHA-256 |
| --- | --- |
| ACT 3.8.5.288 | `934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657` |
| FFXIV main 3.0.3.3 | `af1031cdba8eeba8f53dd9d618e58d62d4919bfa77db9b31c568bd04cb359093` |
| Common 3.0.0.0 | `9c3ce07598b6e77a45a6e346bf6c4efbbb09b9cc61a32534f23bc5adcdb03cb1` |
| Parse 3.0.3.3 | `aabdeedb35f0e9fb2a833cc55e11cf9f7684ca06cddbaffa2a791754676afea9` |
| Logfile 3.0.3.3 | `2f4faa819df144ca70a17c17e79e45c9cdecf413c98c12fc42b51d73af10b5e7` |

All five local file hashes were checked again for this report. Metadata tokens
below belong to their named artifact, not a shared token namespace.

## Import path

| Boundary | Observed behavior | Reconstruction requirement |
| --- | --- | --- |
| ACT `ImportFile(bool,bool,DateTime,DateTime)`, `0x060002c5` | Saves live context, switches `LogFilePath`, raises `LogFileChanged(true,path)`, pauses live reading, reads/imports selected records, restores live context/path, raises `LogFileChanged(false,path)` | A library import must coordinate live reading and restore ownership on failure; copying ACT's progress-window UI is unnecessary |
| Import record dispatch | Timestamp parser supplies the record time; `GlobalTimeSorter` increments before `ParseRawLogLine(true,time,line,String.Empty)` | Preserve import flag, timestamp and sequence order; EOF must not imply that pending combat actions have finished |
| Main `OFormActMain_LogFileChanged`, `0x0600004d`, closure `0x060000d8` | Passes the import flag to `ParseMediator.LogFileChanged(bool)` | Dispatch the file transition before its records |
| Parse `LogFileChanged`, `0x06000028` | Calls `_importEnvironmentState.Reset(true)` only for an import transition | Each import starts with fresh plugin import state |
| Main `OFormActMain_BeforeLogLineRead`, `0x0600004e`, closure `0x060000d6` | Calls `BeforeLogLineRead(isImport,logInfo.detectedTime,logInfo.logLine)` and assigns returned text/type back to the event fields | Run mutable hooks synchronously; retain modifications for later handlers |
| Parse `BeforeLogLineRead`, `0x06000029` | Selects distinct live/import state, cleans state, processes deferred events on timestamp changes, selects and invokes a record strategy | Reuse this parser rather than implementing a second interpretation of FFXIV logs |

The selected FormActMain API and main-plugin references use `LogFileChanged` and
`BeforeLogLineRead` with an import flag. They do not establish a separate event
named `ImportLogEvent`. The original 3,629-byte import method also contains date
selection, companion logs and progress UI; a smaller library API must state which
of those options it actually implements.

| `LogLineEventArgs` member | Metadata and behavior |
| --- | --- |
| `logLine`, `detectedType` | Writable public fields; original plugin replaces both |
| `detectedTime`, `detectedZone`, `inCombat` | Readonly public fields describing context at dispatch |
| `originalLogLine` | Readonly original input, preserved when a handler modifies `logLine` |
| `companionLogName` | Readonly; main-file imports pass an empty string |

The plugin timestamp parser (`ParseLogDateTime`, Parse `0x0600002b`) extracts text
between the first two pipes, calls `DateTime.TryParse`, and returns MinValue on
parse failure or a year above 2100. Culture/time-zone behavior needs explicit
tests; replacing it with a different timestamp parser would change the input
contract. ParseMediator reports malformed records through the plugin settings
message sink and catches strategy exceptions internally. Zero ACT diagnostics
therefore does not prove that a record parsed successfully.

| Mediator path | Returned value or failure |
| --- | --- |
| Missing pipe or invalid enum text | Original line and type 0; type 0 is also valid chat, so this is not a unique failure marker |
| Supplied timestamp is MinValue | Original line and parsed type, after a private diagnostic |
| Recognized strategy throws | Private diagnostic, then the normal normalized line and original parsed type; indistinguishable from success by tuple alone |
| PacketDump 252 | Empty displayed line and type 252, including after a caught strategy failure |
| Factory does not support a numeric type | Factory call at IL331 precedes the protected strategy region IL341–384; `NotImplementedException` propagates to the host |

The strategy catches do not protect earlier cleanup/deferred processing either.
Consequently, a raw transport can truthfully count dispatched records, but cannot
derive accepted/parsed record counts from the returned text/type. Preflighting a
noncombat type allowlist bounds the first slice's host requirements; it does not
prove that malformed allowed records succeeded. General import must expose that
analysis is unavailable until those semantics and plugin-private failures are
observable.

## Action and encounter requirements

| Method | Verified contract | Consequence |
| --- | --- | --- |
| Parse `ReportCombatData.AddCombatAction`, `0x0600004f` | Calls `IActWrapper.SetEncounter(time,attacker,victim)`; forwards the action only when the result is true | Returning true without encounter state would manufacture acceptance; false is a meaningful filter outcome |
| Common action closure, `0x06000114` | Constructs `Dnum(long)` and explicit-special `MasterSwing`; assigns the supplied tags dictionary when nonempty; calls ACT `AddCombatAction(MasterSwing)` | Preserve 64-bit and reserved Dnum values, payload identity and arbitrary tags |
| ACT `SetEncounter`, `0x060002cd` | Consults selective parsing, character selection, zone/history, active encounter and segment settings; raises `OnCombatStart`; updates hostile time and combat state | It cannot be replaced with just `InCombat = true` |
| ACT `AddCombatAction`, `0x060002b6` | Throws `InvalidOperationException` when global root `InCombat` is false; applies corrections; invokes mutable/cancelable Before hooks; enqueues the accepted action | Validate state, honor cancellation and commit the modified payload |
| ACT `ThreadAfterCombatAction`, `0x060002b8` | Copies/clears queue under lock; calls `ZoneData.AddCombatAction`; then raises After hooks | Aggregate before After callbacks; raw-log callbacks and action completion do not have a universal total order |
| ACT `EndCombat`, `0x060002cf` | Clears combat state, waits for queued/active actions, finalizes encounters and raises combat-end events; also has optional UI/export paths | Completion must drain accepted actions before returning a final result |
| Parse `StopCombat`, `0x06000050` | Calls wrapper `EndCombat(force)` only when wrapper `InCombat` is true | Even a territory record can touch host combat state |

Aggregation descends through `ZoneData`, `EncounterData`, `CombatantData`,
`DamageTypeData` and `AttackType`. Plugin-populated category links determine which
actions contribute to which views. An action list is useful evidence, but does not
establish ACT-equivalent totals, encounter segmentation, duration or exports.

## Inputs available without a game

| Input candidate | Static evidence | Useful assertion |
| --- | --- | --- |
| Shipped examples | The supplied SDK directory contains nine DLLs; ACT ZIP contains six executables/configuration/launcher files; no sample log or packet capture was found in those supplied files | No combat corpus is available from this inventory |
| PlayerStats, decimal type 12 | Factory selects `ParseStrategyDoNothing`; original `LogFormat.FormatPlayerStatsMessage` is available | Complete noncombat import framing, hook mutation and record counts |
| Current player, type 2 | Strategy `0x060001c5` requires at least four fields, parses field 2 as hexadecimal player ID and updates parser state | Separate/reset import state without requiring capture |
| PacketDump, type 252 | Factory selects DoNothing; mediator returns an empty displayed line for this type | Hidden-record handling without fabricated combat |
| Version/settings, types 253/249 | `LogFormat` has a parameterless constructor and original `FormatVersion`, `FormatMemorySettings`, `FormatParseSettings` helpers | Generate original message bodies from fixed arguments and inspect plugin messages |
| Territory, type 1 | Strategy `0x060001dd` parses hexadecimal zone ID, resets state on change, resolves resource name, calls `StopCombat(false)` and `ChangeTerritory` | Later semantic zone-transition test after real host zone/combat support exists |
| Combat actions | Supply deterministic `MasterSwing` payloads directly, independently of capture | Compare aggregation boundaries and results against original ACT before adding complicated record fixtures |

Record numbers were read from the original Logfile enum constant table, not
inferred from field order. Factory CIL independently selects ActionEffect for
types 21/22, Death for 25, AddStatus for 26 and RemoveStatus for 30. Known no-op
types do not establish that arbitrary input records are supported.

The DebugMessage strategy (`0x060001ca`), used by types 249/253, forwards joined
body fields to the settings `ProcessException` action despite that action's name.
It also carries ordinary version/settings notices. Counting all new plugin
messages as errors would therefore reject valid input; error reporting needs
the producer's context rather than that sink's name alone.

Use the original `LogFormat` to build message bodies and the original writer where
possible. Hand-wrapped parser fixtures must be labeled as such: a valid pipe
layout is not an original recording or an FF Logs-compatible checksum. The
writer's timestamp/checksum rules are described in [SDK analysis](ffxiv-act-sdk-analysis.md).

## Proposed functional slices

| Slice | Deliverable | Acceptance boundary |
| --- | --- | --- |
| Import transport | Read a complete fixed UTF-8 file from the beginning; coordinate live reading; invoke import transitions and mutable log hooks; expose counts and failures | Recognized noncombat fixtures parse with expected rewritten text/type, reset state between imports, restore live delivery after failure, and produce no unexpected plugin messages; aggregation remains explicitly unavailable |
| Combat object graph | Implement actual category-linked state and verified getters using deterministic actions | Damage/heal/critical/miss/death/tag cases, incoming/outgoing categorization, segment duration and integer totals match an original ACT reference run |
| Action/encounter orchestration | Connect SetEncounter, Before/action queue/aggregation/After, zone changes and drained combat end | Filtering, cancellation, modified payloads, pending actions at EOF and repeated encounters match the reference; unsupported options fail visibly |
| Original semantic imports | Feed fixed actor/player/party/zone/action records through the reused plugin | Compare both emitted action evidence and final ACT results with fixed plugin/settings/input hashes; exercise repeated imports and failure recovery |

The first slice can ship as input functionality, but must not mark M2 aggregation
complete or turn unsupported combat records into apparently successful empty
results. A full no-game import can be validated with synthetic records, while
credible gameplay equivalence still requires a representative combat corpus and
an original ACT baseline. Preserve those as separate acceptance claims.

## Evidence ledger and unresolved work

| Scope | Retained REA evidence |
| --- | --- |
| ACT host, import/action/encounter methods | `ev_e10bfceb62a78ed060b028138f7963d250457608d5f6551af9718f399d5894c9` |
| ACT event payloads and aggregate types | `ev_2fb5619a651ba480afdbaba659cf3ae298a1269cfe5108d9fcfa5e6a37bfa03d` |
| Main import hooks and closures | `ev_ad7641feec72d06f1e82ffc7dfa63bc104ff2f04fa1754d4e4ec55d74681ab87` |
| Parse mediator, strategies and reports | `ev_499142007ecbb9b10fe2807f40a6ffe2fecea996149478cbc355379eb236f3f6` |
| Common wrapper construction and forwarding | `ev_1bd122a43c24894f3a3f64c7de67bfc677124a1b5cb7a2be2ca9b2d4b6864f9f` |
| Original formatter/writer metadata | `ev_65976a60069ca4271098eed44d9bde478731140b09bbec7af16210abcc3912e9` |

Local ignored evidence is in `binaries/analysis/act-host-2026-10-20-evidence.json`
and `sdk-3.0.3.3-core-evidence.json`. The existing PEReader helper was reused to
read selected CIL into `m2-parse-import-il.txt`, `m2-parse-strategies-il.txt` and
`m2-date-and-stop-il.txt`; `m2-log-message-constants.fsx` reads enum constants
without loading target assemblies. These checks inspect bytes, not execution.

| Unresolved | Required evidence |
| --- | --- |
| Original ACT differential runner | Shared constructor/value probes pass against both implementations; the configured genuine action baseline below is the next replacement acceptance target |
| Import edge behavior | Fully restore pause/live context after cancellation, timestamp failure and plugin failure; compare EOF/last-line and date-range behavior |
| Aggregation options | Restore exact correction, selection, segmentation, ally/category and export rules before claiming equivalence |
| Deferred combat effects | Fixed timestamp-transition/EOF tests for DoT/HoT and deferred actions; EOF must not silently discard pending work |
| Real input coverage | Obtain a version-pinned combat corpus; none is supplied by the artifact inventory above |
| Modern runtime behavior | Exercise actual import callbacks, plugin-private errors and generated writer records on .NET 10; static method selection alone does not establish JIT/runtime compatibility |

## Original constructor baseline, 2026-10-21

The isolated [OriginalReference runner](../managed/tests/OriginalReference/README.md)
has no dependency on ActCompat. It loads the hash-checked original ACT EXE and the
explicitly supplied headless Forms DLLs in a separate .NET 10.0.13 Linux process.
It calls original `ActLocalization.Init()` then `AddPrebuild()` to populate 781
shipped strings. Static analysis found only dictionary/localization-object
construction in those routines; no entry point, main form, updater or capture is
invoked. `ActGlobals.oFormActMain` remains null.

| Executed stage | Result |
| --- | --- |
| Genuine `Dnum(1234567890123)` and formatting | Number preserved; default text `1,234,567,890,123` |
| Genuine `ZoneData` constructor | Fixture zone and UnixEpoch start preserved |
| Genuine `EncounterData` constructor | Character and actual parent zone preserved |
| Genuine `CombatantData` constructor | Character name and actual parent encounter preserved |
| Genuine encounter metadata getters | Zone `Fixture Zone`, title `Encounter` |
| Original Dnum value observations | Reserved values, custom labels/suffixes, null/wrong-type behavior, arithmetic overflow, derived type, culture and grouping switches execute |
| Shared original-reference probes | `DnumValueProbe`, `CombatStateProbe`, `DamageStateProbe` all pass against the genuine ACT assembly in this process |

The first run exposed missing `FormWindowState` metadata in the Forms adapter;
that contract was added. A later run exposed missing default localization tables;
calling the two genuine initialization routines resolved it. Neither finding
establishes a Windows-only baseline requirement. Ignored runtime observations are
`binaries/analysis/m2-original-constructors.jsonl` and
`m2-original-dnum-values.jsonl`; the constructor run reports zero failures.
The shared probes additionally verify eight zone flag combinations, merged-child
behavior, sentinel-list mutation/ownership, category insertion order, history and
log-line records, nullable constructor values and virtual category identity. The
ignored record `m2-original-contracts.jsonl` captures those genuine implementation
results. Constructor success does not establish original FFXIV import behavior.

The replacement owns the same constructor-created zone, encounter, combatant,
category and attack collections, with shared probes checking identity, mutability
and defaults. The action slice below extends these real objects.

## Configured original action baseline, 2026-10-21

The `actions` mode executes three genuine `MasterSwing` objects through original
`ZoneData.AddCombatAction` and an original nonselective encounter. The fixture
uses actual public registration fields and `EncounterData.SetAllies`; it does not
create or seed an uninitialized root or aggregate object.

| Explicit fixture configuration | Value |
| --- | --- |
| Outgoing categories, in order | Fixture Outgoing Damage, Fixture Outgoing Healing, Fixture All Outgoing |
| Incoming categories, in order | Fixture Incoming Damage, Fixture Incoming Healing, Fixture All Incoming |
| Swing 1 links | Outgoing Damage / Incoming Damage; damage swing list contains 1 |
| Swing 3 links | Outgoing Healing / Incoming Healing; healing swing list contains 3 |
| Ally values | Damage -1, healing +1, all-actions category 0 |
| Character and manual allies | Source; explicit ally list contains Source |
| Action 1, UnixEpoch +0 seconds | Source → Target, Strike, 100 damage, not critical |
| Action 2, +1 second | Source → Source, Heal, 20 healing |
| Action 3, +2 seconds | Source → Target, Strike, 300 damage, critical |

| Executed query | Original result |
| --- | --- |
| Source damage / healed | 400 / 20 |
| Source swings / hits / critical hits / heals | 2 / 2 / 1 / 1 |
| Source incoming damage / healing | 0 / 20 |
| Target incoming damage / healing | 400 / 0 |
| Encounter damage / healed / duration | 400 / 20 / 2 seconds |
| Encounter combatants / active | 2 / true |
| Main form | Null throughout |

Registration order matters: the original combatant constructor retains the last
outgoing/incoming category as its all-actions accumulator. Omitting those final
categories causes genuine cross-category accumulation, rather than a reliable
fixture. Automatic `GetAllies()` also reaches a null-root `WriteDebugLog` call;
the successful fixture uses the real public manual-ally API instead. This is
explicit test configuration, not a reconstruction of FFXIV defaults.

Ignored evidence is `binaries/analysis/m2-original-actions.jsonl`; the isolated
action run exits successfully. This establishes a usable genuine aggregate
baseline without Windows or a game for this configured case. Host SetEncounter,
corrections, hook/action-queue ordering, automatic ally resolution, combat end and
original FFXIV imports remain outside this execution.

## Reconstructed configured actions, 2026-10-21

The shared `ChildActionProbe` and `CombatActionProbe` compile against the original
ACT metadata and pass in separate processes against both genuine ACT and the
replacement. These are configured direct-action comparisons, not imported game
logs or evidence that host `SetEncounter` is implemented.

| Implemented boundary | Compared behavior |
| --- | --- |
| Zone/encounter routing | Merged and individual encounters receive the same action, participant names resolve case-insensitively through current-culture uppercase keys, duplicate detection checks time sorters |
| Category routing | Registered swing links, last-registered all-actions categories, detailed attack suppression through `restrictToAll`, incoming and outgoing contribution |
| Counts and totals | Damage, healing, hits, critical hits, swings, misses, blocked actions; reserved `Dnum` labels participate in equality |
| Cache behavior | Independent append cursors, explicit full invalidation, global forced recomputation, same-count replacement remains cached, integer overflow retains original Dnum conversion behavior |
| Timing | Action min/max times, manual ally selection, encounter open/closed segments, `EndCombat(false)` and the 400 damage / 20 healed / 2-second reference case |
| Lifetime | Session acquisition and disposal release plugin registries/category names and restore built-in calculation flags |

| Not implemented in this slice | Visible behavior |
| --- | --- |
| Selective action parsing | Reject before changing the encounter or action ownership |
| Custom localization terms | Verified built-in English terms are used; external ACT localization overrides remain unsupported |
| Host action orchestration | The later [headless root slice](m2-root-combat.md) adds nonselective selection, hooks/queue and drained combat end; full corrections and semantic imports remain open |

The replacement-only policy checks verify that rejected selective parsing
does not leave a partially mutated graph. Supported constructor and action cases
continue to run against genuine ACT rather than an invented aggregate baseline.

## Ally graphs, finalization and segmented duration

| Implemented behavior | Validation boundary |
| --- | --- |
| Automatic allies | Genuine and replacement action-derived signed relationship graphs agree for damage, healing, disconnected actors, unknown actors, cancellation of weights and manual-list override |
| Graph cache | Original sorted-node traversal, invalidation and limited-read behavior are preserved; caller-supplied manual lists retain their identity |
| Enemy/title selection | Integer damage-per-death scores convert through single precision before descending ranking; calculation is reconstructed from CIL, with the original null-root runtime limitation below |
| Encounter finalization | `EndCombat(true)` closes the segment, recursively trims actual collections and selects the title; the genuine null-root fixture can compare segment/trim effects before its debug failure |
| Personal duration | Attack/category/combatant scans preserve sequence-number-first action ordering, open and closed segments, original gap/boundary quirks, independent cache invalidation and malformed-list exceptions |
| Numeric rates | Attack/category DPS guards and combatant unguarded division preserve their distinct zero-duration results, including NaN and infinity; deprecated `AttackType.ExtDPS` retains its obsolete metadata |
| Combat event payloads | Original-reference constructor/delegate probes verify mutable scalar snapshots, readonly action/encounter references, cancellation and shared tag-dictionary identity; root notification delivery is still a separate milestone |
| Debug messages | `WriteDebugLog` sends best-effort output to application-owned .NET trace listeners, separate from host error history; listener failure does not interrupt calculation |

Original ACT's standalone `GetAllies` and segmented combatant duration getters
cache their real result before dereferencing the absent main form for a debug
message. The reference probes accept that failure only for the pinned genuine
artifact, exact method and committed private cache, then read the same cached
result. Replacement exceptions are never accepted by that path. Standalone
replacement calculations work without a UI root; hosted calculations can use the
trace adapter.

`GetStrongestEnemy` instead reaches the original debug call before returning the
selected name. Its null-root failure is bounded to the pinned method and observed
IL offset 248 on .NET 10.0.13. Original title-return equivalence is therefore not
claimed: ranking follows the inspected CIL, replacement title assertions execute,
and genuine finalization assertions cover completed segment and trimming work.
The fixture deliberately gives collections spare capacity before checking trims;
`TrimExcess` need not shrink a nearly full collection.

The additional graph/ranking/lock metadata is retained in
`binaries/analysis/automatic-allies-evidence.json`, including
`ev_d830369638953f567ce88dfa21ed5cbd4aae845bba635987f8a695c25b8eeb36`.
Combat event metadata is retained in `continuous-startup-evidence.json` as
`ev_25f8eae1a5f2067b4558699abe21f365b8df6e558b173ff35b10ea0744b1c10c`.
