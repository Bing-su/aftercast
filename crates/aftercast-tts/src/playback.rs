use std::borrow::Cow;
use std::io::Cursor;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

use rodio::{DeviceSinkBuilder, Player, buffer::SamplesBuffer};

use crate::{Backend, Cancellation, Error, SynthesizedAudio, Synthesizer, Utterance, failure};
use crate::{PcmAudio, SoundBackend, SoundRequest};

/// Local encoded files and a generated beep, using the default output device.
/// For queued playback use `SoundPlayer::system()`; direct calls block until completion.
pub struct FilePlayback;

impl SoundBackend for FilePlayback {
    fn play(&mut self, request: &SoundRequest, cancellation: &Cancellation) -> Result<(), Error> {
        request.validate()?;
        cancellation.check()?;
        if request.source == "beep" {
            // A short, quiet tone works with the same output device on both platforms.
            let samples = (0..4800)
                .map(|n| (n as f32 * 880.0 * std::f32::consts::TAU / 24000.0).sin() * 0.1)
                .collect();
            return play(
                SynthesizedAudio::Pcm(PcmAudio::new(samples, 24000, 1)?),
                request.volume,
                cancellation,
            );
        }
        let path = if request.source.starts_with("file:") {
            Cow::Owned(
                url::Url::parse(&request.source)
                    .map_err(failure)?
                    .to_file_path()
                    .map_err(|()| Error::InvalidRequest("invalid local file URI"))?,
            )
        } else {
            if request.source.contains("://") {
                return Err(Error::InvalidRequest(
                    "system sound playback requires a local file",
                ));
            }
            Cow::Borrowed(Path::new(&request.source))
        };
        // Reject devices/FIFOs before open: reading a pipe could otherwise block cancellation and worker teardown.
        let metadata = std::fs::metadata(&path).map_err(failure)?;
        if !metadata.is_file() || metadata.len() > 16 * 1024 * 1024 {
            return Err(Error::InvalidAudio(
                "sound requires a regular file of at most 16 MiB",
            ));
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        std::fs::File::open(&path)
            .map_err(failure)?
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(failure)?;
        cancellation.check()?;
        // rodio sniffs the container, so the extension need not match WAV/MP3/Ogg/FLAC content.
        play(SynthesizedAudio::Mp3(bytes), request.volume, cancellation)
    }
}

/// Turns an inference-only engine into a queued Backend using common PCM/MP3 playback.
/// Construct inside the worker, e.g. `Speaker::new(|| Ok(Playback::new(MySynthesizer::load()?)))`.
pub struct Playback<S>(S);

impl<S: Synthesizer> Playback<S> {
    pub fn new(synthesizer: S) -> Self {
        Self(synthesizer)
    }
}

impl<S: Synthesizer> Backend for Playback<S> {
    fn speak(&mut self, utterance: &Utterance, cancellation: &Cancellation) -> Result<(), Error> {
        utterance.validate()?;
        cancellation.check()?;
        let audio = self.0.synthesize(utterance, cancellation)?;
        play(audio, utterance.volume, cancellation)
    }
}

pub(crate) fn play(
    audio: SynthesizedAudio,
    volume: u8,
    cancellation: &Cancellation,
) -> Result<(), Error> {
    cancellation.check()?;
    if let SynthesizedAudio::Mp3(bytes) = &audio
        && (bytes.is_empty() || bytes.len() > 16 * 1024 * 1024)
    {
        return Err(Error::InvalidAudio("MP3 must contain 1..16777216 bytes"));
    }
    let mut stream = DeviceSinkBuilder::open_default_sink().map_err(failure)?;
    // Completion/cancellation is handled before teardown, so normal stream drop is not an interrupted-playback warning.
    stream.log_on_drop(false);
    let player = match audio {
        SynthesizedAudio::Pcm(pcm) => {
            let player = Player::connect_new(stream.mixer());
            player.append(SamplesBuffer::new(
                pcm.channels,
                pcm.sample_rate,
                pcm.samples,
            ));
            player
        }
        SynthesizedAudio::Mp3(bytes) => {
            rodio::play(stream.mixer(), Cursor::new(bytes)).map_err(failure)?
        }
    };
    player.set_volume(f32::from(volume) / 100.0);
    let started = Instant::now();
    while !player.empty() {
        if cancellation.is_cancelled() {
            player.stop();
            return Err(Error::Cancelled);
        }
        if started.elapsed() > Duration::from_secs(300) {
            player.stop();
            return Err(Error::Backend("TTS playback timed out".into()));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}
