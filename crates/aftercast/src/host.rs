use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::task::{Context, Poll, Waker};
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

use aftercast_tts::{SoundPlayer, Speaker};
use bytes::{BufMut, BytesMut};
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
    Canonicalize(Utf8PathBuf, Reply<Utf8PathBuf>),
}

pub(crate) struct Control {
    stopping: AtomicBool,
    failure: Mutex<Option<String>>,
}

impl Control {
    pub(crate) fn error(&self) -> Error {
        match self.failure.lock() {
            Ok(failure) => failure
                .as_ref()
                .map_or(Error::Closed, |s| Error::WorkerFailed(s.clone())),
            Err(_) => Error::WorkerFailed("host failure state was poisoned".into()),
        }
    }
    fn record(&self, result: &Result<(), Error>) {
        if let Err(error) = result
            && let Ok(mut failure) = self.failure.lock()
            && failure.is_none()
        {
            *failure = Some(error.to_string());
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
                worker_main(
                    config,
                    receiver,
                    ready,
                    finished,
                    worker_control,
                    audio_errors,
                )
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
    pub(crate) async fn canonicalize(&self, path: &Utf8Path) -> Result<Utf8PathBuf, Error> {
        let path = path.to_owned();
        self.execute(|reply| Command::Canonicalize(path, reply))
            .await
    }
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
            control: self.control.clone(),
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

fn worker_main(
    config: HostConfig,
    receiver: mpsc::Receiver<Command>,
    ready: oneshot::Sender<u64>,
    finished: Reply<()>,
    control: Arc<Control>,
    audio_errors: tokio::sync::mpsc::Sender<Error>,
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _apartment = Apartment::new()?;
        let core = CoreHost::start(config)?;
        if ready.send(core.session.id()).is_err() {
            return core.stop();
        }
        run(core, receiver, &control, audio_errors)
    }))
    .unwrap_or_else(|_| Err(Error::WorkerFailed("host thread panicked".into())));
    control.record(&result);
    control.stopping.store(true, Ordering::Release);
    drop(finished.send(result));
}

enum Operation {
    Import(PendingImport),
    Archive(Reply<(i32, BytesMut)>),
    Audio(
        Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>,
        Reply<()>,
    ),
}

impl Operation {
    fn import_progress(&self) -> Option<ImportProgress> {
        match self {
            Self::Import(import) => Some(*import.progress.borrow()),
            _ => None,
        }
    }
    fn advance(self, session: &ManagedSession, stopping: bool) -> Option<Self> {
        match self {
            Self::Import(mut import) => match import.advance(session, stopping) {
                None => Some(Self::Import(import)),
                Some(result) => {
                    drop(import.complete.send(result));
                    None
                }
            },
            Self::Archive(reply) => {
                let mut packet = BytesMut::with_capacity(64);
                packet.put_u32_le(64);
                packet.put_u32_le(1);
                packet.put_u64_le(session.id());
                packet.resize(64, 0);
                let result = session.request(Request::Archive, &mut packet);
                if matches!(result, Ok(1)) {
                    return Some(Self::Archive(reply));
                }
                drop(reply.send(result.map(|status| (status, packet))));
                None
            }
            Self::Audio(mut cleanup, reply) => {
                // The host's bounded tick polls cleanup; no extra runtime or task executor is needed.
                match cleanup
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                {
                    Poll::Pending => Some(Self::Audio(cleanup, reply)),
                    Poll::Ready(result) => {
                        drop(reply.send(result));
                        None
                    }
                }
            }
        }
    }
}

fn run(
    mut core: CoreHost,
    receiver: mpsc::Receiver<Command>,
    control: &Control,
    audio_errors: tokio::sync::mpsc::Sender<Error>,
) -> Result<(), Error> {
    let mut operations = None;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_loop(
            &mut core,
            &receiver,
            control,
            &audio_errors,
            &mut operations,
        )
    }))
    .unwrap_or_else(|_| Err(Error::WorkerFailed("host command panicked".into())));
    // Publish terminal failure before dropping response senders so waiting callers retain its cause.
    control.record(&outcome);
    control.stopping.store(true, Ordering::Release);
    drop(receiver);
    while let Some(operation) = operations.take() {
        operations = operation.advance(&core.session, true);
        drop(core.session.call(4));
        thread::park_timeout(PUMP_INTERVAL);
    }
    let stopped = core.stop();
    outcome.and(stopped)
}

fn run_loop(
    core: &mut CoreHost,
    receiver: &mpsc::Receiver<Command>,
    control: &Control,
    audio_errors: &tokio::sync::mpsc::Sender<Error>,
    operations: &mut Option<Operation>,
) -> Result<(), Error> {
    loop {
        let stopping = control.stopping.load(Ordering::Acquire);
        if stopping {
            return Ok(());
        }
        let callbacks = core.session.call(4)?.runtime_major;
        for result in [core.pump_speech(), core.pump_sounds()] {
            if let Err(error) = result {
                audio_errors.try_send(error).map_err(audio_overflow)?;
            }
        }
        let before = operations.as_ref().and_then(Operation::import_progress);
        *operations = operations
            .take()
            .and_then(|operation| operation.advance(&core.session, false));
        let progressed = before != operations.as_ref().and_then(Operation::import_progress);
        let started = Instant::now();
        let mut commands = 0;
        for _ in 0..8 {
            if control.stopping.load(Ordering::Acquire) || started.elapsed() >= PUMP_INTERVAL {
                break;
            }
            match receiver.try_recv() {
                Ok(command) => {
                    dispatch(core, command, operations);
                    commands += 1;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }
        // Budgets already provide fairness; only idle/pending work needs a timed wait.
        if commands == 0 && callbacks == 0 && !progressed {
            thread::park_timeout(PUMP_INTERVAL);
        }
    }
}

fn audio_overflow(_error: tokio::sync::mpsc::error::TrySendError<Error>) -> Error {
    Error::WorkerFailed("audio error queue is full or closed".into())
}

fn dispatch(core: &mut CoreHost, command: Command, operations: &mut Option<Operation>) {
    match command {
        Command::Canonicalize(path, reply) => {
            drop(reply.send(crate::runtime::canonicalize(&path)));
        }
        Command::Request(request, mut packet, reply) => {
            if operations.is_some()
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
            if matches!(request, Request::Archive) && matches!(result, Ok((1, _))) {
                *operations = Some(Operation::Archive(reply));
                return;
            }
            drop(reply.send(result));
        }
        Command::EndCombat(reply) => {
            drop(reply.send(if operations.is_some() {
                Err(Error::Busy)
            } else {
                core.end_combat()
            }));
        }
        Command::Tts(speaker, reply) => {
            if operations.is_some() {
                drop(reply.send(Err(Error::Busy)));
                return;
            }
            match core.replace_tts(speaker) {
                Ok(Some(previous)) => {
                    *operations = Some(Operation::Audio(
                        Box::pin(async move { previous.shutdown().await.map_err(Error::Speech) }),
                        reply,
                    ))
                }
                result => drop(reply.send(result.map(|_| ()))),
            }
        }
        Command::Sound(player, reply) => {
            if operations.is_some() {
                drop(reply.send(Err(Error::Busy)));
                return;
            }
            match core.replace_sound(player) {
                Ok(Some(previous)) => {
                    *operations = Some(Operation::Audio(
                        Box::pin(async move { previous.shutdown().await.map_err(Error::Sound) }),
                        reply,
                    ))
                }
                result => drop(reply.send(result.map(|_| ()))),
            }
        }
        Command::Import(path, pending, reply) => {
            let result = if operations.is_some() {
                Err(Error::Busy)
            } else {
                pending.begin(&core.session, &path)
            };
            if result.is_ok() {
                *operations = Some(Operation::Import(pending));
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
        let stopped = self.session.stop();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::task::Poll;

    async fn assert_pending<F: Future>(mut future: std::pin::Pin<&mut F>) {
        std::future::poll_fn(|cx| {
            assert!(future.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
    }

    #[tokio::test]
    async fn bounded_admission_dropped_responses_and_shutdown_do_not_depend_on_a_worker() {
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let control = Arc::new(Control {
            stopping: AtomicBool::new(false),
            failure: Mutex::new(None),
        });
        let handle = HostHandle {
            session: 1,
            sender,
            thread: thread::current(),
            control: control.clone(),
        };
        let unpolled = handle.end_combat();
        assert!(matches!(
            receiver.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        drop(unpolled);
        let mut accepted = Vec::new();
        for _ in 0..QUEUE_CAPACITY {
            let mut future = Box::pin(handle.end_combat());
            assert_pending(future.as_mut()).await;
            accepted.push(future);
        }
        assert!(matches!(handle.end_combat().await, Err(Error::Busy)));
        drop(accepted);
        for _ in 0..QUEUE_CAPACITY {
            assert!(
                matches!(receiver.try_recv(), Ok(Command::EndCombat(reply)) if reply.is_closed())
            );
        }
        control.stopping.store(true, Ordering::Release);
        assert!(matches!(handle.end_combat().await, Err(Error::Closed)));
        control.record(&Err(Error::WorkerFailed("fixture failure".into())));
        assert!(
            matches!(handle.end_combat().await, Err(Error::WorkerFailed(message)) if message.contains("fixture failure"))
        );
    }
}
