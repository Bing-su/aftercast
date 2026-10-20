use camino::Utf8Path;
use scopeguard::{ScopeGuard, guard};
use std::fs;

use crate::{Error, io_error};

const FILES: &[(&str, &[u8])] = &[
    (
        "FSharp.Core.LICENSE",
        include_bytes!("../managed/FSharp.Core.LICENSE"),
    ),
    (
        "Bxivact.Bridge.dll",
        include_bytes!("../managed/Bxivact.Bridge.dll"),
    ),
    (
        "Bxivact.Bridge.deps.json",
        include_bytes!("../managed/Bxivact.Bridge.deps.json"),
    ),
    (
        "Bxivact.Bridge.runtimeconfig.json",
        #[cfg(not(windows))]
        include_bytes!("../managed/Bxivact.Bridge.runtimeconfig.json"),
        #[cfg(windows)]
        include_bytes!("../managed/Bxivact.Bridge.runtimeconfig.windows.json"),
    ),
    #[cfg(target_os = "linux")]
    ("System.Windows.Forms.dll", bxivact_winforms::ASSEMBLY),
    (
        "Advanced Combat Tracker.dll",
        include_bytes!("../managed/Advanced Combat Tracker.dll"),
    ),
    (
        "FSharp.Core.dll",
        include_bytes!("../managed/FSharp.Core.dll"),
    ),
];

/// Deploys the bundled bridge into a **new** directory without overwriting existing files.
///
/// The parent directory must already exist and be controlled by the application. For example,
/// deploy to a versioned subdirectory of the application's private data directory. Keep the
/// files available while the process uses .NET; stopping a host does not unload the CLR.
/// No compiler, SDK, or network access is needed.
pub fn extract_bridge(directory: impl AsRef<Utf8Path>) -> Result<(), Error> {
    let directory = directory.as_ref();
    let builder = fs::DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt;
        let mut private = builder;
        private.mode(0o700);
        private
    };
    builder
        .create(directory)
        .map_err(|e| io_error(directory, e))?;
    // Roll back this new directory on any early return, including unwinding during extraction.
    let rollback = guard((), |()| {
        if let Err(error) = fs::remove_dir_all(directory) {
            tracing::warn!(%error, path = %directory, "bridge extraction rollback failed");
        }
    });
    FILES.iter().try_for_each(|(name, bytes)| {
        let path = directory.join(name);
        fs::write(&path, bytes).map_err(|e| io_error(path, e))
    })?;
    ScopeGuard::into_inner(rollback);
    Ok(())
}
