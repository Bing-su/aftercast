//! In-process ACT-compatible hosting.
//!
//! Provides host/plugin lifecycles, synchronous imports, log batches and owned combat results.
//! Call [`extract_bridge`] once to deploy the bundled, prebuilt C# component, then use
//! [`Host::start`] with a trusted .NET 10 installation. No SDK or download is used at runtime.

mod bridge;
mod diagnostics;
mod host;
mod import;
mod logs;
mod plugins;
mod runtime;
mod snapshots;

pub use bridge::extract_bridge;
use camino::Utf8PathBuf;
pub use diagnostics::HostDiagnostics;
pub use host::Host;
pub use import::ImportReport;
pub use logs::{
    LogBatch, LogLoss, LogRecord, LogSubscription, LogSubscriptionConfig, LogTimeKind, LogTimestamp,
};
pub use plugins::{PluginConfig, PluginId};
pub use runtime::HostConfig;
pub use snapshots::{CombatRevision, CombatSnapshot, CombatSnapshotStatus, CombatantSnapshot};

/// Failures that callers can distinguish without parsing diagnostic messages.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("invalid combat snapshot request: {0}")]
    InvalidSnapshotRequest(String),
    #[error("invalid managed combat snapshot: {0}")]
    InvalidSnapshotResponse(String),
    #[error("invalid log subscription request: {0}")]
    InvalidLogRequest(String),
    #[error("invalid managed log batch: {0}")]
    InvalidLogResponse(String),
    #[error("invalid import request: {0}")]
    InvalidImportRequest(String),
    #[error("invalid managed import response: {0}")]
    InvalidImportResponse(String),
    #[error("invalid managed diagnostic response: {0}")]
    InvalidDiagnosticResponse(String),
    #[error("invalid plugin request: {0}")]
    InvalidPluginRequest(String),
    #[error("I/O operation failed for {path}: {source}")]
    Io {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("path contains an embedded NUL: {path}: {source}")]
    InvalidPath {
        path: Utf8PathBuf,
        #[source]
        source: netcorehost::pdcstring::ContainsNul,
    },
    #[error("no stable .NET 10 hostfxr was found under {0}")]
    RuntimeNotFound(Utf8PathBuf),
    #[error("cannot load hostfxr: {0}")]
    Hostfxr(#[from] netcorehost::dlopen2::Error),
    #[error(".NET hosting failed: {0}")]
    Hosting(#[from] netcorehost::error::HostingError),
    #[error("cannot load the managed entry point: {0}")]
    EntryPoint(#[from] netcorehost::hostfxr::GetManagedFunctionError),
    #[error("an ACT host is already active in this process")]
    AlreadyActive,
    #[error("managed bridge command {command} failed with status {status}: {message}")]
    Managed {
        command: u32,
        status: i32,
        message: String,
    },
    #[error("expected .NET 10, but the managed bridge reports major version {0}")]
    RuntimeMismatch(u32),
    #[error(
        "hostfxr is already loaded from {loaded}; cannot switch to {requested} in this process"
    )]
    RuntimeAlreadyLoaded {
        loaded: Utf8PathBuf,
        requested: Utf8PathBuf,
    },
}

fn io_error(path: impl Into<Utf8PathBuf>, source: std::io::Error) -> Error {
    Error::Io {
        path: path.into(),
        source,
    }
}
