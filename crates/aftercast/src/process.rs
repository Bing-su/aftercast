use std::collections::VecDeque;
use std::future::Future;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::pin::Pin;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Waker};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use bytes::BytesMut;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::host::Control;
use crate::local_runtime::ManagedSession;
use crate::runtime::{HostConfig, Packet, Request};
use crate::speech::AudioOutput;
use crate::{Error, io_error};

const MAX_FRAME: usize = 128 * 1024 * 1024;
const TICK: Duration = Duration::from_millis(5);
const CHILD_ARGUMENT: &str = "--aftercast-child";

// Child-only ownership: the parent carries no native delegates, controls or audio workers.
pub(crate) struct ChildHost {
    pub(crate) session: ManagedSession,
    pub(crate) speech: Option<AudioOutput<aftercast_tts::Speaker>>,
    pub(crate) sounds: Option<AudioOutput<aftercast_tts::SoundPlayer>>,
}

fn child_requested() -> bool {
    std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new(CHILD_ARGUMENT))
}

fn bind_parent(parent: u32) -> Result<(), Error> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: prctl installs a death signal with unused arguments zeroed; no pointer is dereferenced.
        let status = unsafe {
            libc::prctl(
                libc::PR_SET_PDEATHSIG,
                libc::SIGKILL,
                0_usize,
                0_usize,
                0_usize,
            )
        };
        // SAFETY: getppid has no preconditions; checking after prctl closes the parent-exit race.
        let actual = unsafe { libc::getppid() };
        if status != 0 {
            return Err(transport(std::io::Error::last_os_error()));
        }
        if u32::try_from(actual).ok() != Some(parent) {
            return Err(transport("supervisor exited before lifetime binding"));
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _parent = parent;
    Ok(())
}

#[cfg(windows)]
fn parent_job(child: &Child) -> Result<std::os::windows::io::OwnedHandle, Error> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::System::JobObjects::*;
    // SAFETY: null security/name create a fresh unnamed, non-inheritable job owned below.
    let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if raw.is_null() {
        return Err(transport(std::io::Error::last_os_error()));
    }
    // SAFETY: CreateJobObjectW returned a unique valid handle, now owned exactly once.
    let job = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    // SAFETY: the structure has the required layout/size and both handles remain alive through these calls.
    let configured = unsafe {
        SetInformationJobObject(
            raw,
            JobObjectExtendedLimitInformation,
            (&raw const limits).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    if configured == 0 {
        return Err(transport(std::io::Error::last_os_error()));
    }
    // SAFETY: the spawned child is awaiting its stdin configuration, before running any plugin code.
    if unsafe { AssignProcessToJobObject(raw, child.as_raw_handle()) } == 0 {
        return Err(transport(std::io::Error::last_os_error()));
    }
    Ok(job)
}

#[derive(Serialize, Deserialize)]
struct Launch {
    token: [u8; 32],
    config: HostConfig,
    parent: u32,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
pub(crate) enum AudioKind {
    Tts,
    Sound,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
enum Message {
    Request(Request),
    Lifecycle(u32),
    ConfigureAudio(AudioKind, bool),
    PollAudio,
}
#[derive(Serialize, Deserialize)]
enum WireError {
    Managed {
        command: u32,
        status: i32,
        message: String,
    },
    Io {
        path: camino::Utf8PathBuf,
        message: String,
    },
    Speech(String),
    Sound(String),
    Other(String),
    StateInUse(camino::Utf8PathBuf),
}
impl From<Error> for WireError {
    fn from(error: Error) -> Self {
        match error {
            Error::Managed {
                command,
                status,
                message,
            } => Self::Managed {
                command,
                status,
                message,
            },
            Error::Io { path, source } => Self::Io {
                path,
                message: source.to_string(),
            },
            Error::Speech(aftercast_tts::Error::Backend(message)) => Self::Speech(message),
            Error::Speech(error) => Self::Speech(error.to_string()),
            Error::Sound(aftercast_tts::Error::Backend(message)) => Self::Sound(message),
            Error::Sound(error) => Self::Sound(error.to_string()),
            Error::WorkerFailed(message) => Self::Other(message),
            Error::StateInUse(path) => Self::StateInUse(path),
            error => Self::Other(error.to_string()),
        }
    }
}
impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        match error {
            WireError::Managed {
                command,
                status,
                message,
            } => Self::Managed {
                command,
                status,
                message,
            },
            WireError::Io { path, message } => io_error(path, std::io::Error::other(message)),
            WireError::Speech(message) => Self::Speech(aftercast_tts::Error::Backend(message)),
            WireError::Sound(message) => Self::Sound(aftercast_tts::Error::Backend(message)),
            WireError::Other(message) => Self::WorkerFailed(message),
            WireError::StateInUse(path) => Self::StateInUse(path),
        }
    }
}
#[derive(Serialize, Deserialize)]
struct Response {
    status: i32,
    lifecycle: Option<Packet>,
    error: Option<WireError>,
    audio: Vec<WireError>,
}
impl Response {
    fn result(result: Result<i32, Error>) -> Self {
        match result {
            Ok(status) => Self {
                status,
                lifecycle: None,
                error: None,
                audio: vec![],
            },
            Err(error) => Self {
                status: 0,
                lifecycle: None,
                error: Some(error.into()),
                audio: vec![],
            },
        }
    }
}
fn transport(error: impl std::fmt::Display) -> Error {
    Error::WorkerFailed(format!("worker IPC: {error}"))
}

// Metadata and binary payload have separate bounded frames; plugin stdout never carries IPC.
fn write_frame(stream: &mut impl Write, bytes: &[u8]) -> Result<(), Error> {
    if bytes.len() > MAX_FRAME {
        return Err(transport("frame exceeds 128 MiB"));
    }
    stream
        .write_all(&(bytes.len() as u32).to_le_bytes())
        .map_err(transport)?;
    stream.write_all(bytes).map_err(transport)
}
fn read_frame(stream: &mut impl Read) -> Result<Vec<u8>, Error> {
    let mut length = [0; 4];
    stream.read_exact(&mut length).map_err(transport)?;
    let length = u32::from_le_bytes(length) as usize;
    if length > MAX_FRAME {
        return Err(transport("frame exceeds 128 MiB"));
    }
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes).map_err(transport)?;
    Ok(bytes)
}
fn write_metadata(stream: &mut impl Write, value: &impl Serialize) -> Result<(), Error> {
    write_frame(stream, &postcard::to_allocvec(value).map_err(transport)?)
}
fn read_metadata<T: DeserializeOwned>(stream: &mut impl Read) -> Result<T, Error> {
    let bytes = read_frame(stream)?;
    let (value, remaining) = postcard::take_from_bytes(&bytes).map_err(transport)?;
    // Each frame carries exactly one value; reject appended data, e.g. a second command.
    if !remaining.is_empty() {
        return Err(transport("trailing metadata bytes"));
    }
    Ok(value)
}

struct Supervisor {
    abort: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<(), Error>>>,
}

fn watch_child(
    child: &mut Child,
    control: &Control,
    ready: &AtomicBool,
    abort: &AtomicBool,
    startup_timeout: Duration,
    shutdown_timeout: Duration,
) -> Result<(), Error> {
    let started = Instant::now();
    let mut stopping = None;
    loop {
        if let Some(status) = child.try_wait().map_err(transport)? {
            return if status.success() {
                Ok(())
            } else {
                Err(transport(format!("worker exited: {status}")))
            };
        }
        if control.stopping.load(Ordering::Acquire) {
            stopping.get_or_insert_with(|| control.shutdown_started());
        }
        let timed_out = stopping.is_some_and(|at: Instant| at.elapsed() >= shutdown_timeout);
        let startup_expired =
            !ready.load(Ordering::Acquire) && started.elapsed() >= startup_timeout;
        if !timed_out && !startup_expired && !abort.load(Ordering::Acquire) {
            thread::sleep(TICK);
            continue;
        }
        // Reap before reporting completion so the same state directory can be reused immediately.
        let killed = child.kill();
        let status = child.wait().map_err(transport)?;
        if let Err(error) = killed
            && !status.success()
        {
            return Err(transport(error));
        }
        if timed_out {
            return Err(Error::ShutdownTimeout(shutdown_timeout));
        }
        if startup_expired {
            return Err(Error::StartupTimeout(startup_timeout));
        }
        return Err(transport("worker abandoned"));
    }
}

impl Supervisor {
    fn start(
        child: Child,
        control: Arc<Control>,
        config: &HostConfig,
        ready: Arc<AtomicBool>,
    ) -> Result<Self, Error> {
        let abort = Arc::new(AtomicBool::new(false));
        let abort_worker = abort.clone();
        let startup_timeout = config.startup_timeout;
        let shutdown_timeout = config.shutdown_timeout;
        // Also cover thread-spawn failure and panics: dropping Child alone would leave an orphan.
        let mut child = scopeguard::guard(child, |mut child| {
            drop(child.kill());
            drop(child.wait());
        });
        #[cfg(windows)]
        let job = parent_job(&child)?;
        // This thread never calls managed code and can kill a worker stuck anywhere in a callback.
        let thread = thread::Builder::new()
            .name("aftercast-supervisor".into())
            .spawn(move || {
                #[cfg(windows)]
                let _job = job;
                watch_child(
                    &mut child,
                    &control,
                    &ready,
                    &abort_worker,
                    startup_timeout,
                    shutdown_timeout,
                )
            })
            .map_err(transport)?;
        Ok(Self {
            abort,
            thread: Some(thread),
        })
    }
    fn finish(&mut self) -> Result<(), Error> {
        match self.thread.take() {
            Some(thread) => thread
                .join()
                .map_err(|_panic| transport("supervisor panicked"))?,
            None => Ok(()),
        }
    }
    fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }
    fn abort(&mut self) -> Result<(), Error> {
        self.abort.store(true, Ordering::Release);
        self.finish()
    }
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        drop(self.abort());
    }
}

fn accept_worker(
    listener: &TcpListener,
    token: &[u8; 32],
    supervisor: &mut Supervisor,
) -> Result<TcpStream, Error> {
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream
                    .set_read_timeout(Some(Duration::from_millis(100)))
                    .map_err(transport)?;
                let mut actual = [0; 32];
                if stream.read_exact(&mut actual).is_ok() && actual == *token {
                    return Ok(stream);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(transport(error)),
        }
        if supervisor.is_finished() {
            supervisor.finish()?;
            return Err(transport("worker exited before authentication"));
        }
        thread::sleep(TICK);
    }
}

pub(crate) struct RemoteSession {
    stream: TcpStream,
    supervisor: Supervisor,
    session: u64,
    audio: VecDeque<Error>,
}
impl RemoteSession {
    pub(crate) fn start(mut config: HostConfig, control: Arc<Control>) -> Result<Self, Error> {
        // A missing application entry hook must fail before it can recursively spawn grandchildren.
        if child_requested() {
            return Err(transport(
                "call run_child_if_requested before Host::start in child mode",
            ));
        }
        if config.startup_timeout.is_zero() || config.shutdown_timeout.is_zero() {
            return Err(transport("lifetime deadlines must be positive"));
        }
        config.dotnet_root = crate::runtime::canonicalize(&config.dotnet_root)?;
        config.bridge_directory = crate::runtime::canonicalize(&config.bridge_directory)?;
        let executable = match &config.application_executable {
            Some(path) => path.clone(),
            None => camino::Utf8PathBuf::try_from(std::env::current_exe().map_err(transport)?)
                .map_err(transport)?,
        };
        let executable = crate::runtime::canonicalize(&executable)?;
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).map_err(transport)?;
        listener.set_nonblocking(true).map_err(transport)?;
        let mut token = [0; 32];
        getrandom::fill(&mut token).map_err(transport)?;
        let mut child = Command::new(&executable)
            .arg(CHILD_ARGUMENT)
            .env(
                "AFTERCAST_WORKER_ENDPOINT",
                listener.local_addr().map_err(transport)?.to_string(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| io_error(&executable, error))?;
        control.process_id.store(child.id(), Ordering::Release);
        let mut input = child
            .stdin
            .take()
            .ok_or_else(|| transport("missing worker stdin"))?;
        let ready = Arc::new(AtomicBool::new(false));
        let mut supervisor = Supervisor::start(child, control, &config, ready.clone())?;
        write_metadata(
            &mut input,
            &Launch {
                token,
                config: config.clone(),
                parent: std::process::id(),
            },
        )?;
        drop(input);
        let stream = accept_worker(&listener, &token, &mut supervisor)?;
        stream.set_nodelay(true).map_err(transport)?;
        stream.set_read_timeout(None).map_err(transport)?;
        let mut session = Self {
            stream,
            supervisor,
            session: 0,
            audio: VecDeque::new(),
        };
        let (response, _) = session.receive().map_err(|error| {
            // Socket EOF can be the effect of the supervisor deadline; keep the actual termination cause.
            match session.supervisor.abort() {
                Err(timeout @ (Error::StartupTimeout(_) | Error::ShutdownTimeout(_))) => timeout,
                _ => error,
            }
        })?;
        session.session = response
            .lifecycle
            .ok_or_else(|| transport("missing startup session"))?
            .session;
        ready.store(true, Ordering::Release);
        Ok(session)
    }
    pub(crate) fn id(&self) -> u64 {
        self.session
    }
    fn receive(&mut self) -> Result<(Response, BytesMut), Error> {
        let mut response: Response = read_metadata(&mut self.stream)?;
        let packet = BytesMut::from(read_frame(&mut self.stream)?.as_slice());
        self.audio.extend(response.audio.drain(..).map(Error::from));
        if let Some(error) = response.error.take() {
            return Err(error.into());
        }
        Ok((response, packet))
    }
    fn exchange(&mut self, message: Message, packet: &[u8]) -> Result<(Response, BytesMut), Error> {
        write_metadata(&mut self.stream, &message)?;
        write_frame(&mut self.stream, packet)?;
        self.receive()
    }
    pub(crate) fn request(
        &mut self,
        request: Request,
        packet: &mut BytesMut,
    ) -> Result<i32, Error> {
        let (response, output) = self.exchange(Message::Request(request), packet)?;
        *packet = output;
        Ok(response.status)
    }
    pub(crate) fn call(&mut self, command: u32) -> Result<Packet, Error> {
        self.exchange(Message::Lifecycle(command), &[])?
            .0
            .lifecycle
            .ok_or_else(|| transport("missing lifecycle response"))
    }
    pub(crate) fn audio(&mut self, kind: AudioKind, enabled: bool) -> Result<i32, Error> {
        self.exchange(Message::ConfigureAudio(kind, enabled), &[])
            .map(|(response, _)| response.status)
    }
    pub(crate) fn poll_audio(&mut self) -> Result<i32, Error> {
        self.exchange(Message::PollAudio, &[])
            .map(|(response, _)| response.status)
    }
    pub(crate) fn audio_error(&mut self) -> Option<Error> {
        self.audio.pop_front()
    }
    pub(crate) fn stop(mut self) -> Result<(), Error> {
        // Retire backends while the managed owner context still exists, including native STA cleanup.
        let mut audio = Ok(());
        for kind in [AudioKind::Tts, AudioKind::Sound] {
            let result = self.retire_audio(kind);
            audio = audio.and(result);
        }
        let result = self.exchange(Message::Lifecycle(3), &[]).map(|_| ());
        let reaped = self.supervisor.finish();
        if matches!(reaped, Err(Error::ShutdownTimeout(_))) {
            return reaped;
        }
        result.and(reaped).and(audio)
    }
    fn retire_audio(&mut self, kind: AudioKind) -> Result<(), Error> {
        let mut status = self.audio(kind, false)?;
        while status == 1 {
            self.call(4)?;
            status = self.poll_audio()?;
            thread::sleep(TICK);
        }
        Ok(())
    }
}

/// Call first in main, before starting a UI or async runtime. Returns true after child mode ends.
/// Normal launches return false; `--aftercast-child` requires an authenticated supervisor launch.
pub fn run_child_if_requested() -> Result<bool, Error> {
    run_child_if_requested_with_audio(
        |_| aftercast_tts::Speaker::system(),
        |_| {
            #[cfg(feature = "audio")]
            {
                aftercast_tts::SoundPlayer::system()
            }
            #[cfg(not(feature = "audio"))]
            {
                Err(aftercast_tts::Error::Backend(
                    "worker requires the audio feature".into(),
                ))
            }
        },
    )
}

/// Like run_child_if_requested, with child-only factories for custom audio backends.
/// For example, construct an application-specific synthesizer using its private state directory.
pub fn run_child_if_requested_with_audio(
    tts: fn(&camino::Utf8Path) -> Result<aftercast_tts::Speaker, aftercast_tts::Error>,
    sound: fn(&camino::Utf8Path) -> Result<aftercast_tts::SoundPlayer, aftercast_tts::Error>,
) -> Result<bool, Error> {
    if !child_requested() {
        return Ok(false);
    }
    run_child(tts, sound)?;
    Ok(true)
}

fn run_child(
    tts: fn(&camino::Utf8Path) -> Result<aftercast_tts::Speaker, aftercast_tts::Error>,
    sound: fn(&camino::Utf8Path) -> Result<aftercast_tts::SoundPlayer, aftercast_tts::Error>,
) -> Result<(), Error> {
    let address = std::env::var("AFTERCAST_WORKER_ENDPOINT").map_err(transport)?;
    let launch: Launch = read_metadata(&mut std::io::stdin().lock())?;
    bind_parent(launch.parent)?;
    let mut stream = TcpStream::connect(address).map_err(transport)?;
    stream.set_nodelay(true).map_err(transport)?;
    stream.write_all(&launch.token).map_err(transport)?;
    let _apartment = Apartment::new()?;
    let state = launch.config.data_directory.clone();
    let core = ManagedSession::start(launch.config).map(|session| ChildHost {
        session,
        speech: None,
        sounds: None,
    });
    let mut startup = Response::result(Ok(0));
    let mut core = match core {
        Ok(core) => {
            startup.lifecycle = Some(Packet {
                size: 24,
                version: 1,
                command: 1,
                runtime_major: 10,
                session: core.session.id(),
            });
            core
        }
        Err(error) => {
            write_metadata(&mut stream, &Response::result(Err(error)))?;
            write_frame(&mut stream, &[])?;
            return Ok(());
        }
    };
    write_metadata(&mut stream, &startup)?;
    write_frame(&mut stream, &[])?;
    let mut audio_cleanup = None;
    loop {
        let message: Message = read_metadata(&mut stream)?;
        let mut packet = BytesMut::from(read_frame(&mut stream)?.as_slice());
        if matches!(message, Message::Lifecycle(3)) {
            // The parent finishes audio retirement before requesting disposal of the managed owner context.
            let result = core.session.stop();
            write_metadata(&mut stream, &Response::result(result.map(|_| 0)))?;
            write_frame(&mut stream, &[])?;
            return Ok(());
        }
        let mut response = match message {
            Message::Request(request) => {
                Response::result(core.session.request(request, &mut packet))
            }
            Message::ConfigureAudio(kind, enabled) => {
                let result = configure_audio(&mut core, kind, enabled, &state, tts, sound);
                Response::result(result.and_then(|cleanup| {
                    audio_cleanup = cleanup;
                    poll_audio(&mut audio_cleanup)
                }))
            }
            Message::PollAudio => Response::result(poll_audio(&mut audio_cleanup)),
            Message::Lifecycle(command) => {
                let mut response = Response::result(Ok(0));
                match core.session.call(command) {
                    Ok(lifecycle) => response.lifecycle = Some(lifecycle),
                    Err(error) => response.error = Some(error.into()),
                }
                response
            }
        };
        if matches!(message, Message::Lifecycle(4)) {
            response.audio.extend(
                [core.pump_speech(), core.pump_sounds()]
                    .into_iter()
                    .filter_map(Result::err)
                    .map(WireError::from),
            );
        }
        write_metadata(&mut stream, &response)?;
        write_frame(&mut stream, &packet)?;
    }
}

type AudioCleanup = Pin<Box<dyn Future<Output = Result<(), Error>>>>;

fn poll_audio(cleanup: &mut Option<AudioCleanup>) -> Result<i32, Error> {
    // Poll between owner pump requests so native teardown can synchronously invoke that owner.
    let Some(pending) = cleanup.as_mut() else {
        return Ok(0);
    };
    match pending
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Pending => Ok(1),
        Poll::Ready(result) => {
            *cleanup = None;
            result.map(|_| 0)
        }
    }
}

fn configure_audio(
    core: &mut ChildHost,
    kind: AudioKind,
    enabled: bool,
    state: &camino::Utf8Path,
    tts: fn(&camino::Utf8Path) -> Result<aftercast_tts::Speaker, aftercast_tts::Error>,
    sound: fn(&camino::Utf8Path) -> Result<aftercast_tts::SoundPlayer, aftercast_tts::Error>,
) -> Result<Option<AudioCleanup>, Error> {
    match kind {
        AudioKind::Tts => {
            let next = if enabled { Some(tts(state)?) } else { None };
            if let Some(previous) = core.replace_tts(next)? {
                return Ok(Some(Box::pin(async move {
                    previous.shutdown().await.map_err(Error::Speech)
                })));
            }
        }
        AudioKind::Sound => {
            let next = if enabled {
                Some(sound(state).map_err(Error::Sound)?)
            } else {
                None
            };
            if let Some(previous) = core.replace_sound(next)? {
                return Ok(Some(Box::pin(async move {
                    previous.shutdown().await.map_err(Error::Sound)
                })));
            }
        }
    }
    Ok(None)
}

struct Apartment;

impl Apartment {
    fn new() -> Result<Self, Error> {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
            // SAFETY: The fresh child main thread initializes COM once and balances it below.
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

    #[test]
    fn error_responses_preserve_error_kinds_and_diagnostics() {
        let path = camino::Utf8PathBuf::from("/state 한글/plugin.dll");
        for error in [
            Error::Managed {
                command: u32::MAX,
                status: i32::MIN,
                message: "plugin 오류\0detail".into(),
            },
            io_error(&path, std::io::Error::other("파일 오류")),
            Error::Speech(aftercast_tts::Error::Backend("음성 오류".into())),
            Error::Sound(aftercast_tts::Error::Backend("효과음 오류".into())),
            Error::WorkerFailed("worker 오류".into()),
            Error::StateInUse(path),
        ] {
            let kind = std::mem::discriminant(&error);
            let diagnostic = error.to_string();
            let mut bytes = Vec::new();
            write_metadata(&mut bytes, &Response::result(Err(error))).expect("error response");
            let mut input = bytes.as_slice();
            let response: Response = read_metadata(&mut input).expect("decode error response");
            let error = Error::from(response.error.expect("remote error"));
            assert_eq!(std::mem::discriminant(&error), kind);
            assert_eq!(error.to_string(), diagnostic);
            assert!(input.is_empty());
        }
    }

    #[test]
    fn audio_commands_validate_kind_and_enablement_before_dispatch() {
        let mut bytes = Vec::new();
        write_metadata(
            &mut bytes,
            &Message::ConfigureAudio(AudioKind::Sound, false),
        )
        .expect("audio metadata");
        write_frame(&mut bytes, &[]).expect("empty payload");
        let mut input = bytes.as_slice();
        assert!(matches!(
            read_metadata::<Message>(&mut input).expect("typed audio command"),
            Message::ConfigureAudio(AudioKind::Sound, false)
        ));
        assert!(read_frame(&mut input).expect("payload frame").is_empty());
        assert!(input.is_empty());

        for invalid in [
            &[2, 2, 1][..], // Unknown audio kind must not select a backend.
            &[2, 0, 2],     // Invalid boolean must not enable audio.
            &[2, 0],        // A truncated command must not reach dispatch.
            &[4],           // Unknown message variant must not reach dispatch.
            &[3, 0],        // A second value must not hide behind PollAudio.
        ] {
            bytes.clear();
            write_frame(&mut bytes, invalid).expect("invalid metadata frame");
            assert!(read_metadata::<Message>(&mut bytes.as_slice()).is_err());
        }
    }

    #[test]
    fn audio_cleanup_keeps_pending_work_and_reports_failure_once() {
        let mut polls = 0;
        let mut cleanup: Option<AudioCleanup> = Some(Box::pin(std::future::poll_fn(move |_| {
            polls += 1;
            if polls == 1 {
                Poll::Pending
            } else {
                Poll::Ready(Err(transport("cleanup failed")))
            }
        })));
        assert_eq!(poll_audio(&mut cleanup).expect("pending cleanup"), 1);
        assert!(cleanup.is_some());
        assert!(matches!(
            poll_audio(&mut cleanup),
            Err(Error::WorkerFailed(message)) if message.contains("cleanup failed")
        ));
        assert!(cleanup.is_none());
        assert_eq!(poll_audio(&mut cleanup).expect("retired cleanup"), 0);
    }

    #[test]
    fn ipc_frames_reject_oversize_and_incomplete_input() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, b"payload").expect("frame");
        for length in 0..bytes.len() {
            assert!(
                read_frame(&mut &bytes[..length]).is_err(),
                "truncated frame accepted"
            );
        }
        assert_eq!(
            read_frame(&mut bytes.as_slice()).expect("complete frame"),
            b"payload"
        );
        let oversized = ((MAX_FRAME + 1) as u32).to_le_bytes();
        read_frame(&mut oversized.as_slice()).expect_err("reject before allocating payload");
    }
}
