use crate::worker::Worker;
use crate::{Cancellation, Error, Speech};

/// A local file (including file:// URIs), or the ACT timer's built-in beep.
/// HTTP sources are deliberately not fetched; applications can supply their own SoundBackend.
#[derive(Clone, Debug)]
pub struct SoundRequest {
    pub source: String,
    pub volume: u8,
}

impl SoundRequest {
    pub fn new(source: impl Into<String>, volume: u8) -> Self {
        Self {
            source: source.into(),
            volume,
        }
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.source.trim().is_empty()
            || self.source.len() > 4096
            || self.source.chars().any(char::is_control)
            || self.volume > 100
        {
            return Err(Error::InvalidRequest(
                "sound requires a source of 1..4096 bytes and volume 0..100",
            ));
        }
        Ok(())
    }
}

/// Runs on one owning thread, independently of TTS. Return after playback or cancellation.
pub trait SoundBackend: 'static {
    fn play(&mut self, request: &SoundRequest, cancellation: &Cancellation) -> Result<(), Error>;
}

impl<T: SoundBackend + ?Sized> SoundBackend for Box<T> {
    fn play(&mut self, request: &SoundRequest, cancellation: &Cancellation) -> Result<(), Error> {
        (**self).play(request, cancellation)
    }
}

/// Independent bounded FIFO for effects; dropping it cancels active and queued audio.
/// Speech handles expose the same wait/cancel contract for both kinds of playback.
pub struct SoundPlayer(Worker<SoundRequest>);

impl SoundPlayer {
    pub fn new<B: SoundBackend>(
        factory: impl FnOnce() -> Result<B, Error> + Send + 'static,
    ) -> Result<Self, Error> {
        Worker::new(factory, B::play).map(Self)
    }
    /// Play WAV/MP3/Ogg/FLAC files through the default device on Windows or Linux.
    #[cfg(feature = "playback")]
    pub fn system() -> Result<Self, Error> {
        Self::new(|| Ok(crate::playback::FilePlayback))
    }
    pub fn play(&self, request: SoundRequest) -> Result<Speech, Error> {
        request.validate()?;
        self.0.submit(request)
    }
    pub fn cancel_all(&self) {
        self.0.cancel_all();
    }
}
