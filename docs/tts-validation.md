# TTS implementation and verification — 2026-10-21

TTS is implemented in the standalone `aftercast-tts` crate with local SAPI and
Speech Dispatcher backends and an explicitly selected online Edge backend.
ACT's `FormActMain.TTS(string)` forwards bounded, owned UTF-8 requests through the
existing bridge to Rust. The subsequent [audio follow-up](audio-validation.md)
adds effects and spell-timer cues; notification windows remain unsupported.

## Architecture and usage

| Owner                      | Responsibility                                                                                            |
| -------------------------- | --------------------------------------------------------------------------------------------------------- |
| `aftercast-tts::Backend`   | Engine contract: play a request, report completion/failure and cooperate with cancellation                |
| `Speaker` / `Speech`       | Worker ownership, bounded FIFO, final results and cancellation                                            |
| `Synthesizer` / `PcmAudio` | Framework-independent inference output, with explicit sample rate, channels and validated CPU f32 samples |
| `Playback<S>`              | Reusable PCM/MP3 playback for synthesis-only engines, including Edge; optional offline `playback` feature |
| `Sapi`                     | Thread-owned COM apartment and installed Windows voice playback                                           |
| `SpeechDispatcher`         | Linux daemon connection, local voice selection and completion/cancel notifications                        |
| `Edge`                     | Online synthesis via msedge-tts 0.4.1; MP3 playback via rodio 0.23                                        |
| ACT compatibility          | Exact `void TTS(string)` member; diagnostics for invalid/disabled/full requests; no engine-specific logic |
| Rust host                  | Explicit backend injection, automatic delivery, async error reception and shutdown                        |

For standalone playback, add `aftercast-tts` directly to your application's dependencies.
ACT host playback runs inside `aftercast-worker`; build that executable with `--features audio`.
Custom worker executables call `run_worker_with_audio` to construct their own backends.

```toml
[dependencies]
aftercast-tts = { path = "../aftercast/crates/aftercast-tts", default-features = false, features = [
  "system",
] }
```

```rust,ignore
// Configure before plugin construction/initialization; no backend is enabled implicitly.
host.set_tts(true).await?;
// The library drives playback; receive errors alongside application work when audio is enabled.
if let Some(error) = host.next_audio_error().await { eprintln!("{error}"); }
host.clear_tts().await?;
```

`set_tts` cancels the previous backend and clears speech waiting in the managed
queue. `clear_tts` disables ACT speech and cancels both queues. The bridge holds at
most 32 waiting requests; the Rust host retains at most 32 accepted jobs. Errors
from completed jobs arrive as `aftercast::Error::Speech` through
`Host::next_audio_error().await`. Consume that receiver when audio is enabled;
32 unconsumed failures fill its queue and the next failure stops the host visibly.
Pending speech is canceled on host shutdown. Direct Speaker users can await
`Speech::wait_async`, or use the blocking wait/poll methods from synchronous code.

The dated device results below predate the async host refactor. Current Linux
fixtures cover async construction/completion/disposal and ACT playback error
delivery; Windows runtime/device checks must be repeated for the new model.

The host adapter currently uses the backend's default voice, normal rate and
full volume. Standalone requests can override voice/rate/volume. ACT's global
speech settings UI, regex speech corrections, duplicate suppression, concurrent
SAPI modes and WAV intermediary/cache workflows are not replicated. Original
ACT's decompiled TTS calls a replaceable playback delegate, applies configured
corrections and catches/logs failures. This implementation preserves the member
binding and diagnostic failure boundary while deliberately using bounded FIFO
playback. Full ACT audio-policy equivalence is not claimed.

## Verification

| Execution                    | Outcome / tested scope                                                                                                                                                                 |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rust worker tests            | FIFO, Unicode/plain text, validation, initialization failure, backend failure, full queue, active/queued cancellation, new work after cancel, joined shutdown                          |
| Managed tests                | Disabled/full/closing diagnostics, invalid Unicode and bounded text, concurrent admission, queue clearing; ABI session/version/capacity validation                                     |
| Windows SAPI                 | Actual system voice playback completion, cancellation and later playback                                                                                                               |
| Linux Speech Dispatcher      | Actual daemon with espeak-ng and WSLg PulseAudio; playback completion, cancellation and later playback using a private socket/process                                                  |
| Edge                         | Real Korean MP3 synthesis including escaped markup characters; pre/in-flight cancellation; decoded Windows/Linux audio playback completes                                              |
| ACT → Rust                   | Windows and Linux fixture-plugin TTS member invocation delivers unchanged Korean text; successive playback errors reach Pump; restarted host retains no prior backend                  |
| Neural-engine extension seam | Non-Send inference fixture stays loaded across requests and is dropped on its owner thread; invalid PCM/layout rejected; silent PCM playback/cancellation checked without a real model |
| Original cactbot             | Unmodified CEF and WS handlers both call cactbotSay and enqueue the expected ACT TTS text; renderer/timeline/shutdown regressions still pass                                           |

The cactbot check verifies its original handler through managed enqueue; the Rust
delivery and real backend playback are exercised by separate checks. It does not
claim a single real-game-to-audible-output recording. Playback completion is
observed through APIs, without microphone/acoustic comparison.

| Local evidence under target/                                         | Contents                                                                                  |
| -------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| tts-tests.txt, tts-clippy.txt, tts-full-check.txt                    | Unit/contract checks and complete regression result                                       |
| tts-rust-final.txt, tts-linux-clippy.txt, tts-metadata-reference.txt | Final Rust regressions, Linux platform lint and independent original ACT signature check  |
| tts-package-check.txt, tts-synthesis-final.txt                       | Extracted Cargo package verification and non-Send model reuse/destruction contract checks |
| tts-managed-tests.txt, tts-host-tests.txt                            | Managed bounds/ABI and ACT→Rust route/error/restart checks                                |
| tts-windows-native.txt, tts-edge-playback.txt                        | SAPI and Edge live checks                                                                 |
| tts-linux-tests.txt, tts-linux-all.txt, tts-linux-native.txt         | Linux builds/tests and private Speech Dispatcher live check                               |
| tts-linux-edge.txt, tts-linux-edge-playback.txt, tts-linux-host.txt  | Linux real Edge synthesis/playback and ACT→Rust route                                     |
| tts-pcm-contract.txt, tts-linux-core.txt, tts-linux-pcm.txt          | Inference-only interface, PCM validation and offline playback extension                   |
| tts-cactbot.txt, overlay-cactbot-20261021/stack-cef/tts-route.json   | Original CEF/WS cactbotSay route and unchanged stack assertions                           |

Native tests are opt-in because they require installed voices/audio or contact
the online service. Linux `system` builds need libspeechd development headers and
libclang; Edge playback also needs ALSA development headers. See the
[crate README](../crates/aftercast-tts/README.md) for commands and feature choices.

## Sources and boundaries

The Synthesizer interface prepares for future Hugging Face model implementations
using Burn or burn-onnx; it does not implement or download a model. The model
implementation owns its tokenizer/phonemizer, weights, vocoder and device, and
returns CPU PCM. Factory construction and Drop stay on one worker, so models and
GPU state need not implement Send/Sync. Cancellation is cooperative between
inference stages; the framework does not promise to interrupt a running GPU
kernel. Full-utterance PCM is supported; incremental streaming is deferred.
Burn operator coverage/model compatibility will be verified when a model is
selected. See [Burn ONNX import](https://burn.dev/books/burn/onnx-import.html).

| Source                                                                                                          | Used for                                                                          |
| --------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| [SAPI Speak flags](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/ee431843%28v%3Dvs.85%29) | Asynchronous plain-text synthesis and purging only the owned voice                |
| [Speech Dispatcher](https://github.com/brailcom/speechd)                                                        | Linux daemon/synthesis module contract                                            |
| [msedge-tts](https://github.com/hs-CN/msedge-tts)                                                               | Maintained Rust Edge protocol implementation, including current service handshake |
| [rodio](https://github.com/RustAudio/rodio)                                                                     | Cross-platform decoding/playback                                                  |

Local backends never automatically fall back to Edge. Speech Dispatcher must be
configured with a local module for offline use; installation and voice selection
remain explicit. Edge sends text to a remote service and requires network access.
Other Linux distributions/audio servers, audible quality, device hotplug, long
runs, full game capture and ACT's remaining audio policies are unverified.
