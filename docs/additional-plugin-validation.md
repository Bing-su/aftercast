# Additional ACT plugin validation — 2026-10-21

Five plugins were downloaded from the official
[ACT downloads page](https://advancedcombattracker.com/download.php).
They exercise settings, another game's parser, event instrumentation, spell
timers and export workflows. All results below are Windows x64/.NET 10 results
against aftercast's ACT 3.8.5.288 identity.

## Input provenance

Downloads and extracted sources remain local under
`binaries/additional-plugins-20261021/`, excluded from Git and Cargo packages.
Sources were compiled unchanged against the supplied genuine ACT executable
using the installed .NET Framework C# compiler, not against aftercast. These
derived DLLs are local test inputs, not vendor-distributed binaries. The benchmark
DLL was used exactly as distributed.

| Plugin | Official download | Downloaded file | SHA-256 of downloaded file |
| --- | --- | --- | --- |
| ACT Plugin Sample, assembly 1.0.0.2 | [ID 32](https://advancedcombattracker.com/download.php?id=32) | `ACT_Plugin_Sample.zip` | `bfcc3573729c27862b8cbe731ab3072f3640472f1cf27d062f0f5962af0a4024` |
| EQ2 English Parsing, assembly 1.4.2.30 | [ID 46](https://advancedcombattracker.com/download.php?id=46) | `ACT_English_Parser.cs` | `5dd5c8f9342e4c495deb75d02c9976bb34fef5827d26425af024b2e821b84d07` |
| Event Handler Benchmark, assembly 1.0.3.4 | [ID 84](https://advancedcombattracker.com/download.php?id=84) | `EventHandlerBenchmark.zip` | `290c3ec879c775724a8fc75017df611b3213e3083671991f7e462c6b0b6cbd11` |
| Timer Repeater, file version 1.0.2.3 | [ID 27](https://advancedcombattracker.com/download.php?id=27) | `TimerRepeatPlugin.cs` | `6e10ca61592508296bb75efc9de32b050823b72729ce0cdd8a2aae1bb74ccc6e` |
| Export the All encounter to clipboard, assembly 1.0.0.0 | [ID 17](https://advancedcombattracker.com/download.php?id=17) | `Export_All.cs` | `6364bd9eef4641aee63455c367dcac50b02553dd87655186f90a5dad3540f46b` |

The extracted benchmark DLL has SHA-256
`f28ff92106b6ac216fdede0d4b7a66cf296a5493cf4b9de81eff0ed83f141f2d`.
REA metadata inspection records assembly version 1.0.3.4, AnyCPU CIL and a
.NET Framework 4.0 Client target. Evidence:
`ev_697d523acd0574b09950516f1397144edd2f604763937ed5331f845ec036c6c3`;
member inspection:
`ev_4faed82e214b8c67a4f2e121f7008ac4b3cfc807054b8bba8789eeded7d08178`.
Static inspection does not establish successful execution.

## Initial observed results

| Plugin / type | Constructor | Initialization / tested workflow | First blocker or limit |
| --- | --- | --- | --- |
| Sample / `ACT_Plugin.PluginSample` | Pass after correction | InitPlugin, 1.5 seconds of pumping, DeInitPlugin and shutdown pass with zero host diagnostics; a seeded TextBox setting is restored and saved unchanged | Its combat-action callback deliberately throws `NotImplementedException`; it is a settings/lifecycle sample, not a parser |
| EQ2 / `ACT_Plugin.ACT_English_Parser` | Pass after correction | InitPlugin fails | Missing `FormActMain.get_OptionsTreeView()`; source also requires OptionsControlSets, encounter-log colors, spell-timer mods, localization, text export and additional action overloads |
| Benchmark / `ACT_Plugin.EventHandlerBenchmark` | Type selection fails before constructor | Not reached | Loader rejects its ACT 3.5.0.273 reference. Static members additionally reference ListViewNoFlicker, ThreadInvokes, WriteInfoLog, LastLogLine and ResizeLVCols; these later paths were not executed |
| Repeater / `ACT_Plugin.TimerRepeat` | Pass | InitPlugin fails | Missing `ActGlobals.oFormSpellTimers`; source needs expiry events, TimerFrame/TimerData, NotifySpell, frame enumeration and active timer cancellation |
| Export / `ACT_Plugin1.Export_All` | Type selection fails before constructor | Not reached | Distributed source declares a nonpublic plugin class, rejected by current public-type eligibility; its later workflow needs GetTextExport and SendToClipboard |

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

| Check | Observation |
| --- | --- |
| Official sample before correction | Public Rust `initialize` example fails during construction with a null reference |
| Regression before correction | GoodPlugin constructor now requires ACT globals/Config; managed plugin requests fail |
| Official sample after correction | Managed external-plugin runner passes; settings XML preserves `aftercast settings round trip 20261021` |
| Regression after correction | `mise run check` passes, including constructor failure recovery, stop/restart, managed/WinForms contracts, Rust tests, formatting and strict Clippy |
| Existing FFXIV 3.0.3.4 path after correction | Original synthetic import passes three iterations (7 records, 400 damage each), zero worker/private-message errors and successful shutdown |
| Reusable external-plugin task | Official sample lifecycle passes with a state path containing spaces |

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

| Change / check | Result |
| --- | --- |
| Version redirect | The loader accepts ACT's documented 2.x/3.x range while retaining exact simple-name, culture and public-key-token checks |
| Nonpublic plugin type | Internal classes with public parameterless constructors are eligible; abstract/open-generic/missing-public-constructor types remain rejected |
| Boundary regression checks | Old/new version bounds, missing version, wrong name, culture and signing token are checked; Rust integration constructs an internal fixture and rejects a private constructor |
| Export_All | Constructor, InitPlugin, pumping, DeInitPlugin and shutdown pass with zero host diagnostics; combat-end export is still unimplemented |
| Benchmark | ACT 3.5 binding succeeds; construction now reaches missing `Advanced_Combat_Tracker.ListViewNoFlicker` |
| Review and validation | Connected loader/caller/fixture paths reviewed; `mise run check` passes after the change |

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

| Contract | Evidence / limit |
| --- | --- |
| TextExportFormatOptions and registrations | Original hash-based duplicate equality, including an intentional concatenation collision, is preserved |
| Unaligned GetTextExport | Configured sort callbacks, reverse order, explicit/empty allies, Unknown exclusion, token extras, unknown tokens, final empty LF and 256-line cap match the same compiled original-reference probe |
| Callback failures | Replacement checks verify observable diagnostics and empty failure output; format validation precedes best-effort encounter copying |
| Clipboard | BeforeClipboardSet edits affect native output; local copy works with CopyLocal=false when no remote sharer is connected; three fallback attempts retain failure diagnostics |
| Full regression | `mise run check` and `mise run test:text-exports` pass |
| Remaining export scope | Font-aligned overloads, UI configuration/selectors, selective-export options, remote Clipboard Sharer and HTML export remain unimplemented; Linux clipboard explicitly throws |

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

| Priority | Work | Acceptance condition |
| --- | --- | --- |
| 1 | Establish legacy assembly-version and nonpublic-type binding behavior against original ACT | Independent original-loader evidence before changing current guards; enumerate later missing contracts separately |
| 2 | Implement EQ2 options/log-color registration and remaining parser contracts | Unmodified parser initializes, then a controlled EQ2 log agrees with original ACT; UI-only placeholders do not count |
| 3 | Implement spell-timer state and expiry contracts | Unmodified Repeater starts, repeats and cancels timers with original timing/event behavior |
| 4 | Implement text export and clipboard workflows | Unmodified Export_All produces the original formatted All encounter output after a live combat end |

Linux execution, visible UI, actual EQ2 gameplay, timer operation and clipboard
output were not verified. Existing FFXIV gameplay/recording limits still apply.
