use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;

use bon::Builder;
use bytes::BytesMut;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::host::Control;
pub(crate) use crate::local_runtime::canonicalize;
use crate::{Error, process};

/// Trusted deployment paths and process lifetime limits. No executable is searched on PATH.
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
pub struct HostConfig {
    #[builder(into)]
    pub(crate) dotnet_root: Utf8PathBuf,
    #[builder(into)]
    pub(crate) bridge_directory: Utf8PathBuf,
    #[builder(into)]
    pub(crate) data_directory: Utf8PathBuf,
    /// Defaults to the current application executable. Override for an embedding test harness or launcher.
    #[builder(into)]
    pub(crate) application_executable: Option<Utf8PathBuf>,
    /// Includes worker launch, authentication and CLR initialization.
    #[builder(default = Duration::from_secs(30))]
    pub(crate) startup_timeout: Duration,
    /// Starts when shutdown is requested, even if the owner is inside a plugin callback.
    #[builder(default = Duration::from_secs(5))]
    pub shutdown_timeout: Duration,
}

#[repr(C)]
#[derive(Serialize, Deserialize)]
pub(crate) struct Packet {
    pub(crate) size: u32,
    pub(crate) version: u32,
    pub(crate) command: u32,
    pub(crate) runtime_major: u32,
    pub(crate) session: u64,
}
const _: () = assert!(size_of::<Packet>() == 24 && std::mem::offset_of!(Packet, session) == 16);

#[derive(Clone, Copy, Serialize, Deserialize)]
pub(crate) enum Request {
    ConfigureHost,
    CreatePlugin,
    PluginCommand,
    ImportOperation,
    Logs,
    Snapshot,
    Combat,
    Diagnostics,
    Speech,
    Archive,
}

// The local variant is constructed only in child mode; library clients always use IPC.
pub(crate) enum ManagedSession {
    Local(Box<crate::local_runtime::ManagedSession>),
    Remote(RefCell<process::RemoteSession>),
}
impl ManagedSession {
    pub(crate) fn start(config: HostConfig, control: Arc<Control>) -> Result<Self, Error> {
        process::RemoteSession::start(config, control)
            .map(|session| Self::Remote(RefCell::new(session)))
    }
    pub(crate) fn id(&self) -> u64 {
        match self {
            Self::Local(s) => s.id(),
            Self::Remote(s) => s.borrow().id(),
        }
    }
    pub(crate) fn request(&self, request: Request, packet: &mut BytesMut) -> Result<i32, Error> {
        match self {
            Self::Local(s) => s.request(request, packet),
            Self::Remote(s) => s.borrow_mut().request(request, packet),
        }
    }
    pub(crate) fn call(&self, command: u32) -> Result<Packet, Error> {
        match self {
            Self::Local(s) => s.call(command),
            Self::Remote(s) => s.borrow_mut().call(command),
        }
    }
    pub(crate) fn configure_audio(&self, command: u32, enabled: bool) -> Result<i32, Error> {
        match self {
            Self::Remote(s) => s.borrow_mut().audio(command, enabled),
            Self::Local(_) => Err(Error::WorkerFailed(
                "audio control requires the process boundary".into(),
            )),
        }
    }
    pub(crate) fn audio_error(&self) -> Option<Error> {
        match self {
            Self::Remote(s) => s.borrow_mut().audio_error(),
            Self::Local(_) => None,
        }
    }
    pub(crate) fn stop(self) -> Result<(), Error> {
        match self {
            Self::Local(s) => s.stop(),
            Self::Remote(s) => s.into_inner().stop(),
        }
    }
}
