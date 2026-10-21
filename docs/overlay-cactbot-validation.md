# OverlayPlugin and cactbot validation — 2026-10-21

The [audio follow-up](audio-validation.md) also verifies original CEF and WS
`playSound` requests alongside `cactbotSay`. Native ACT timer speech/effects are
checked independently through complete Windows and Linux host/device paths.
The [browser audio follow-up](browser-audio-validation.md) also passes real CEF
HTMLAudio and remote Edge/Linux Chromium HTMLAudio/local Web Speech playback.

The controlled Windows x64/.NET 10 stack passes original FFXIV initialization,
maintained OverlayPlugin phase 2, cactbot addon startup, real CEF rendering and
real WebSocket delivery into unchanged raidboss bundles. Game memory is replaced
by an explicit public player-state fixture; log ingress uses the real FFXIV
plugin. This establishes the tested integration path, not gameplay coverage.

Packages came from the [official ACT download list](https://advancedcombattracker.com/download.php):
**[OverlayPlugin/OverlayPlugin 0.19.111](https://github.com/OverlayPlugin/OverlayPlugin/releases/tag/v0.19.111)**
and **[OverlayPlugin/cactbot 0.37.5](https://github.com/OverlayPlugin/cactbot/releases/tag/v0.37.5)**.
Downloaded files and JavaScript bundles remain unchanged on disk. The host applies
one hash-pinned **in-memory HtmlRenderer import correction**, described below.

## Results

| Layer | Asserted result | Boundary |
| --- | --- | --- |
| FFXIV 3.0.3.4 | Starts before OverlayPlugin; follows five controlled raw records | No game process or packet capture |
| OverlayPlugin 0.19.111 | Deferred initialization reaches `Initialised`; no unexpected integration errors | Native Windows x64, prepared CefSharp 95.7.14 |
| cactbot 0.37.5 | Actual event source registers, starts its ACT log subscription and serves handlers | Player identity/HP/job supplied through its public event API |
| Original CEF overlay | Unchanged raidboss page renders alerts and timeline; real bitmap frame captured | Desktop composition/transparency and audio not assessed |
| Original WebSocket server + Edge | Actual handlers, player event and five LogLine events reach unchanged raidboss | Loopback transport, controlled logs |
| Shutdown | Event-source Stop, plugin DeInit and IDisposable run; cactbot log hook removed and ACT globals cleared | CEF/process restart and long-running reconnect remain untested |

The observer alone is not a readiness test: plugins can catch errors and return.
The integration test additionally asserts final status, source registration,
actual handler responses, CEF/Edge DOM output, frame capture, log errors and
cleanup. The original cactbot emits `Requires .NET 4.6 or above. Using 10.` because
its check independently compares major and minor (`major < 4 || minor < 6`). It
logs this after startup without aborting. Only that known false version error is
allowed; other `Error:` log entries fail integration.

## Pinned inputs

Inputs are excluded from Git under `binaries/overlay-cactbot-20261021/`.

| Input | Bytes | SHA-256 |
| --- | ---: | --- |
| OverlayPlugin ZIP, [ACT ID 86](https://advancedcombattracker.com/download.php?id=86) | 1,521,991 | `84ec07042f793752c3601cdd79f7d6cf711012222afdd956d7c723ac0347a00c` |
| cactbot ZIP, [ACT ID 97](https://advancedcombattracker.com/download.php?id=97) | 10,874,048 | `1a9a277982259e370304ebfda3e81434fbbc039d0ddf5ec299fd776a5d3477f9` |
| OverlayPlugin.dll, assembly 0.19.111.0 | 18,944 | `45f56980210bf94c3c9c85605867131f102bcd091787223ee1c17e9adb9ca725` |
| CactbotOverlay.dll, assembly 0.37.5.0 | 10,752 | `c972270c0699e3a51c3966a2df96a5d06c8b4f97e29c9272b10e54a4257babe2` |
| HtmlRenderer.dll | — | `7d5a9b39940e8a4470398707801fbf4f7d643e082e06dd9e244fbc80549d742f` |
| CefSharp 95.7.14 x64 archive | 71,848,018 | `1146c6964b95af1f2b63f6b9a58bee3cc59dcb4f7d3c0bc7635c54952eb27211` |

Both plugin DLLs target x64/.NET Framework 4.8. OverlayPlugin references ACT
3.8.0.283; cactbot references ACT 3.8.4.287 and OverlayPlugin.Common 0.19.86.0.
Independent contract probes use original ACT 3.8.5.288. REA artifact evidence:
`ev_76184c49d257592ab207cf4a3965a98cdff3058856d845cffecd743bae606ed3`,
`ev_216a8c77542954b77456791c2e4bd0afa6df49b0fbdf84354c478c160a08efb2`.
Relevant original types were decompiled locally with ilspycmd 11.1.0.9782.

The maintained updater still specifies this legacy **CEF dependency** URL:
`https://github.com/ngld/OverlayPlugin/releases/download/v0.7.0/CefSharp-95.7.14-x64.7z`.
The tested plugin itself comes from OverlayPlugin/OverlayPlugin. The complete
archive is extracted into private state `OverlayPluginCef/x64`, with version.txt
set to `95.7.14`; no system CEF installation is changed.

## Host corrections

| Contract / failure | Implementation and evidence |
| --- | --- |
| InitActDone | False during construction/initialization; Pump marks a completed startup batch ready only when no pending plugin remains; adding another plugin resets it; shutdown clears it. Lifecycle fixtures cover these transitions. Initialize the whole stack before its first Pump. |
| Plugin metadata | Host owns native parent tabs, status/title labels and enabled checkboxes separately from mutable ActPlugins entries. cbEnabled is true during Init and false before DeInit. Independent original probe checks metadata/readiness signatures. |
| PluginGetRemoteVersion(int) | Queries the actual HTTPS ACT endpoint with bounded timeout and legacy WebException errors. Original and replacement both return `v0.19.111` for ID 86; no fabricated version response. |
| FormImportProgress | Exact nullable ActGlobals field/type required by MiniParse. It remains null because embedded imports have no progress window; a full import-progress UI is not implemented. |
| Addon type identity | Share OverlayPlugin.Common and Newtonsoft.Json between load contexts, checking name, version, culture and token. Required for cactbot's Func<JObject,JToken> handler signatures. Binding tests include incompatible identities and simple-name loads. |
| Native CopyMemory | Hash-pinned import correction enables real CEF pixel copying under .NET 10; buffer bounds/zero-length/source-preservation checks plus real capture pass. |
| Embedded shutdown | Stop registered OverlayPlugin event sources before DeInit/container disposal, then dispose plugin resources once. Generic success/failure fixtures and real cactbot subscription removal are checked. |

The original HtmlRenderer imports `kernel32!CopyMemory`, which resolves under
CLR 4 (confirmed independently) but throws EntryPointNotFoundException under
.NET 10 during rendering. For the exact hash above on Windows x64, the loader
validates its signature and sole module use, clones its bytes, and changes the
import to `ucrtbase!memcpy` in the cloned metadata string heap. Internal calls use
the existing MethodDef token. Windows x64's unified calling convention permits
the ignored pointer return and zero-extended uint length. Unknown hashes are
loaded without rewriting; they are **not** claimed renderer-compatible. This
correction neither patches a user's installation nor changes cactbot JavaScript.

## End-to-end assertions

[The managed check](../managed/tests/ManagedTests/OverlayIntegrationChecks.cs)
starts the original WSServer and a real MiniParseOverlay. CEF DOM inspection goes
through its real ExecuteScript binding and original cactbotSaveData handler.
[The browser check](../managed/tests/cactbot-browser.cjs) connects Edge
**155.0.4283.62** to that original WebSocket server without an injected API in
integration mode. Its standalone mode remains a separate fixture-only check.

| Input / operation | Asserted CEF and Edge result |
| --- | --- |
| Public player-state event; raw zone 134 log | Built-in CactbotTest data selected |
| Raw GameLog echo `cactbot test response` | Alarm `1`, alert `2`, info `3` |
| Raw bow emote to striking dummy | TimerBars include Angry Dummy; actual CEF bitmap captured |
| Raw echo `test sync2` | Forcejump produces bar Three |
| Raw farewell emote | Timeline bars cleared |
| cactbotSaveData / cactbotLoadData | Original server handler round trip returns 42 |
| WebSocket frames / page errors | Actual version response, player event and at least five LogLine events; no uncaught page exceptions |

Raw records are appended to a private file followed through the original FFXIV
plugin and ACT log events. The player fixture uses public Cactbot entity/event
types and DispatchToJS; no private callback or fake game process is invoked.
Both rendered screenshots were inspected. Audio is explicitly disabled.

A [TTS follow-up](tts-validation.md) enables the bounded ACT speech route and
calls the unmodified `cactbotSay` handler from both real CEF and WS. The expected
two texts reach the managed TTS queue (`stack-cef/tts-route.json`). Rust delivery
and real backend playback are validated separately; this browser harness still
disables its built-in alert audio.

## Reproduction and evidence

Prepare trusted extracted plugins, the FFXIV copy's chart dependency and CEF:

```powershell
$state = 'target/overlay-cactbot-20261021/stack-cef'
7z x 'binaries/overlay-cactbot-20261021/CefSharp-95.7.14-x64.7z' "-o$state/OverlayPluginCef/x64" -y
Set-Content "$state/OverlayPluginCef/x64/version.txt" '95.7.14' -NoNewline
# Use an existing Playwright install and Microsoft Edge; no product npm dependency is added.
$env:NODE_PATH = '<existing node_modules containing playwright>'
mise run test:overlay-integration -- 'target/windows-original-3034/plugin/FFXIV_ACT_Plugin.dll' 'binaries/overlay-cactbot-20261021/OverlayPlugin/OverlayPlugin/OverlayPlugin.dll' 'binaries/overlay-cactbot-20261021/cactbot/cactbot/cactbot/CactbotOverlay.dll' $state
mise run test:overlay-renderer -- 'binaries/overlay-cactbot-20261021/OverlayPlugin/OverlayPlugin/libs/HtmlRenderer.dll'
mise run test:plugin-metadata
mise run test:plugin-updates
mise run check
```

| Local evidence relative to target/overlay-cactbot-20261021/ | Contents |
| --- | --- |
| integration-cef.txt, integration-repeat.txt | Complete original CEF + WS/Edge assertions; repeated successfully after the implementation commit with reused private state |
| stack-cef/stack-observation.json, stack-cef/overlay-integration.log | Original final status, plugin errors and shutdown results |
| stack-cef/cef-report.json, stack-cef/cef-raidboss.png | Five real CEF DOM probes and captured bitmap |
| stack-cef/browser/browser-report.json, stack-cef/browser/raidboss.png | Real WS frames/handlers and rendered Edge output |
| stack-cef/teardown-report.json | Cleared host globals and removed cactbot log subscription |
| renderer-check.txt, framework-copy.txt | Native memcpy boundary checks and independent CLR 4 original-import check |
| metadata-reference.txt, update-reference.txt | Independent original ACT contract/version query comparisons |
| check-integrated.txt, ffxiv-integrated.txt | Full regression suite and original FFXIV synthetic import regression |
| cef-download.json, cef-extract.txt | CEF archive provenance and extraction |

Earlier phase-1/missing-member observations are historical and superseded by
these integration assertions. Live game memory/network capture, actual raids,
other cactbot overlays, acoustic quality, desktop composition and sustained operation
remain outside this controlled verification.
