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
#[cfg(all(target_os = "linux", feature = "system"))]
mod speech_dispatcher;
mod synthesis;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak, mpsc};
use std::thread::{self, JoinHandle};

#[cfg(feature = "edge")]
pub use edge::Edge;
#[cfg(feature = "playback")]
pub use playback::Playback;
#[cfg(all(windows, feature = "system"))]
pub use sapi::Sapi;
#[cfg(all(target_os = "linux", feature = "system"))]
pub use speech_dispatcher::SpeechDispatcher;
pub use synthesis::{PcmAudio, SynthesizedAudio, Synthesizer};

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

/// Cooperative cancellation shared with a backend. For example, check between audio chunks.
#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn check(&self) -> Result<(), Error> {
        if self.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// One accepted request, including its eventual playback result. Dropping the
/// handle does not cancel speech; use cancel, Speaker::cancel_all, or drop Speaker.
pub struct Speech {
    result: mpsc::Receiver<Result<(), Error>>,
    cancellation: Cancellation,
}

impl Speech {
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }
    pub fn wait(self) -> Result<(), Error> {
        self.result.recv().map_err(|_disconnected| Error::Closed)?
    }
    /// Consume a ready result once; subsequent calls report Closed.
    pub fn try_wait(&self) -> Result<Option<()>, Error> {
        match self.result.try_recv() {
            Ok(result) => result.map(Some),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(Error::Closed),
        }
    }
}

struct Work {
    utterance: Utterance,
    result: mpsc::SyncSender<Result<(), Error>>,
    cancellation: Cancellation,
}

/// A bounded FIFO (32 waiting requests plus the active utterance). Failures are
/// returned through Speech, never converted to successful silence. No network fallback is implicit.
pub struct Speaker {
    sender: Option<mpsc::SyncSender<Work>>,
    pending: Mutex<Vec<Weak<AtomicBool>>>,
    worker: Option<JoinHandle<()>>,
}

impl Speaker {
    /// Initialize a backend on its owning worker, reporting initialization errors before returning.
    pub fn new<B: Backend>(
        factory: impl FnOnce() -> Result<B, Error> + Send + 'static,
    ) -> Result<Self, Error> {
        let (sender, receiver) = mpsc::sync_channel::<Work>(32);
        let (ready, initialized) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("aftercast-tts".into())
            .spawn(move || {
                let mut backend = match factory() {
                    Ok(backend) => {
                        drop(ready.send(Ok(())));
                        backend
                    }
                    Err(error) => {
                        drop(ready.send(Err(error)));
                        return;
                    }
                };
                for work in receiver {
                    let result = work
                        .cancellation
                        .check()
                        .and_then(|()| backend.speak(&work.utterance, &work.cancellation));
                    work.cancellation.0.store(true, Ordering::Release);
                    drop(work.result.send(result));
                }
            })
            .map_err(|e| Error::Backend(e.to_string()))?;
        if let Err(error) = initialized
            .recv()
            .map_err(|_disconnected| Error::Closed)
            .and_then(|r| r)
        {
            drop(worker.join());
            return Err(error);
        }
        Ok(Self {
            sender: Some(sender),
            pending: Mutex::new(Vec::new()),
            worker: Some(worker),
        })
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
        let cancellation = Cancellation(Arc::new(AtomicBool::new(false)));
        let (result, receiver) = mpsc::sync_channel(1);
        let mut pending = self.pending.lock().map_err(|_poisoned| Error::Closed)?;
        // Serialize admission with cancel_all so an accepted request cannot escape a concurrent cancellation.
        self.sender
            .as_ref()
            .ok_or(Error::Closed)?
            .try_send(Work {
                utterance,
                result,
                cancellation: cancellation.clone(),
            })
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => Error::QueueFull,
                mpsc::TrySendError::Disconnected(_) => Error::Closed,
            })?;
        pending.retain(|flag| {
            flag.upgrade()
                .is_some_and(|flag| !flag.load(Ordering::Acquire))
        });
        pending.push(Arc::downgrade(&cancellation.0));
        Ok(Speech {
            result: receiver,
            cancellation,
        })
    }

    pub fn cancel_all(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            for flag in pending.drain(..).filter_map(|flag| flag.upgrade()) {
                flag.store(true, Ordering::Release);
            }
        }
    }
}

impl Drop for Speaker {
    fn drop(&mut self) {
        self.cancel_all();
        self.sender.take();
        // Built-in backends poll cancellation; join also guarantees their native resources stay on the owning thread.
        if let Some(worker) = self.worker.take() {
            drop(worker.join());
        }
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
