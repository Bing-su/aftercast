//! Pluggable TTS, independent of ACT and .NET. System backends use local voices;
//! `Edge` is an explicitly selected online backend when the `edge` feature is enabled.
//!
//! ```no_run
//! use aftercast_tts::{Speaker, Utterance};
//! let speaker = Speaker::system()?;
//! speaker.speak(Utterance::new("전투를 시작합니다"))?.wait()?;
//! # Ok::<(), aftercast_tts::Error>(())
//! ```

#[cfg(feature = "edge")]
mod edge;
#[cfg(feature = "playback")]
mod playback;
#[cfg(all(windows, feature = "system"))]
mod sapi;
mod sound;
#[cfg(all(target_os = "linux", feature = "system"))]
mod speech_dispatcher;
mod synthesis;
mod worker;

use worker::Worker;

#[cfg(feature = "edge")]
pub use edge::Edge;
#[cfg(feature = "playback")]
pub use playback::{FilePlayback, Playback};
#[cfg(all(windows, feature = "system"))]
pub use sapi::Sapi;
pub use sound::{SoundBackend, SoundPlayer, SoundRequest};
#[cfg(all(target_os = "linux", feature = "system"))]
pub use speech_dispatcher::SpeechDispatcher;
pub use synthesis::{PcmAudio, SynthesizedAudio, Synthesizer};
pub use worker::{Cancellation, Speech};

/// Recoverable failures; queue acceptance does not imply synthesis/playback succeeded.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("invalid speech request: {0}")]
    InvalidRequest(&'static str),
    #[error("invalid synthesized audio: {0}")]
    InvalidAudio(&'static str),
    #[error("speech queue is full")]
    QueueFull,
    #[error("speech was cancelled")]
    Cancelled,
    #[error("speech worker is closed")]
    Closed,
    #[error("speech backend failed: {0}")]
    Backend(String),
}

/// Plain text only. Voice IDs belong to the selected backend; rate is -100..100,
/// with 0 meaning its normal rate. Volume is 0..100. Numeric mappings are not acoustically identical.
#[derive(Clone, Debug)]
pub struct Utterance {
    pub text: String,
    pub voice: Option<String>,
    pub rate: i16,
    pub volume: u8,
}

impl Utterance {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            voice: None,
            rate: 0,
            volume: 100,
        }
    }

    /// Shared validation for direct backend/synthesizer callers; Speaker and Playback call this before dispatch.
    pub fn validate(&self) -> Result<(), Error> {
        if self.text.trim().is_empty()
            || self.text.len() > 4096
            || self
                .text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        {
            return Err(Error::InvalidRequest(
                "text must be nonempty, at most 4096 UTF-8 bytes, and contain no unsupported control characters",
            ));
        }
        if !(-100..=100).contains(&self.rate) || self.volume > 100 {
            return Err(Error::InvalidRequest(
                "rate or volume is outside its supported range",
            ));
        }
        if self
            .voice
            .as_ref()
            .is_some_and(|v| v.is_empty() || v.len() > 512 || v.chars().any(char::is_control))
        {
            return Err(Error::InvalidRequest(
                "voice ID must contain 1..512 bytes without control characters",
            ));
        }
        Ok(())
    }
}

/// Backends are constructed, called and destroyed on one worker thread, so COM
/// objects need not implement Send. Return only after playback ends or cancellation is handled.
/// Implementations must check cancellation regularly and bound external I/O waits.
pub trait Backend: 'static {
    fn speak(&mut self, utterance: &Utterance, cancellation: &Cancellation) -> Result<(), Error>;
}

// Keep runtime backend selection usable with the same worker factory as concrete engine types.
impl<T: Backend + ?Sized> Backend for Box<T> {
    fn speak(&mut self, utterance: &Utterance, cancellation: &Cancellation) -> Result<(), Error> {
        (**self).speak(utterance, cancellation)
    }
}

/// A bounded FIFO (32 waiting requests plus the active utterance). Failures are
/// returned through Speech, never converted to successful silence. No network fallback is implicit.
pub struct Speaker(Worker<Utterance>);

impl Speaker {
    /// Initialize without blocking the caller; the factory still runs on the audio owner thread.
    pub async fn new_async<B: Backend>(
        factory: impl FnOnce() -> Result<B, Error> + Send + 'static,
    ) -> Result<Self, Error> {
        Worker::new_async(factory, B::speak).await.map(Self)
    }

    pub async fn system_async() -> Result<Self, Error> {
        #[cfg(all(windows, feature = "system"))]
        {
            Self::new_async(Sapi::new).await
        }
        #[cfg(all(target_os = "linux", feature = "system"))]
        {
            Self::new_async(SpeechDispatcher::new).await
        }
        #[cfg(not(all(feature = "system", any(windows, target_os = "linux"))))]
        {
            Err(Error::Backend(
                "enable the system feature for a local backend".into(),
            ))
        }
    }
    /// Cancel accepted work and wait asynchronously for native backend disposal.
    pub async fn shutdown(self) -> Result<(), Error> {
        self.0.shutdown().await
    }
    /// Explicit blocking cleanup for a dedicated owner thread.
    pub fn shutdown_blocking(self) -> Result<(), Error> {
        self.0.shutdown_blocking()
    }
    /// Initialize a backend on its owning worker, reporting initialization errors before returning.
    pub fn new<B: Backend>(
        factory: impl FnOnce() -> Result<B, Error> + Send + 'static,
    ) -> Result<Self, Error> {
        Worker::new(factory, B::speak).map(Self)
    }

    /// Local system backend only: SAPI on Windows, Speech Dispatcher on Linux.
    pub fn system() -> Result<Self, Error> {
        #[cfg(all(windows, feature = "system"))]
        {
            Self::new(Sapi::new)
        }
        #[cfg(all(target_os = "linux", feature = "system"))]
        {
            Self::new(SpeechDispatcher::new)
        }
        #[cfg(not(all(feature = "system", any(windows, target_os = "linux"))))]
        {
            Err(Error::Backend(
                "enable the system feature on Windows or Linux for a local backend".into(),
            ))
        }
    }

    pub fn speak(&self, utterance: Utterance) -> Result<Speech, Error> {
        utterance.validate()?;
        self.0.submit(utterance)
    }

    pub fn cancel_all(&self) {
        self.0.cancel_all();
    }
}

#[cfg(any(
    feature = "edge",
    feature = "playback",
    all(feature = "system", any(windows, target_os = "linux"))
))]
fn failure(error: impl std::fmt::Display) -> Error {
    Error::Backend(error.to_string())
}
