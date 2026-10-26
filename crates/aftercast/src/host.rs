use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

use aftercast_tts::{SoundPlayer, Speaker};
use bytes::BytesMut;
use camino::{Utf8Path, Utf8PathBuf};
use tokio::sync::{oneshot, watch};

use crate::import::{ImportJob, ImportProgress, PendingImport};
use crate::runtime::{ManagedSession, Request};
use crate::speech::AudioOutput;
use crate::{Error, HostConfig};

const QUEUE_CAPACITY: usize = 32;
const PUMP_INTERVAL: Duration = Duration::from_millis(5);

type Reply<T> = oneshot::Sender<Result<T, Error>>;

pub(crate) enum Command {
    Request(Request, BytesMut, Reply<(i32, BytesMut)>),
    EndCombat(Reply<()>),
    Tts(Option<Speaker>, Reply<()>),
    Sound(Option<SoundPlayer>, Reply<()>),
    Import(Utf8PathBuf, PendingImport, Reply<()>),
}

struct Control {
    stopping: AtomicBool,
    failure: Mutex<Option<String>>,
}

impl Control {
    fn error(&self) -> Error {
        match self.failure.lock() {
            Ok(failure) => failure
                .as_ref()
                .map_or(Error::Closed, |s| Error::WorkerFailed(s.clone())),
            Err(_) => Error::WorkerFailed("host failure state was poisoned".into()),
        }
    }
}

/// Owns the host lifetime. Use `shutdown().await` to observe cleanup errors.
/// Dropping this owner requests shutdown without blocking; handles cannot keep it running.
pub struct Host {
    handle: HostHandle,
    finished: Option<oneshot::Receiver<Result<(), Error>>>,
    audio_errors: tokio::sync::mpsc::Receiver<Error>,
}

/// Cloneable, thread-safe access to the single host thread. No caller-side pump is needed.
/// Accepted mutations continue if their response future is dropped. A full queue returns Busy.
#[derive(Clone)]
pub struct HostHandle {
    pub(crate) session: u64,
    sender: mpsc::SyncSender<Command>,
    thread: Thread,
    control: Arc<Control>,
}

impl Deref for Host {
    type Target = HostHandle;
    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

impl Host {
    /// Starts the CLR and host on a library-owned thread (STA on Windows).
    pub async fn start(config: HostConfig) -> Result<Self, Error> {
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (ready, initialized) = oneshot::channel::<u64>();
        let (finished, completion) = oneshot::channel();
        let (audio_errors, errors) = tokio::sync::mpsc::channel(32);
        let control = Arc::new(Control {
            stopping: AtomicBool::new(false),
            failure: Mutex::new(None),
        });
        let worker_control = control.clone();
        let worker = thread::Builder::new()
            .name("aftercast-host".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _apartment = Apartment::new()?;
                    let core = CoreHost::start(config)?;
                    let session = core.session.id();
                    if ready.send(session).is_err() {
                        return core.stop();
                    }
                    run(core, receiver, &worker_control, audio_errors)
                }))
                .unwrap_or_else(|_| Err(Error::WorkerFailed("host thread panicked".into())));
                if let Err(error) = &result {
                    if let Ok(mut failure) = worker_control.failure.lock() {
                        *failure = Some(error.to_string());
                    }
                }
                worker_control.stopping.store(true, Ordering::Release);
                drop(finished.send(result));
            })
            .map_err(|e| Error::WorkerFailed(e.to_string()))?;
        let thread = worker.thread().clone();
        // The completion channel owns joining semantics; dropping the OS handle never blocks the caller.
        drop(worker);
        let session = match initialized.await {
            Ok(session) => session,
            Err(_) => {
                return match completion.await {
                    Ok(Err(error)) => Err(error),
                    _ => Err(control.error()),
                };
            }
        };
        Ok(Self {
            handle: HostHandle {
                session,
                sender,
                thread,
                control,
            },
            finished: Some(completion),
            audio_errors: errors,
        })
    }

    pub fn handle(&self) -> HostHandle {
        self.handle.clone()
    }

    /// Receives playback failures without stopping parsing. Consume this stream when audio is enabled;
    /// failing to consume 32 pending failures stops the host rather than silently losing errors.
    pub async fn next_audio_error(&mut self) -> Option<Error> {
        self.audio_errors.recv().await
    }

    /// Stops admission, finishes cleanup on the owner thread, and reports its outcome.
    /// A plugin callback that never returns can prevent completion.
    pub async fn shutdown(mut self) -> Result<(), Error> {
        self.request_shutdown();
        match self.finished.take() {
            Some(finished) => finished
                .await
                .map_err(|_error| self.handle.control.error())?,
            None => Err(Error::Closed),
        }
    }

    fn request_shutdown(&self) {
        self.handle.control.stopping.store(true, Ordering::Release);
        self.handle.thread.unpark();
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.request_shutdown();
    }
}

impl HostHandle {
    /// Starts an import with explicit cancellation and coalesced progress.
    pub async fn start_import(&self, path: impl AsRef<Utf8Path>) -> Result<ImportJob, Error> {
        let cancellation = Arc::new(AtomicBool::new(false));
        let (progress, updates) = watch::channel(ImportProgress::default());
        let (complete, result) = oneshot::channel();
        let pending = PendingImport {
            cancellation: cancellation.clone(),
            progress,
            complete,
        };
        let path = path.as_ref().to_owned();
        self.execute(|reply| Command::Import(path, pending, reply))
            .await?;
        Ok(ImportJob {
            cancellation,
            thread: self.thread.clone(),
            progress: updates,
            result,
        })
    }
    pub async fn session_id(&self) -> Result<u64, Error> {
        if self.control.stopping.load(Ordering::Acquire) {
            return Err(self.control.error());
        }
        Ok(self.session)
    }

    async fn execute<T>(&self, make: impl FnOnce(Reply<T>) -> Command) -> Result<T, Error> {
        if self.control.stopping.load(Ordering::Acquire) {
            return Err(self.control.error());
        }
        let (reply, result) = oneshot::channel();
        self.sender.try_send(make(reply)).map_err(|e| match e {
            mpsc::TrySendError::Full(_) => Error::Busy,
            mpsc::TrySendError::Disconnected(_) => self.control.error(),
        })?;
        self.thread.unpark();
        result.await.map_err(|_error| self.control.error())?
    }

    pub(crate) async fn request(
        &self,
        request: Request,
        packet: &mut BytesMut,
    ) -> Result<i32, Error> {
        let input = std::mem::take(packet);
        let (status, output) = self
            .execute(|reply| Command::Request(request, input, reply))
            .await?;
        *packet = output;
        Ok(status)
    }

    /// Finalizes active combat after all accepted combat actions have settled.
    pub async fn end_combat(&self) -> Result<(), Error> {
        self.execute(Command::EndCombat).await
    }

    pub async fn set_tts(&self, speaker: Speaker) -> Result<(), Error> {
        self.execute(|reply| Command::Tts(Some(speaker), reply))
            .await
    }
    pub async fn clear_tts(&self) -> Result<(), Error> {
        self.execute(|reply| Command::Tts(None, reply)).await
    }
    pub async fn set_sound_player(&self, player: SoundPlayer) -> Result<(), Error> {
        self.execute(|reply| Command::Sound(Some(player), reply))
            .await
    }
    pub async fn clear_sound_player(&self) -> Result<(), Error> {
        self.execute(|reply| Command::Sound(None, reply)).await
    }
}

fn run(
    mut core: CoreHost,
    receiver: mpsc::Receiver<Command>,
    control: &Control,
    audio_errors: tokio::sync::mpsc::Sender<Error>,
) -> Result<(), Error> {
    let mut importing: Option<PendingImport> = None;
    let outcome = (|| {
        loop {
            let stopping = control.stopping.load(Ordering::Acquire);
            if stopping && importing.is_none() {
                break;
            }
            core.session.call(4)?;
            for result in [core.pump_speech(), core.pump_sounds()] {
                if let Err(error) = result {
                    audio_errors.try_send(error).map_err(|_error| {
                        Error::WorkerFailed("audio error queue is full or closed".into())
                    })?;
                }
            }
            if let Some(import) = &mut importing {
                if let Some(result) = import.advance(&core.session, stopping) {
                    if let Some(import) = importing.take() {
                        drop(import.complete.send(result));
                    }
                }
            }
            let started = Instant::now();
            for _ in 0..8 {
                if control.stopping.load(Ordering::Acquire) || started.elapsed() >= PUMP_INTERVAL {
                    break;
                }
                match receiver.try_recv() {
                    Ok(command) => dispatch(&mut core, command, &mut importing),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
                }
            }
            thread::park_timeout(PUMP_INTERVAL);
        }
        Ok(())
    })();
    control.stopping.store(true, Ordering::Release);
    // Even a pump failure must restore import state before managed teardown.
    while let Some(import) = &mut importing {
        match import.advance(&core.session, true) {
            Some(result) => {
                if let Some(import) = importing.take() {
                    drop(import.complete.send(result));
                }
            }
            None => {
                drop(core.session.call(4));
                thread::park_timeout(PUMP_INTERVAL);
            }
        }
    }
    // Reject pending work before plugin teardown, e.g. a queued import must not start during shutdown.
    drop(receiver);
    let stopped = core.stop();
    outcome.and(stopped)
}

fn dispatch(core: &mut CoreHost, command: Command, importing: &mut Option<PendingImport>) {
    match command {
        Command::Request(request, mut packet, reply) => {
            if importing.is_some()
                && !matches!(
                    request,
                    Request::Diagnostics | Request::Snapshot | Request::Combat | Request::Logs
                )
            {
                drop(reply.send(Err(Error::Busy)));
                return;
            }
            let result = core
                .session
                .request(request, &mut packet)
                .map(|status| (status, packet));
            drop(reply.send(result));
        }
        Command::EndCombat(reply) => {
            drop(reply.send(if importing.is_some() {
                Err(Error::Busy)
            } else {
                core.end_combat()
            }));
        }
        Command::Tts(speaker, reply) => {
            if importing.is_some() {
                drop(reply.send(Err(Error::Busy)));
                return;
            }
            let result = match speaker {
                Some(speaker) => core.set_tts(speaker),
                None => core.clear_tts(),
            };
            drop(reply.send(result));
        }
        Command::Sound(player, reply) => {
            if importing.is_some() {
                drop(reply.send(Err(Error::Busy)));
                return;
            }
            let result = match player {
                Some(player) => core.set_sound_player(player),
                None => core.clear_sound_player(),
            };
            drop(reply.send(result));
        }
        Command::Import(path, pending, reply) => {
            let result = if importing.is_some() {
                Err(Error::Busy)
            } else {
                pending.begin(&core.session, &path)
            };
            if result.is_ok() {
                *importing = Some(pending);
            }
            drop(reply.send(result));
        }
    }
}

pub(crate) struct CoreHost {
    pub(crate) session: ManagedSession,
    pub(crate) speech: Option<AudioOutput<Speaker>>,
    pub(crate) sounds: Option<AudioOutput<SoundPlayer>>,
}

impl CoreHost {
    pub(crate) fn start(config: HostConfig) -> Result<Self, Error> {
        ManagedSession::start(config).map(|session| Self {
            session,
            speech: None,
            sounds: None,
        })
    }
    pub(crate) fn end_combat(&mut self) -> Result<(), Error> {
        self.session.call(5).map(|_| ())
    }
    pub(crate) fn stop(mut self) -> Result<(), Error> {
        let speech = self
            .speech
            .take()
            .map(|output| output.backend.shutdown_blocking())
            .transpose()
            .map_err(Error::Speech);
        let sound = self
            .sounds
            .take()
            .map(|output| output.backend.shutdown_blocking())
            .transpose()
            .map_err(Error::Sound);
        let stopped = self.session.stop();
        speech.and(sound).and(stopped)
    }
}

struct Apartment;
impl Apartment {
    fn new() -> Result<Self, Error> {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
            // SAFETY: This fresh library-owned thread initializes COM once and balances it below.
            let status =
                unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
            if status < 0 {
                return Err(Error::WorkerFailed(format!(
                    "STA initialization failed: {status:#x}"
                )));
            }
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        #[cfg(windows)]
        // SAFETY: Apartment is scoped to the same thread as its successful initialization.
        unsafe {
            windows_sys::Win32::System::Com::CoUninitialize()
        };
    }
}
