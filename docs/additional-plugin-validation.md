# Additional ACT plugin validation — 2026-10-21

Five plugins were downloaded from the official
[ACT downloads page](https://advancedcombattracker.com/download.php).
They exercise settings, another game's parser, event instrumentation, spell
timers and export workflows. All results below are Windows x64/.NET 10 results
against aftercast's ACT 3.8.5.288 identity.

## Current verified workflows

| Download                | Verified result                                                                            | Scope limit                                                                                |
| ----------------------- | ------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| Sample                  | Constructor, initialization, settings round trip and cleanup                               | The sample deliberately has no combat callback implementation                              |
| EQ2 English parser      | Four raw records produce 400 damage / 50 healing; captured actions agree with original ACT | Controlled fixture; gameplay and complete parser coverage unverified                       |
| Event Handler Benchmark | Four event handlers are counted, a slow handler is logged, disabling restores handlers     | Windows native controls; no exact theme/rendering or exception isolation equivalence claim |
| Timer Repeater          | Expiry, combat-only repetition and native-button cancellation                              | Silent timer definitions; audio, timer editor and visible rendering unsupported            |
| Export_All              | Two live combat ends copy 100 then 400 cumulative damage from All                          | Fixture formatters and local clipboard; broader export modes remain unsupported            |

All five pass initialization and cleanup with zero host diagnostics in their
tested workflows. Later sections retain the initial failures and evidence for
each correction; these are not current blockers.

## Input provenance

Downloads and extracted sources remain local under
`binaries/additional-plugins-20261021/`, excluded from Git and Cargo packages.
Sources were compiled unchanged against the supplied genuine ACT executable
using the installed .NET Framework C# compiler, not against aftercast. These
derived DLLs are local test inputs, not vendor-distributed binaries. The benchmark
DLL was used exactly as distributed.

| Plugin                                                  | Official download                                             | Downloaded file             | SHA-256 of downloaded file                                         |
| ------------------------------------------------------- | ------------------------------------------------------------- | --------------------------- | ------------------------------------------------------------------ |
| ACT Plugin Sample, assembly 1.0.0.2                     | [ID 32](https://advancedcombattracker.com/download.php?id=32) | `ACT_Plugin_Sample.zip`     | `bfcc3573729c27862b8cbe731ab3072f3640472f1cf27d062f0f5962af0a4024` |
| EQ2 English Parsing, assembly 1.4.2.30                  | [ID 46](https://advancedcombattracker.com/download.php?id=46) | `ACT_English_Parser.cs`     | `5dd5c8f9342e4c495deb75d02c9976bb34fef5827d26425af024b2e821b84d07` |
| Event Handler Benchmark, assembly 1.0.3.4               | [ID 84](https://advancedcombattracker.com/download.php?id=84) | `EventHandlerBenchmark.zip` | `290c3ec879c775724a8fc75017df611b3213e3083671991f7e462c6b0b6cbd11` |
| Timer Repeater, file version 1.0.2.3                    | [ID 27](https://advancedcombattracker.com/download.php?id=27) | `TimerRepeatPlugin.cs`      | `6e10ca61592508296bb75efc9de32b050823b72729ce0cdd8a2aae1bb74ccc6e` |
| Export the All encounter to clipboard, assembly 1.0.0.0 | [ID 17](https://advancedcombattracker.com/download.php?id=17) | `Export_All.cs`             | `6364bd9eef4641aee63455c367dcac50b02553dd87655186f90a5dad3540f46b` |

The extracted benchmark DLL has SHA-256
`f28ff92106b6ac216fdede0d4b7a66cf296a5493cf4b9de81eff0ed83f141f2d`.
REA metadata inspection records assembly version 1.0.3.4, AnyCPU CIL and a
.NET Framework 4.0 Client target. Evidence:
`ev_697d523acd0574b09950516f1397144edd2f604763937ed5331f845ec036c6c3`;
member inspection:
`ev_4faed82e214b8c67a4f2e121f7008ac4b3cfc807054b8bba8789eeded7d08178`.
Static inspection does not establish successful execution.

## Initial observed results

| Plugin / type                                  | Constructor                             | Initialization / tested workflow                                                                                                                        | First blocker or limit                                                                                                                                                                              |
| ---------------------------------------------- | --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Sample / `ACT_Plugin.PluginSample`             | Pass after correction                   | InitPlugin, 1.5 seconds of pumping, DeInitPlugin and shutdown pass with zero host diagnostics; a seeded TextBox setting is restored and saved unchanged | Its combat-action callback deliberately throws `NotImplementedException`; it is a settings/lifecycle sample, not a parser                                                                           |
| EQ2 / `ACT_Plugin.ACT_English_Parser`          | Pass after correction                   | InitPlugin fails                                                                                                                                        | Missing `FormActMain.get_OptionsTreeView()`; source also requires OptionsControlSets, encounter-log colors, spell-timer mods, localization, text export and additional action overloads             |
| Benchmark / `ACT_Plugin.EventHandlerBenchmark` | Type selection fails before constructor | Not reached                                                                                                                                             | Loader rejects its ACT 3.5.0.273 reference. Static members additionally reference ListViewNoFlicker, ThreadInvokes, WriteInfoLog, LastLogLine and ResizeLVCols; these later paths were not executed |
| Repeater / `ACT_Plugin.TimerRepeat`            | Pass                                    | InitPlugin fails                                                                                                                                        | Missing `ActGlobals.oFormSpellTimers`; source needs expiry events, TimerFrame/TimerData, NotifySpell, frame enumeration and active timer cancellation                                               |
| Export / `ACT_Plugin1.Export_All`              | Type selection fails before constructor | Not reached                                                                                                                                             | Distributed source declares a nonpublic plugin class, rejected by current public-type eligibility; its later workflow needs GetTextExport and SendToClipboard                                       |

Each failure is a first observed boundary, not a complete list of missing APIs.
The source-derived dependency notes do not mean those paths executed. No plugin
source was patched to bypass failures. The following loader work supersedes the
initial binding/type failures without changing the downloaded plugins.

## Constructor-order correction

The official sample computes its settings filename in an instance field
initializer using `ActGlobals.oFormActMain.AppDataFolder`. The previous host
created ACT globals only inside InitPlugin, so constructor-only loading failed
with a null reference. EQ2 uses the same pattern.

[PluginSession.Construct](../managed/Bridge/PluginSession.cs) now prepares the
host before invoking plugin constructors on the owner context. The native entry
point and original FFXIV runner use that path. Windows therefore requires STA
from plugin construction onward. InitPlugin remains a separate operation;
uninitialized plugins still do not receive DeInitPlugin.

| Check                                        | Observation                                                                                                                                         |
| -------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Official sample before correction            | Public Rust `initialize` example fails during construction with a null reference                                                                    |
| Regression before correction                 | GoodPlugin constructor now requires ACT globals/Config; managed plugin requests fail                                                                |
| Official sample after correction             | Managed external-plugin runner passes; settings XML preserves `aftercast settings round trip 20261021`                                              |
| Regression after correction                  | `mise run check` passes, including constructor failure recovery, stop/restart, managed/WinForms contracts, Rust tests, formatting and strict Clippy |
| Existing FFXIV 3.0.3.4 path after correction | Original synthetic import passes three iterations (7 records, 400 damage each), zero worker/private-message errors and successful shutdown          |
| Reusable external-plugin task                | Official sample lifecycle passes with a state path containing spaces                                                                                |

## Loader compatibility follow-up

ACT's supplied executable configuration redirects versions
`2.0.0.0–3.65535.65535.65535` to `3.8.5.288`. Static inspection of
`PluginListingPanel.cbEnabled_CheckedChanged` shows byte-array assembly loading,
interface enumeration and `Assembly.CreateInstance(string)`, without a public
type requirement. REA evidence:
`ev_e307ce2aae5552323c421e9d558757f5abe620d34d83e01fa081f2390d39ff76`.

An independent .NET Framework process repeated those loading/creation operations
against genuine ACT with its original binding configuration. Both Benchmark and
the nonpublic Export_All class construct and implement genuine IActPluginV1;
Export_All has a public default constructor. This checks the creation operations,
not the original plugin-management UI or InitPlugin. The original ACT files were
unchanged; only the test copy's Internet zone marker was removed for Framework
LoadFrom. Probe source/configuration are retained under
`target/additional-plugins-20261021/original-loader/`.

| Change / check             | Result                                                                                                                                                                        |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Version redirect           | The loader accepts ACT's documented 2.x/3.x range while retaining exact simple-name, culture and public-key-token checks                                                      |
| Nonpublic plugin type      | Internal classes with public parameterless constructors are eligible; abstract/open-generic/missing-public-constructor types remain rejected                                  |
| Boundary regression checks | Old/new version bounds, missing version, wrong name, culture and signing token are checked; Rust integration constructs an internal fixture and rejects a private constructor |
| Export_All                 | Constructor, InitPlugin, pumping, DeInitPlugin and shutdown pass with zero host diagnostics; combat-end export is still unimplemented                                         |
| Benchmark                  | ACT 3.5 binding succeeds; construction now reaches missing `Advanced_Combat_Tracker.ListViewNoFlicker`                                                                        |
| Review and validation      | Connected loader/caller/fixture paths reviewed; `mise run check` passes after the change                                                                                      |

Runtime logs are `export-binding-fixed.txt` and `benchmark-binding-fixed.txt` in
the local evidence directory. The version/type guards now follow the pinned
ACT behavior; this does not establish compatibility with every member used by
older plugins.

## Text export workflow follow-up

Unmodified Export_All now completes a live combat-end workflow on Windows:
two fixture combats export 100 and then 400 cumulative damage from the zone's
All encounter, rather than only the latest combat. Host diagnostics remain zero,
and DeInitPlugin/shutdown complete. The native clipboard's previous contents are
restored in a finally block without printing them. These fixture formatter
callbacks test the export contract; they are not FFXIV/EQ2 number formatters.

| Contract                                  | Evidence / limit                                                                                                                                                                                   |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| TextExportFormatOptions and registrations | Original hash-based duplicate equality, including an intentional concatenation collision, is preserved                                                                                             |
| Unaligned GetTextExport                   | Configured sort callbacks, reverse order, explicit/empty allies, Unknown exclusion, token extras, unknown tokens, final empty LF and 256-line cap match the same compiled original-reference probe |
| Callback failures                         | Replacement checks verify observable diagnostics and empty failure output; format validation precedes best-effort encounter copying                                                                |
| Clipboard                                 | BeforeClipboardSet edits affect native output; local copy works with CopyLocal=false when no remote sharer is connected; three fallback attempts retain failure diagnostics                        |
| Full regression                           | `mise run check` and `mise run test:text-exports` pass                                                                                                                                             |
| Remaining export scope                    | Font-aligned overloads, UI configuration/selectors, selective-export options, remote Clipboard Sharer and HTML export remain unimplemented; Linux clipboard explicitly throws                      |

The original probe bootstraps only disabled export options on an uninitialized
main object, with native Windows controls; it does not run ACT's main window.
`TextExportProbe.cs` compiles against genuine ACT and runs in separate original
and replacement processes. Decompilation inputs are the pinned ACT binary;
local readable references are under `target/additional-plugins-20261021/decompiled/`.

```powershell
mise run test:text-exports
mise run test:external-export -- 'binaries/additional-plugins-20261021/Export_All.dll' 'target/additional-plugins-20261021/export-workflow'
```

The original text-export comparison uses Desktop Runtime on Windows. On other
platforms the task runs the replacement probe only. The external-export task
requires Windows and a writable native clipboard. Runtime evidence is retained
in `text-exports-reference.txt`, `export-workflow.txt` and `check-exports.txt`.

## Timer workflow follow-up

Unmodified Timer Repeater now passes lifecycle, repeat-on-expiry, combat-only
repetition and cancellation through its own native Button. Its own one-second
WinForms Timer creates that button while the host pumps Windows messages.
Expiry is driven by the host's timer state processing during pump; no plugin
source or private handler is modified/invoked by the test.

| Contract                                       | Evidence / limit                                                                                                                                                                                                         |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| TimerData, TimerMod, SpellTimer and TimerFrame | A shared genuine-ACT reference probe checks defaults, case/key equality, duplicate-mod handling, truncation, master selection, empty sentinels and owner/target removal windows                                          |
| Timer state                                    | Registration/lookups, category/self restrictions, master/nonmaster ticks, two-second duplicate suppression, warning/expiry/removal and recast mods are implemented                                                       |
| Combat input                                   | Edited accepted actions notify before aggregation; cancelled/imported actions do not start live timers                                                                                                                   |
| Estimated time                                 | LastKnownTime plus monotonic elapsed time; MinValue setter is ignored; import restores the live monotonic baseline rather than restarting elapsed time                                                                   |
| Ownership                                      | Host owns/disposes its timer form, clears the global on teardown and processes events on the caller's pumped context                                                                                                     |
| Review and checks                              | `mise run check`, `mise run test:timer-models` and `mise run test:external-timer` pass; callback invocation occurs outside the timer state gate                                                                          |
| Remaining scope                                | Sound-enabled definitions explicitly reject; timer editor/rendering, localization and additional TimerData constructor overloads remain unsupported. Precise original UI scheduling/diagnostic parity is not established |

The native workflow advances the host log clock to expire known definitions;
it also pumps real time for the plugin's native UI timer. The model reference
uses a stopped original estimator and a monotonic replacement baseline, with
integer-second cases away from rounding boundaries. It compares values without
starting the original timer window. Original timer behavior was read from the
pinned ACT binary with ilspycmd 11.1.0.9782, retained locally under `decompiled/`.

```powershell
mise run test:timer-models
mise run test:external-timer -- 'binaries/additional-plugins-20261021/TimerRepeatPlugin.dll' 'target/additional-plugins-20261021/timer-workflow-final'
```

The original model comparison uses Desktop Runtime on Windows; elsewhere the
task runs the replacement probe only. Native cancellation workflow verification
requires Windows. Evidence files: `timer-reference.txt`, `timer-workflow-final.txt`
and `check-timers-final.txt` in the local evidence directory.

## EQ2 parser follow-up

The unmodified English EQ2 parser now initializes, registers its options and log
colors, imports four controlled raw records and cleans up its options on exit.

| Check                           | Result / scope                                                                                                                                 |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Options tree and log colors     | Native Windows controls and a headless tree model retain parser registrations; encounter-log rendering remains unsupported                     |
| Default EQ2 timestamp parser    | Shared original/replacement probe checks displayed dates, invalid values and unspecified DateTime kind; the Unix prefix is not the time source |
| Controlled raw import           | Zone change, two hits and one heal produce 400 damage, 50 healing and two actors with the displayed 2026 timestamp                             |
| Original aggregation comparison | The three actions captured from that parser pass the same recorded-action probe in replacement and pinned original ACT                         |
| Selection API                   | Add/remove/check registration is supported; selective combat filtering remains unsupported                                                     |
| Review                          | `mise run check`, `test:eq2-timestamps`, `test:external-eq2` and `test:recorded-actions` pass                                                  |

```powershell
mise run test:eq2-timestamps
mise run test:external-eq2 -- 'binaries/additional-plugins-20261021/ACT_English_Parser.dll' 'target/additional-plugins-20261021/eq2-workflow-final'
mise run test:recorded-actions -- 'target/additional-plugins-20261021/eq2-workflow-final/eq2-actions.json'
```

This checks a controlled parser path, not actual EQ2 gameplay or every regex,
audio, options UI and selective parsing mode. Evidence: `check-eq2.txt`,
`eq2-timestamp-reference.txt`, `eq2-workflow-final.txt` and `eq2-replay.txt`.

## Benchmark workflow follow-up

The distributed DLL now initializes and runs without modification. Native
checkbox changes run its actual event wrapping/restoration handlers; the test
dismisses only that test thread's known ENABLED/DISABLED confirmation dialogs.
The plugin's own native Timer refreshes its table. A deliberately slow combat
handler exercises worker-to-owner RichTextBox invocation and WriteInfoLog.

| Contract                                    | Validation / limit                                                                                                                                      |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ListViewNoFlicker                           | Native double buffering, extended styles and nested update flushing; native drawing substitutes ACT's themed owner drawing                              |
| ResizeLVCols                                | Original character-weighted proportions; shared original/replacement probe covers empty and populated two-column tables                                 |
| ThreadInvokes.RichTextBoxAppendDateTimeLine | Original Form/RichTextBox signature, timestamp/newline and owner-thread dispatch; no other ThreadInvokes APIs are claimed                               |
| LastLogLine                                 | Updated after BeforeLogLineRead edits and before OnLogLineRead in live delivery and imports                                                             |
| Event field reflection                      | The original DLL locates the four private event backing fields, calls each handler once and restores them when disabled                                 |
| Cleanup                                     | Disable instrumentation before unloading, matching the plugin's own restore path; its DeInitPlugin removes wrappers without restoring captured handlers |

```powershell
mise run test:benchmark-ui
mise run test:external-benchmark -- 'binaries/additional-plugins-20261021/EventHandlerBenchmark/EventHandlerBenchmark.dll' 'target/additional-plugins-20261021/benchmark-workflow-final'
```

Both tasks pass on Windows. The benchmark fixture supplies combat category
registrations because Benchmark is an instrumenter, not a parser. Evidence:
`benchmark-ui-reference.txt` and `benchmark-workflow-final.txt`.
Native UI support does not establish headless Linux Benchmark compatibility.

The final `mise run check` passes with `DOTNET_ROOT` configured, including the
live LastLogLine ordering regression. Original FFXIV 3.0.3.4 synthetic import
still passes three iterations (seven records and 400 damage each), with zero
worker diagnostics and completed shutdown. Final logs: `check-benchmark.txt`
and `ffxiv-final.txt`.

## Reproduction

Use a trusted plugin and a private writable test directory. The runner executes
arbitrary plugin code; it checks lifecycle/host diagnostics, not full plugin
readiness, private diagnostics, visible rendering or game parsing. Invoke each
plugin in a separate process. ACT source-plugin compilation/loading is not yet
part of the public host API.

For source downloads, build with original ACT references first. For example, from
the repository root on Windows:

```powershell
$compiler = "$env:WINDIR\Microsoft.NET\Framework64\v4.0.30319\csc.exe"
$act = (Resolve-Path 'binaries/act/Advanced Combat Tracker.exe').Path
$source = (Resolve-Path 'binaries/additional-plugins-20261021/ACT_English_Parser.cs').Path
& $compiler /nologo /target:library /out:binaries/additional-plugins-20261021/ACT_English_Parser.dll "/reference:$act" /reference:System.Windows.Forms.dll /reference:System.Drawing.dll /reference:System.Data.dll /reference:System.Xml.dll $source

mise run test:external-plugin -- 'binaries/additional-plugins-20261021/ACT_English_Parser.dll' 'ACT_Plugin.ACT_English_Parser' 'target/additional-plugins-20261021/eq2-state'
```

Substitute the DLL and exact type from the results table to probe other plugins.
The sample's source is under `PluginSample/ACT_Plugin_Sample/PluginSample.cs` after
extracting its ZIP. To repeat its settings check, seed
`<state>/Config/PluginSample.config.xml` before running:

```xml
<Config><SettingsSerializer><TextBox Name="textBox1" Value="aftercast settings round trip 20261021" /></SettingsSerializer></Config>
```

Verify that the saved XML retains the same Value after successful shutdown.
The runner prints the actual DLL SHA-256 for provenance; it does not enforce the
download hashes above. Compare downloaded files before compiling or executing.

Local runtime output is under `target/additional-plugins-20261021/*-after.txt`;
REA evidence is exported to `rea-evidence.json` there. Downloaded sources and
original binaries remain outside the shipped payload.

## Next compatibility work

| Priority | Work                                             | Acceptance condition                                                                                                      |
| -------- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------- |
| 1        | Complete the pinned public-member inventory      | Separate missing signatures from implemented-but-uncompared behavior across ACT API families                              |
| 2        | Expand parser and combat-policy coverage         | Add representative EQ2 and other game logs, selective parsing and correction behavior with original-reference comparisons |
| 3        | Complete triggers, audio and timer UI            | Compare matching/capture/order, sound delivery and visible timer lifecycle against original ACT                           |
| 4        | Complete application/history/export UI workflows | Save/restart/search/reimport encounters and validate settings, graphs and additional export formats                       |

Linux execution of these downloads, visible rendering and actual EQ2 gameplay
remain unverified. Native Windows timer and clipboard workflows above are
verified within their stated limits. Existing FFXIV gameplay/recording limits
still apply.
