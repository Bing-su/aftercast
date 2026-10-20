# ACT host compatibility analysis

Research date: 2026-10-20. Static observations from original binaries, not an ACT
replacement implementation or successful plugin initialization. The approved
[blueprint](blueprint.md) assigns ACT contracts/aggregation to F# and exposes a Rust
library; the current M0 session scaffold does not satisfy these contracts.

## Fixed targets

| Target | Identity |
| --- | --- |
| ACT | binaries/act/Advanced Combat Tracker.exe; 3.8.5.288; .NET Framework 4.7 CIL, AnyCPU |
| ACT SHA-256 | 934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657 |
| ACT MVID | 94f61cd2-1f88-4a7c-98d1-0646897b1fb4 |
| ACT assembly | Advanced Combat Tracker; public key token a946b61e93d97868; strong-name-signed flag |
| FFXIV main plugin | SDK 3.0.3.3; SHA-256 af1031cdba8eeba8f53dd9d618e58d62d4919bfa77db9b31c568bd04cb359093 |
| Common | Assembly 3.0.0.0; SHA-256 9c3ce07598b6e77a45a6e346bf6c4efbbb09b9cc61a32534f23bc5adcdb03cb1 |

Metadata tokens below are scoped to these exact files. An x86 PE machine field
alone does not override the managed AnyCPU classification.

## Registration and initialization

| ACT method | Token | Observation |
| --- | --- | --- |
| IActPluginV1.InitPlugin | 0x060005a6 | void(TabPage, Label) |
| IActPluginV1.DeInitPlugin | 0x060005a7 | void() |
| FormActMain.AddPluginPanel | 0x060001fb | Creates file/UI/ActPluginData and registers the entry |
| PluginListingPanel.cbEnabled_CheckedChanged | 0x060004e5 | AppDomain.Load(bytes), GetTypes/GetInterfaces, actual Type equality, CreateInstance, InitPlugin |
| Same disable branch | 0x060004e5 | Removes tab, calls DeInitPlugin, clears pluginObj, disposes tab |
| PluginCheckInterface | 0x060001f9 | Separate AppDomain, callback/result, unload |

ActPluginData.pluginFile/pluginObj/tpPluginSpace/lblPluginStatus/cbEnabled and
ActGlobals.oFormActMain/charName are public fields. Properties with matching names
are not binary-compatible replacements. Preserve static/instance distinctions.
DLL hosting need not reproduce ACT's C#/VB source compilation feature, but actual
plugin-consumed contracts remain necessary without the registration screen.

The main plugin contains 79 MemberRefs across 16 Advanced_Combat_Tracker parent
types, plus nested definitions. This is a partial static count, not the complete
minimum API including reflection and transitive dependencies.

| Area | Referenced examples |
| --- | --- |
| Host | ActGlobals.oFormActMain, charName |
| Combat | SetEncounter, EndCombat, ChangeZone, InCombat |
| Actions | MasterSwing, Dnum, AddCombatAction, GlobalTimeSorter |
| Logs | BeforeLogLineRead, LogFileChanged, GetDateTimeFromLog, LogLineEventArgs, OpenLog, ReadThreadLock |
| Definitions | CombatantData, EncounterData, DamageTypeData, AttackType, ColumnDefs, ExportVariables, swing maps |
| Settings/UI | SettingsSerializer, TraySlider, GenerateAttackTypeGraph, ValidateLists, ValidateTableSetup |
| Update/error | PluginGetSelfData, downloads/version checks, UnZip, RestartACT, AppDataFolder, error/debug logs |

Common IActWrapper.AddCombatAction (0x0600002e) receives int swingType, bool
critical, string special, long damage, DateTime time, int timeSorter, four strings
attackType/attacker/damageType/victim, and Dictionary<string,object> tags. The
concrete wrapper creates Dnum/MasterSwing and assigns tags. Do not reduce reserved
Dnum values or arbitrary tags to a single numerical damage field.

## WinForms and binding

| Main-plugin method | Token | Observation |
| --- | --- | --- |
| InitPlugin | 0x06000105 | 64-bit/library checks, IoC/resources/update/UI/settings/Deucalion/memory initialization |
| ACT_UIMods.ConfigureUI | 0x0600004c | Finds ACT control tree, connects Clear Click and log events |
| LoadPluginSettingsControl | 0x0600004f | Sets Dock and adds control to TabPage.Controls |
| VerifyAssemblyVersions | 0x06000107 | Machina, Machina.FFXIV, Common and embedded library version/token checks |

ConfigureUI searches `tc1\tpMain\pLeftView\pTvBtns\tableLayoutPanel11\btnClear`
(#US 0x7000056d), connects parser OnClear, BeforeLogLineRead, and LogFileChanged.
Hiding a settings window does not remove control/thread/message-loop dependencies.
Direct SDK composition is an alternative, not proof of ordinary InitPlugin support.

Main and Common reference Advanced Combat Tracker, Version=3.8.5.288,
PublicKeyToken=a946b61e93d97868. Full original-plugin binding remains unverified;
the later [M1 experiment](m1-contract-binding.md) verifies a selected Dnum reference
against our replacement on .NET 10. Observed VerifyAssemblyVersions messages name Machina 2.3.1.3,
Machina.FFXIV 2.4.8.1, Common 3.0.0.0 and embedded libraries; do not infer that this
method directly checks ACT's public key. F# does not solve assembly identity,
Framework API availability, WinForms, or Windows native dependencies by itself.

## Event and aggregation semantics

| ACT method | Token | Restored behavior |
| --- | --- | --- |
| ParseRawLogLine (four arguments) | 0x060002c7 | Under lock, BeforeLogLineRead; reread modified logLine/detectedType; later OnLogLineRead |
| AddCombatAction(MasterSwing) | 0x060002b6 | Reject when not InCombat; trim/rename/redirect; BeforeCombatAction; cancellation/modifications; enqueue |
| ThreadAfterCombatAction | 0x060002b8 | Snapshot/clear queue; ZoneData.AddCombatAction; AfterCombatAction |
| ZoneData.AddCombatAction | 0x060007b1 | Overall/individual encounter and segment/active updates according to settings |
| EndCombat | 0x060002cf | Clear InCombat; wait for pending queue and active snapshot; finish encounter and OnCombatEnd |
| CheckIdleEndCombat | 0x060002b1 | Settings and LastKnownTime minus LastHostileTime drive termination |

The action worker does not establish a universal order between AfterCombatAction
and OnLogLineRead. LogLineEventArgs.logLine/detectedType are mutable fields;
detectedTime/detectedZone/originalLogLine are init-only. CombatActionEventArgs
includes mutable damage/names/time/tags/cancelAction. Copying and ignoring changes
breaks the contract. Handler exception/debugHandlers behavior also differs from
aborting a session on any callback error. See [delivery research](act-event-delivery-analysis.md).

## Duration and DPS

| Getter | Token | Static rule |
| --- | --- | --- |
| Encounter Duration | 0x0600079b | Sum End-Start across segments; last open segment uses get_EndTime; observed exception path returns TimeSpan.Zero |
| Single-segment duration | Same | EndTime > StartTime gives difference; otherwise Zero |
| Encounter EndTime | 0x06000799 | longDuration selects maximum combatant end or ShortEndTime path |
| Combatant Duration | 0x06000741 | Multi-segment copy/sort/segment/cache path; not fully restored; valid single-segment difference or Zero |
| Combatant DPS | 0x06000754 | Damage / own Duration.TotalSeconds |
| Combatant EncDPS | 0x06000755 | Damage / parent encounter Duration.TotalSeconds |
| Combatant EncHPS | 0x06000757 | Healed / parent encounter Duration.TotalSeconds |
| Encounter DPS | 0x060007a1 | Encounter damage / encounter Duration.TotalSeconds |

For example, closed segments [0,10] and [20,30] yield 20 seconds, not 30. This is
an explanatory vector, not an executed ACT result. The small DPS getters contain
no observed zero-denominator guard. Do not impose a one-second minimum or replace
all non-finite values with zero without comparing getters, display, and export.

## Evidence and limits

| Scope | REA parent evidence ID |
| --- | --- |
| ACT artifact | ev_483ce29795ddeff4a55b0da84c7b6db403f2268f646489057ce9360dec1887de |
| Eight host/interface/aggregation types | ev_e10bfceb62a78ed060b028138f7963d250457608d5f6551af9718f399d5894c9 |
| Six event/zone/damage/attack types | ev_2fb5619a651ba480afdbaba659cf3ae298a1269cfe5108d9fcfa5e6a37bfa03d |
| PluginListingPanel | ev_7ff174463d0086dfb5958a4fd8dbddf8d567182a95189c08a9ccd2507aa86c78 |
| Plugin wrapper/UI/init selection | ev_e34271e902daeebb7244c5c6d537fed8b0a7269754cfb0ff3ddd714e90bca67c |
| Main artifact | ev_7f739d52cda237f7fd3d3c1a4f4aeb67959061566ba6e99cfbcda8548630fd72 |
| Common artifact | ev_885edcb4923504b6051fa5011dffb9157e5366daace11ae443bf41d59dd2a699 |
| Common contracts | ev_1bd122a43c24894f3a3f64c7de67bfc677124a1b5cb7a2be2ca9b2d4b6864f9f |

Local ignored evidence: binaries/analysis/act-host-2026-10-20-evidence.json,
act-host-plugin-contract-2026-10-20.json, act-host-metadata.fsx, and
act-host-selected-il.txt. PEReader/F# Interactive checked five selected IL hashes
and instruction counts without loading the original ACT/plugin assemblies. SDK
first-run setup reported a development HTTPS certificate installation; it did not
execute the target or alter plugin settings.

Static coverage complete refers only to selected metadata/CIL. Full ACT API,
reflection dependencies, original-plugin binding/init, ACT differential runs,
Windows execution, real-game capture, and overlays remain unverified. M0's own F#
bridge execution must not be confused with executing these original artifacts.
