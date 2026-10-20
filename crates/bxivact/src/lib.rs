//! In-process ACT-compatible hosting.
//!
//! Provides host/plugin lifecycles and an initial ACT contract slice.
//! Call [`extract_bridge`] once to deploy the bundled, prebuilt F# component, then use
//! [`Host::start`] with a trusted .NET 10 installation. No SDK or download is used at runtime.

mod bridge;
mod runtime;

use camino::Utf8PathBuf;

pub use bridge::extract_bridge;
pub use runtime::{Host, HostConfig, PluginConfig, PluginId};

/// Failures that callers can distinguish without parsing diagnostic messages.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
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
