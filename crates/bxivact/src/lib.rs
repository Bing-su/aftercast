//! In-process ACT-compatible hosting.
//!
//! Provides bridge lifecycle and an initial ACT contract slice, not plugin execution or parsing.
//! Call [`extract_bridge`] once to deploy the bundled, prebuilt F# component, then use
//! [`Host::start`] with a trusted .NET 10 installation. No SDK or download is used at runtime.

mod bridge;
mod runtime;

pub use bridge::extract_bridge;
pub use runtime::{Host, HostConfig};

/// Failures that callers can distinguish without parsing diagnostic messages.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("I/O operation failed for {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("path contains an embedded NUL: {path}: {source}")]
    InvalidPath {
        path: std::path::PathBuf,
        #[source]
        source: netcorehost::pdcstring::ContainsNul,
    },
    #[error("no stable .NET 10 hostfxr was found under {0}")]
    RuntimeNotFound(std::path::PathBuf),
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
        loaded: std::path::PathBuf,
        requested: std::path::PathBuf,
    },
}

fn io_error(path: impl Into<std::path::PathBuf>, source: std::io::Error) -> Error {
    Error::Io {
        path: path.into(),
        source,
    }
}
