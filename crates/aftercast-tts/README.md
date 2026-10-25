# aftercast-tts

Standalone Rust TTS with a pluggable backend trait. Backends own their native
resources on one worker thread; callers enqueue speech without waiting for
playback. Applications can use it independently of ACT and without providing
an async runtime.

| Backend                                  | Platform       | Requirements                                                                                               | Network                                                             |
| ---------------------------------------- | -------------- | ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| `Sapi` / `Speaker::system()`             | Windows        | Installed SAPI voices and an audio device                                                                  | Local synthesis                                                     |
| `SpeechDispatcher` / `Speaker::system()` | Linux          | libspeechd, a running/auto-startable Speech Dispatcher, configured local synthesis module and audio output | Local synthesis when configured with a local module, e.g. espeak-ng |
| `Edge`                                   | Windows, Linux | Edge Read Aloud service and a working output device for playback                                           | Explicitly selected online synthesis; text is sent to the service   |

There is no automatic fallback from a local backend to Edge. Missing voices,
daemon/device failures, queue overflow and synthesis/playback failures are errors.
Edge uses the Rust `msedge-tts` client and `rodio` playback; Python, Edge browser
and an external audio-player executable are not required.

## Local speech

```rust,no_run
use aftercast_tts::{Speaker, Utterance};

let speaker = Speaker::system()?;
let speech = speaker.speak(Utterance::new("전투를 시작합니다"))?;
// Queue acceptance and eventual playback completion are separate results.
speech.wait()?;
# Ok::<(), aftercast_tts::Error>(())
```

| Cargo features                                      | Effect                                                                                 |
| --------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Default: `system`, `edge`                           | Build local platform backend and optional online backend                               |
| `default-features = false, features = ["system"]`   | Local backend only; excludes Edge/network/audio-decoder dependencies                   |
| `default-features = false, features = ["edge"]`     | Edge only; Linux does not require libspeechd                                           |
| `default-features = false, features = ["playback"]` | PCM/MP3 playback adapter for custom inference engines; no Edge/system-TTS dependencies |
| No features                                         | Trait, bounded worker and custom backends only; used by the ACT host dependency        |

Linux builds with `system` require libspeechd headers and libclang for generated
bindings. Edge playback builds require ALSA development headers. For Debian/Ubuntu:

```sh
sudo apt-get install libspeechd-dev libclang-dev speech-dispatcher speech-dispatcher-espeak-ng libasound2-dev
cargo run -p aftercast-tts --no-default-features --features system --example speak -- system 'Offline speech test'
```

The caller chooses/configures installed voices. `Sapi::voices()` returns SAPI
token IDs; `SpeechDispatcher::voices()` returns names from its configured output
module. Linux's default selection matches the daemon language, falling back to
the first available voice. `None` restores the backend's default selection after
a request that explicitly selected another voice. Language quality and coverage
depend on the installed local voices.

## Explicit online speech

```rust,no_run
use aftercast_tts::{Edge, Speaker, Utterance};

let speaker = Speaker::new(|| Edge::new("ko-KR-SunHiNeural"))?;
speaker.speak(Utterance::new("음성 테스트입니다"))?.wait()?;
# Ok::<(), aftercast_tts::Error>(())
```

Edge constructor does not contact the service. A request connects and synthesizes
with a 30-second total network deadline, cancellation checks and a 16 MiB received
audio limit. Plain text is escaped before SSML encoding. The service protocol can
change; failures are returned without silently retrying or changing providers.

## Requests, completion and cancellation

| Contract                | Behavior                                                                                                                              |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `Utterance`             | Plain text, up to 4096 UTF-8 bytes; optional backend-specific voice ID; rate -100..100 (0 normal), volume 0..100                      |
| Rate mapping            | SAPI maps to -10..10; Speech Dispatcher to -100..100; Edge uses percentage adjustment. Equal values do not imply equal acoustic speed |
| Queue                   | FIFO, 32 waiting requests plus the active request; `speak()` returns `QueueFull` immediately when full                                |
| `Speech::wait()`        | Blocks for the final playback result; avoid on an ACT/UI thread                                                                       |
| `Speech::try_wait()`    | Polls and consumes a completed result once; later polls report `Closed`                                                               |
| `Speech::cancel()`      | Cooperative cancellation of this request; queued cancelled requests never reach the backend                                           |
| `Speaker::cancel_all()` | Cancels active and currently queued requests; later submissions are allowed                                                           |
| Dropping `Speech`       | Does not cancel playback                                                                                                              |
| Dropping `Speaker`      | Cancels its requests and joins its worker before returning                                                                            |
| Built-in playback       | Polls cancellation at approximately 20 ms intervals and has a 300-second deadline; OS/library calls can add latency                   |

Implement `Backend::speak(&mut self, &Utterance, &Cancellation)` to add an engine.
Construct it with `Speaker::new(move || MyBackend::new(...))`; the factory is Send,
but the backend itself need not be. The worker creates, uses and drops it on the
same thread, keeping COM/native handles out of other threads. The method returns
after playback completes and must cooperate with cancellation and bound external
I/O waits. Arbitrary third-party backends that ignore this contract can delay
shutdown. Shared queues and lifecycle handling remain in Speaker.

## Preparing for local neural TTS

Implement `Synthesizer` when an engine produces audio rather than playing it
directly. `Playback<S>` adapts it to the same Backend/queue contract:

```rust,no_run
use aftercast_tts::{Error, Playback, Speaker, Synthesizer};

// load() can create a non-Send model/device/tokenizer on its owning worker.
fn speaker_for_model<S: Synthesizer>(
    load: impl FnOnce() -> Result<S, Error> + Send + 'static,
) -> Result<Speaker, Error> {
    Speaker::new(move || load().map(Playback::new))
}
```

| Extension contract                              | Responsibility                                                                                                                  |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `Synthesizer::synthesize`                       | Convert plain text to `SynthesizedAudio`; apply requested voice/rate, leave volume to Playback                                  |
| `PcmAudio::new(samples, sample_rate, channels)` | Validate CPU-owned interleaved f32 PCM; preserve the model's actual sample rate/channel count                                   |
| PCM limits                                      | Finite normalized -1..1 samples, 1..8 channels, 1..192000 Hz, up to 64 Mi samples and 300 seconds                               |
| `SynthesizedAudio::Mp3`                         | Encoded output, e.g. Edge; common playback checks size and decodes it                                                           |
| Model lifecycle                                 | Factory loads model/tokenizer/vocoder/device once; synthesize reuses them; Drop releases them on that same thread               |
| Device boundary                                 | Engine performs tensor dtype/layout conversion and CPU readback; no framework-specific tensor/device type enters the common API |
| Runtime engine selection                        | `Box<dyn Synthesizer>` works with Playback; `Box<dyn Backend>` works with Speaker factories                                     |
| Cancellation                                    | Check between inference stages/chunks and before readback; an already executing GPU kernel need not be preemptible              |

This seam is intended to accept future Burn or burn-onnx implementations for
Hugging Face models. Model acquisition, tokenizer/phonemizer, weights, vocoder,
compute backend and engine-specific settings stay with that implementation.
Voice IDs can select engine-defined speaker profiles. No Burn/Hugging Face
dependency, model download or real neural inference is implemented now.
The current contract returns a complete utterance; streaming inference output is
deferred. Edge also implements Synthesizer and uses the same playback path;
`Edge::synthesize_mp3` remains available for headless byte-level verification.

## Verification

Local effects use the independent `SoundPlayer` queue with the `playback` feature:

```rust,no_run
use aftercast_tts::{SoundPlayer, SoundRequest};
let sounds = SoundPlayer::system()?;
sounds.play(SoundRequest::new("beep", 30))?.wait()?;
# Ok::<(), aftercast_tts::Error>(())
```

| Output         | Contract                                                                              |
| -------------- | ------------------------------------------------------------------------------------- |
| `SoundBackend` | Worker-owned injectable player; no ACT types or TTS model dependency                  |
| `FilePlayback` | Direct blocking local WAV/MP3/Ogg Vorbis/FLAC/file URI or beep, default device        |
| `SoundPlayer`  | Separate FIFO, same completion/cancellation handle contract as speech                 |
| Limits         | 4096-byte source, 16 MiB encoded file, 300-second playback, no implicit network fetch |

See `docs/audio-validation.md` for ACT timer and original plugin evidence.

```sh
cargo test -p aftercast-tts
cargo test -p aftercast-tts --no-default-features
cargo test -p aftercast-tts --test native system_playback -- --ignored --nocapture
# Explicitly contacts Edge; does not open an audio device.
cargo test -p aftercast-tts --test native edge_synthesizes -- --ignored --nocapture
cargo run -p aftercast-tts --example speak -- edge '음성 테스트' ko-KR-SunHiNeural
```

Windows SAPI and Linux Speech Dispatcher playback completion, cancellation and
recovery were run locally. Edge real Korean MP3 synthesis and Windows/Linux playback
were also run. Automated completion is based on backend/device APIs; audible
quality is not measured. See the repository's `docs/tts-validation.md` for evidence
and ACT integration limits.
