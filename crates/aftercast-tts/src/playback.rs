use std::io::Cursor;
use std::time::{Duration, Instant};

use rodio::{DeviceSinkBuilder, Player, buffer::SamplesBuffer};

use crate::{Backend, Cancellation, Error, SynthesizedAudio, Synthesizer, Utterance, failure};

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
