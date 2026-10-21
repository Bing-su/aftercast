# OverlayPlugin and cactbot validation — 2026-10-21

Original packages came from the [official ACT download list](https://advancedcombattracker.com/download.php).
The tested release is **[OverlayPlugin/OverlayPlugin 0.19.111](https://github.com/OverlayPlugin/OverlayPlugin/releases/tag/v0.19.111)**,
the maintained fork, together with **[OverlayPlugin/cactbot 0.37.5](https://github.com/OverlayPlugin/cactbot/releases/tag/v0.37.5)**.
No downloaded DLL, JavaScript bundle or source was modified. Windows x64/.NET 10
and the existing original FFXIV 3.0.3.4 copy were used for native startup.

## Results

| Layer | Observed result | Interpretation |
| --- | --- | --- |
| Original FFXIV 3.0.3.4 | Initializes before OverlayPlugin | Startup check; gameplay capture is not tested here |
| OverlayPlugin | Reaches `Init Phase 1: Waiting for plugins to load` with CEF files present | Blocked: its native timer requires missing `FormActMain.InitActDone`; the updater also encounters missing `PluginGetRemoteVersion(int)` |
| CactbotOverlay | InitPlugin returns `Ready.` and removes its native settings tab | Entry point only; the subsequent shared pump fails in OverlayPlugin's timer |
| cactbot addon/event source | Not verified | OverlayPlugin phase 2 has not completed; addon Init, registration and actual game events are not established |
| raidboss web release | Alarm/alert/info outputs, timeline start, forced jump and reset pass in real Edge | Fixture OverlayPlugin API; independent browser verification, not an aftercast/CEF integration pass |

Zero host diagnostics is insufficient evidence of readiness. OverlayPlugin can
catch an error, display its own dialog and return normally. The observer records
status, failure phase, selected first-chance exceptions, dialogs and shutdown
failures. Its successful exit means the observation completed. Pump failures can
originate from an earlier plugin while a later plugin is being observed.

## Pinned inputs

Inputs remain excluded from Git under `binaries/overlay-cactbot-20261021/`.

| Input | Bytes | SHA-256 |
| --- | ---: | --- |
| OverlayPlugin ZIP, [ACT ID 86](https://advancedcombattracker.com/download.php?id=86) | 1,521,991 | `84ec07042f793752c3601cdd79f7d6cf711012222afdd956d7c723ac0347a00c` |
| cactbot ZIP, [ACT ID 97](https://advancedcombattracker.com/download.php?id=97) | 10,874,048 | `1a9a277982259e370304ebfda3e81434fbbc039d0ddf5ec299fd776a5d3477f9` |
| OverlayPlugin.dll, assembly 0.19.111.0 | 18,944 | `45f56980210bf94c3c9c85605867131f102bcd091787223ee1c17e9adb9ca725` |
| CactbotOverlay.dll, assembly 0.37.5.0 | 10,752 | `c972270c0699e3a51c3966a2df96a5d06c8b4f97e29c9272b10e54a4257babe2` |
| CefSharp 95.7.14 x64 archive | 71,848,018 | `1146c6964b95af1f2b63f6b9a58bee3cc59dcb4f7d3c0bc7635c54952eb27211` |

Both plugin DLLs target x64/.NET Framework 4.8. OverlayPlugin references ACT
3.8.0.283; cactbot references ACT 3.8.4.287 and OverlayPlugin.Common 0.19.86.0.
REA artifact evidence:
`ev_76184c49d257592ab207cf4a3965a98cdff3058856d845cffecd743bae606ed3`,
`ev_216a8c77542954b77456791c2e4bd0afa6df49b0fbdf84354c478c160a08efb2`.
Relevant types were decompiled with maintainer-only ilspycmd 11.1.0.9782 into the
local evidence directory's `decompiled/` folder.

The **current** updater still specifies this legacy CEF dependency URL:
`https://github.com/ngld/OverlayPlugin/releases/download/v0.7.0/CefSharp-95.7.14-x64.7z`.
That URL is distinct from the maintained OverlayPlugin release tested here.
The complete archive was extracted into private test state `OverlayPluginCef/x64`,
with `version.txt` set to `95.7.14`; no system CEF installation was changed.
Reaching phase 1 does not establish renderer initialization or complete CefSharp
compatibility with .NET 10. An earlier bounded download observation without
complete CEF remained at CEF initialization and is not the final result.

## Host corrections and checks

| Initial blocker | Correction / check | Limit |
| --- | --- | --- |
| Missing ActPlugins | Writable list tracks registered plugin data; lifecycle/removal tests added | Ownership remains separate from the public mutable list |
| Version/closing dependencies | GetVersion reports the compatibility assembly version; IsActClosing becomes true before shutdown callbacks | Original version scanning/logging side effects are not fully replicated |
| Common fails despite extracted libs | Private libs DLLs resolve in the plugin load context; fixture checks path/context | Shared identity across every addon/dependency is not established |
| Parent TabControl absent | Windows plugin pages now belong to a native host-owned TabControl | Linux parent-tab UI is outside this change |
| NotificationAdd binding | Four-argument signature explicitly rejects unsupported notification delivery | No notification UI is claimed |
| Original contract evidence | Shared PluginMetadataProbe checks list assignment, version, initial closing state and notification signature against original ACT 3.8.5.288 | Replacement fixtures separately check shutdown transitions and native parents |

The observer uses private state, dismisses and records native message boxes
owned only by its own test thread, and surfaces native Timer exceptions instead
of leaving WinForms' unhandled-exception dialog open. First-chance exceptions can
be caught by a plugin and are not individually classified as fatal results.

## Browser verification

The [browser check](../managed/tests/cactbot-browser.cjs) serves the extracted
release on loopback and loads its unchanged raidboss page in headless Microsoft
Edge **155.0.4283.62**. Only the OverlayPlugin API is a fixture; unexpected API
calls and uncaught page exceptions fail the test. Audio is disabled.

| Input | Asserted result |
| --- | --- |
| Player state and Middle La Noscea, zone 134 | Built-in CactbotTest data selected |
| GameLog echo `cactbot test response` | Alarm `1`, alert `2`, info `3` |
| Bow emote to the striking dummy | Rendered TimerBars include Angry Dummy, Long Castbar and Final Sting |
| Echo `test sync2` | Forced jump produces loop bars Two, Three and Four |
| Farewell emote | Timeline bars cleared |
| Page exceptions | None |

The captured screenshot was inspected. This validates browser rendering of the
fixture; CEF/desktop transparency, game input, event-source delivery, audio and
actual raid gameplay remain unverified.

## Reproduction

```powershell
mise run test:plugin-metadata
mise run observe:overlay-stack -- 'target/windows-original-3034/plugin/FFXIV_ACT_Plugin.dll' 'binaries/overlay-cactbot-20261021/OverlayPlugin/OverlayPlugin/OverlayPlugin.dll' 'binaries/overlay-cactbot-20261021/cactbot/cactbot/cactbot/CactbotOverlay.dll' 'target/overlay-cactbot-20261021/stack-cef'

# Use existing Playwright and Microsoft Edge; no npm dependency is added to the product.
$env:NODE_PATH = '<existing node_modules containing playwright>'
node managed/tests/cactbot-browser.cjs 'binaries/overlay-cactbot-20261021/cactbot/cactbot/cactbot' 'target/overlay-cactbot-20261021/browser'
```

| Local evidence, relative to target/overlay-cactbot-20261021/ | Contents |
| --- | --- |
| overlay-initial.txt, overlay-host-contracts.txt | Initial ACT list and internally caught Common load failures |
| stack-parent.txt, stack-cef/stack-observation.json | CEF present, native parent tabs, missing API evidence and failure-phase attribution |
| cef-download.json, cef-extract.txt | Complete CEF archive provenance and extraction |
| browser.txt, browser/browser-report.json, browser/raidboss.png | Assertions, API requests and rendered UI |
| check.txt, metadata-reference.txt | Regression suite and independent original comparison |

The full `mise run check` suite and independent metadata reference comparison
pass. Original FFXIV synthetic import is also rerun after the host changes;
`ffxiv-regression.txt` records that bounded regression separately from the stack
startup observation.

Next integration work needs an explicit host-ready lifecycle boundary, remaining
ACT plugin-manager/update contracts, addon identity checks and an end-to-end
event-source/renderer test. A constant readiness flag would not establish that
lifecycle.
