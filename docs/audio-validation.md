# ACT sound and timer audio validation

Validated on 2026-10-21. Effects and speech have separate bounded workers; either
can play while the other is active. No remote audio fallback is selected.

## Contracts and scope

| Entry point                            | Implemented behavior                                                                                                                                         | Boundary                                                                                                                                                |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `FormActMain.PlaySound(string)`        | Absolute paths, executable-relative paths, original nested `PlaySoundDelegate` field, sound volume and existing `sapi.wav` TTS volume selection              | Host executable directory replaces the original ACT executable directory; missing files become file URIs and fail asynchronously in the default backend |
| `PlayTtsMethod`                        | Original nested delegate/field can be replaced by plugins; default delegates enqueue speech                                                                  | ACT speech correction UI/cache/duplicate suppression remains outside this implementation                                                                |
| `Host::set_sound_player`               | Opt-in independent effect queue, completion failures as `Error::Sound` through `Host::next_audio_error().await`, cancellation on replacement/clear/stop/drop | Managed queue: 32; Rust host in-flight limit: 32; direct player: 32 waiting plus active                                                                 |
| `SoundPlayer::system` / `FilePlayback` | Local paths/file URIs, WAV/MP3/Ogg Vorbis/FLAC decoding, generated 880 Hz beep, 0..100 volume                                                                | Requires `playback`; at most 16 MiB per encoded file and 300 seconds per playback; no HTTP fetch or WMP codecs                                          |
| `SoundBackend`                         | Injectable synchronous worker-owned player with cooperative cancellation                                                                                     | Applications can add other sources/devices without changing the ACT bridge                                                                              |
| `AudioSettings`                        | Separate effect/TTS volumes and default start/warning/expiry cues                                                                                            | Defaults are full volume and `none`; settings reset at session stop                                                                                     |
| Timer cues                             | `none`, `beep`, `tts`, `tts text`, existing file; per-definition start/warning override defaults                                                             | Blank start/warning use defaults; expiry uses its global default                                                                                        |
| Timer ordering                         | Start on master tick only; warning and expiry once per frame, before event callbacks; first timer's captures for warning                                     | Existing Pump granularity and timer event ordering remain; effects use FIFO rather than original WMP overlap                                            |
| Capture expansion                      | `${attacker}`, `${victim}`, other `ExtraInfo` keys; unknown labels lose their `${}` wrapper, as in ACT                                                       | At most 32 substitutions/4096 characters; cyclic data reports a diagnostic instead of stalling the owner thread                                         |

Timer `tts ` dispatch deliberately preserves the leading space passed by the
original `Substring(3)`. Standard English `tts` cues are `<name> started`,
`<name> warning`, and `<name> expired`; localized ACT settings UI is not emulated.
Built-in beep uses the shared device and is not an acoustic copy of Windows' system sound.

## Executed checks

| Check                                                         | Environment                                                               | Result / evidence                                                                                                                                                                                                                         |
| ------------------------------------------------------------- | ------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Independent original-compiled `AudioProbe`                    | Genuine ACT 3.8.5.288 and replacement, separate processes on Windows      | Pass: delegate identity/binding, file/sapi volumes, custom/default timer speech and `none`; `target/sound-original-audio.txt`, `target/sound-replacement-audio.txt`                                                                       |
| Managed timer/queue/ABI                                       | Windows native Forms                                                      | Pass: capture labels, callback order, nonmaster suppression, warning/expiry once, full/replaced/closing queues; `target/sound-managed-contracts.txt` and full check                                                                       |
| ACT → Rust injected outputs                                   | Windows and Linux .NET 10/headless Forms                                  | Pass: file + start/warning/expiry, independent volumes, existing TTS failure/restart checks; `target/sound-windows-host.txt`, `target/sound-linux-host.txt`                                                                               |
| ACT → native outputs                                          | Windows SAPI + rodio; Ubuntu WSLg private Speech Dispatcher + rodio/Pulse | Pass: all four fixture cues completed through native APIs, not just queue admission; same host reports                                                                                                                                    |
| Effect device test                                            | Windows default output and Linux WSLg output                              | Pass: WAV file URI, playback cancellation, beep completion, damaged file error; `target/sound-windows-native.txt`, `target/sound-linux-native.txt`                                                                                        |
| Unmodified maintained OverlayPlugin 0.19.111 + cactbot 0.37.5 | Original CEF and real WS/Edge on Windows                                  | Pass: two `playSound` requests and two `cactbotSay` requests reach ACT queues; live raidboss alerts/timeline/reset still pass; `target/sound-overlay-integration.txt`, `target/overlay-cactbot-20261021/stack-cef/{sound,tts}-route.json` |

## Review follow-up

| Finding                                                        | Fix and regression check                                                                                                             |
| -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Effect failures inherited TTS's error classification           | Separate `Error::Sound`; inject two failures and observe both across subsequent Pumps                                                |
| A special-file source could block file reading/worker teardown | Reject nonregular and oversized files before open; check directories on both platforms and `/dev/zero` on Linux before device access |
| More than one host test could race the process-wide CLR lease  | Serialize the speech integration scenarios; default parallel Rust test runner now passes                                             |

Full `mise run check` and Windows/Linux all-feature clippy passed. Native
Windows/Linux host and effect tests were rerun after the review fixes. Other
than the documented scope boundaries, the final review found no remaining
blocking issue for these controlled scenarios.

The original reference EXE is SHA-256
`934fc00d0a3c8325b0f7445358860b4c013c5dab3d8fff012512acf6502dd657`.
The oracle installs only the option controls/delegates needed by original methods;
it does not start ACT's application or use its WMP audio engine. Static REA identity
evidence: `ev_cedd981ddf9f10f6869ce9d72e036ca6c4f8a318dc2bdb7f5f3773830cc65446`.
See [Overlay/cactbot validation](overlay-cactbot-validation.md) for pinned artifacts.

The browser handler harness records original transport separately from native
playback. The subsequent [browser audio validation](browser-audio-validation.md)
enables CEF HTMLAudio and verifies remote browser HTMLAudio/Web Speech on Windows
and Linux. Native CEF coverage remains Windows only; real raid audio and acoustic
quality are unverified. Completion is measured through native APIs/events.

## Reproduce

```sh
# DOTNET_ROOT points to .NET 10; Windows caller owns an STA through the fixture.
mise run check
mise run test:audio
cargo test -p aftercast-tts --test sound -- --include-ignored --nocapture
cargo test -p aftercast --features audio --test speech -- --ignored --nocapture
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

On Linux, install libasound2-dev, libspeechd-dev, libclang-dev, Speech Dispatcher
and a local voice module. Start a private dispatcher and set `SPEECHD_ADDRESS` for
isolated testing. WSLg tests used an explicit ALSA `type pulse` default and
`PULSE_SERVER=unix:/mnt/wslg/PulseServer`; no user's global audio config was edited.
The private Linux runner and logs are under `target/sound-linux-host.sh` and
`target/sound-linux-host.txt` (ignored evidence).
