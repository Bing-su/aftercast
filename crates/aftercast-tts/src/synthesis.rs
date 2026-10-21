use std::num::{NonZeroU16, NonZeroU32};

use crate::{Cancellation, Error, Utterance};

/// CPU-owned, interleaved f32 PCM. Engines convert their tensor layout/dtype and
/// device output here; no Burn, ONNX or tensor type crosses the playback boundary.
pub struct PcmAudio {
    pub(crate) samples: Vec<f32>,
    pub(crate) sample_rate: NonZeroU32,
    pub(crate) channels: NonZeroU16,
}

impl PcmAudio {
    /// For example, `PcmAudio::new(vec![0.0; 24_000], 24_000, 1)` is one second of mono silence.
    /// Samples must be finite and normalized to -1..1; channels are interleaved, not planar.
    pub fn new(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Result<Self, Error> {
        let sample_rate = NonZeroU32::new(sample_rate)
            .filter(|rate| rate.get() <= 192_000)
            .ok_or(Error::InvalidAudio("sample rate must be 1..192000 Hz"))?;
        let channels = NonZeroU16::new(channels)
            .filter(|count| count.get() <= 8)
            .ok_or(Error::InvalidAudio("channel count must be 1..8"))?;
        if samples.is_empty()
            || samples.len() > 64 * 1024 * 1024
            || !samples.len().is_multiple_of(usize::from(channels.get()))
        {
            return Err(Error::InvalidAudio(
                "PCM must contain complete frames and 1..67108864 samples",
            ));
        }
        if samples
            .iter()
            .any(|sample| !sample.is_finite() || !(-1.0..=1.0).contains(sample))
        {
            return Err(Error::InvalidAudio(
                "PCM samples must be finite values in -1..1",
            ));
        }
        let frames = samples.len() / usize::from(channels.get());
        if frames as u64 > u64::from(sample_rate.get()) * 300 {
            return Err(Error::InvalidAudio(
                "PCM exceeds the 300-second playback limit",
            ));
        }
        Ok(Self {
            samples,
            sample_rate,
            channels,
        })
    }

    pub fn samples(&self) -> &[f32] {
        &self.samples
    }
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.get()
    }
    pub fn channels(&self) -> u16 {
        self.channels.get()
    }
    pub fn into_samples(self) -> Vec<f32> {
        self.samples
    }
}

/// Unscaled synthesized audio. Playback applies Utterance::volume once, independently of the engine.
#[non_exhaustive]
pub enum SynthesizedAudio {
    Pcm(PcmAudio),
    Mp3(Vec<u8>),
}

/// Synthesis without an audio device. Model, tokenizer, vocoder and compute device
/// belong to the implementation. Load them once in Speaker's factory and reuse them.
///
/// Implementations need not be Send/Sync: construction, inference and destruction
/// stay on the same worker. Return CPU-owned PCM (or encoded audio), honoring
/// voice/rate; volume belongs to Playback. Check cancellation between inference
/// stages/chunks and before device readback. In-flight GPU kernels may not be preemptible.
/// No model download or inference framework is selected by this contract.
pub trait Synthesizer: 'static {
    fn synthesize(
        &mut self,
        utterance: &Utterance,
        cancellation: &Cancellation,
    ) -> Result<SynthesizedAudio, Error>;
}

// Erase model/backend-specific generic types at the application boundary without requiring model state to be Send.
impl<T: Synthesizer + ?Sized> Synthesizer for Box<T> {
    fn synthesize(
        &mut self,
        utterance: &Utterance,
        cancellation: &Cancellation,
    ) -> Result<SynthesizedAudio, Error> {
        (**self).synthesize(utterance, cancellation)
    }
}

#[cfg(test)]
mod tests {
    use super::PcmAudio;

    #[test]
    fn pcm_preserves_layout_and_rejects_invalid_model_outputs() {
        let audio = PcmAudio::new(vec![0.25, -0.25, 0.5, -0.5], 24_000, 2).expect("stereo PCM");
        assert_eq!((audio.sample_rate(), audio.channels()), (24_000, 2));
        assert_eq!(audio.into_samples(), vec![0.25, -0.25, 0.5, -0.5]);
        for (samples, rate, channels) in [
            (vec![], 24_000, 1),
            (vec![0.0], 0, 1),
            (vec![0.0], 24_000, 0),
            (vec![0.0], 24_000, 2),
            (vec![f32::NAN], 24_000, 1),
            (vec![f32::INFINITY], 24_000, 1),
            (vec![1.1], 24_000, 1),
            (vec![0.0; 301], 1, 1),
        ] {
            assert!(matches!(
                PcmAudio::new(samples, rate, channels),
                Err(crate::Error::InvalidAudio(_))
            ));
        }
    }
}
