# cactbot browser audio validation — 2026-10-21

The unchanged cactbot 0.37.5 release now passes real HTMLAudio and local Web Speech
checks on Windows and Linux. The harness observes native objects and their
completion/error events; it does not return synthetic playback success.

## Executed coverage

| Surface                                                 | Voice/effect path                                                                                          | Result                                                                                                                                        |
| ------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Windows Edge 155.0.4283.62, headless with audio unmuted | Browser Web Speech, local `Microsoft Heami - Korean (Korean)`, `ko-KR`                                     | Pass: shipped response trigger speaks `4` twice, with separate native start/end events                                                        |
| Ubuntu 24.04/WSLg, Chromium 151.0.7922.34 headless      | Browser Web Speech → private Speech Dispatcher → local espeak-ng `English (America)`, `en-US`              | Pass: same trigger speaks twice, both native completions observed                                                                             |
| Both browsers above                                     | Shipped alarm, info, alert and custom clap sounds via native HTMLAudio                                     | Pass: alarm repeats twice (time/duration 1.003 seconds); info/alert/custom assets also finish, with the custom trigger's volume 0.3 preserved |
| Maintained OverlayPlugin original Windows CEF 95.7.14   | Actual FFXIV log ingress → original cactbot trigger → file-based HTMLAudio                                 | Pass: original alarm completes; CEF/WS handlers, DOM alerts/timeline, screenshot and teardown checks still pass                               |
| Audio priority                                          | `forceTTS=1` remote raidboss                                                                               | Pass: speech plays instead of an overlapping effect; neither `cactbotSay` nor `playSound` is used by the browser audio scenarios              |
| Autoplay                                                | Normal browser restrictions, no autoplay-policy bypass                                                     | Pass: cactbot's visible enable-audio button is clicked by a real browser input action before triggers                                         |
| Offline page behavior                                   | Loopback-served release assets; external page requests aborted; selected voice reports `localService=true` | Pass: no unexpected external requests or playback errors                                                                                      |
| Failure control                                         | Request English on the Windows machine with only a Korean local voice                                      | Expected failure with an explicit missing matching voice diagnostic, not a successful silent test                                             |

The standalone browser scenarios use a controlled OverlayPlugin API/player/log
fixture. Media and speech APIs remain native. The CEF scenario uses the genuine
maintained plugin and live-log route. Native CEF is not available on Linux in this
host; Linux coverage is the remote browser route supported by cactbot.
Native completion events are evidence of the tested playback paths; this is not
microphone capture, a listening-quality comparison, or real-raid validation.

## Problems found and corrected

| Finding                                                                                                                     | Correction                                                                                                                                                                                      |
| --------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Previous browser harness used `audio=0` and mute flags; Playwright also mutes headless Chromium by default                  | Separate audio runner enables `audio=1`, removes default `--mute-audio`, and requires native start/end events                                                                                   |
| Windows had only a Korean local voice while the fixture requested English; cactbot requires an exact display-language voice | Discover installed local voices and configure the fixture's display language to match. Explicit incompatible `AFTERCAST_AUDIO_LANG` fails before firing a trigger; no remote fallback is chosen |
| Linux Chromium needed its native speech service enabled and installed browser libraries                                     | Enable `--enable-speech-dispatcher`, use a private dispatcher, install standard Playwright runtime dependencies and a local voice module                                                        |
| cactbot caches utterance objects; old observer listeners could overwrite a completed request during a replay/error          | Remove listeners on completion/error/throw; unit test replays the same object and verifies that an old successful record stays unchanged                                                        |
| Native speech throws and media Promise rejections could be mistaken for queue admission                                     | Preserve the original error/Promise and fail the runner on native error, rejected playback, missing completion, or unexpected ACT routing                                                       |
| Media completion alone does not prove unmuted playback                                                                      | Require positive volume and HTMLAudio `muted=false`; explicitly disable CEF `MuteWhenHidden` and call the original renderer's `SetMuted(false)`                                                 |
| HTTP fixture lacked WebM MIME handling                                                                                      | Shared fixture server serves `audio/webm`; existing visual and actual WS/CEF regression checks pass                                                                                             |

No cactbot release files or installed browser voice registries were modified.
The missing voice was an environment/fixture mismatch; this change does not add a
language fallback to cactbot's own BrowserTTSEngine. Users must select a display
language with a matching installed browser voice. Embedded local raidboss normally
uses ACT TTS; browser-owned speech is tested with the supported `remote=1` mode.

## Reproduce

Use an existing Node.js and Playwright installation. On Windows install Edge and a
local Windows voice. On Linux install Chromium through Playwright, Speech
Dispatcher, a local voice module such as espeak-ng, and browser runtime libraries.
The runner uses the platform's default audio output; WSLg here used its PulseAudio
socket and the private ALSA `type pulse` configuration already used by native audio tests.

```sh
node --test managed/tests/cactbot-audio.test.cjs
# NODE_PATH must resolve playwright if it is outside this checkout.
mise run test:browser-audio -- <extracted-cactbot-root> <report-directory>
```

| Environment setting                | Purpose                                                                     |
| ---------------------------------- | --------------------------------------------------------------------------- |
| `NODE_PATH`                        | Existing Playwright installation                                            |
| `PLAYWRIGHT_BROWSERS_PATH`         | Existing Linux Chromium cache, if not its default location                  |
| `NODE_BINARY`                      | Linux Node path, if not available as `node`; only used by the Linux wrapper |
| `AFTERCAST_AUDIO_LANG`             | Optional explicit cactbot language; must match a local browser voice        |
| `ALSA_CONFIG_PATH`, `PULSE_SERVER` | Optional Linux/WSLg output routing                                          |

The Linux wrapper runs observer unit checks, owns a temporary Speech Dispatcher
socket/process and stops only that process on exit. It does not change the user's
global speech/audio configuration. Test runtimes and browsers were downloaded
under ignored `target/browser-linux`; no new production Rust/.NET dependencies were added.

## Evidence

| File                                                                    | Observation                                                                        |
| ----------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| `target/browser-audio/windows/audio-report.json`                        | Windows local voice, five effect and two speech completions, no ACT routing/errors |
| `target/browser-audio/linux-chromium/audio-report.json`                 | Linux local voice and both repeated playback scenarios                             |
| `target/overlay-cactbot-20261021/stack-cef/cef-audio-report.json`       | Genuine CEF file audio playing/end/time/duration                                   |
| `target/browser-audio-cef.txt`                                          | Full original stack regression pass                                                |
| `target/browser-audio-missing-voice.txt`                                | Expected incompatible-language failure                                             |
| `target/browser-audio-unit.txt`                                         | Two observer regression tests, including preserved native failures                 |
| `target/browser-audio-legacy.txt`, `target/browser-audio-contracts.txt` | Existing visual and managed-contract regressions                                   |

The shipped raidboss bundle SHA-256 is
`97894e6cf0d959f7b550dfed4cfd08c9b50154b4e6c54f4f5c30a311bcdf3633`.
The server reads this file unchanged. Release assets, not the separate development
checkout, are the authority for these runtime results.
