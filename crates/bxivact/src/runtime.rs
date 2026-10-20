use std::{
    fs,
    marker::PhantomData,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use bon::Builder;
use netcorehost::{
    hostfxr::{
        AssemblyDelegateLoader, Hostfxr, HostfxrContext, InitializedForRuntimeConfig,
        ManagedFunctionWithDefaultSignature,
    },
    pdcstr,
    pdcstring::PdCString,
};
use scopeguard::{ScopeGuard, guard};

use crate::{Error, io_error};

static ACTIVE: AtomicBool = AtomicBool::new(false);
// CoreCLR keeps hostpolicy state after a session closes; unloading hostfxr breaks reinitialization.
static HOSTFXR: OnceLock<(PathBuf, Hostfxr)> = OnceLock::new();

/// Paths are explicit so a library never silently selects or downloads a runtime.
#[derive(Debug, Builder)]
pub struct HostConfig {
    /// Trusted .NET installation containing `host/fxr/10.*` and the .NET 10 shared runtime.
    #[builder(into)]
    dotnet_root: PathBuf,
    /// Trusted directory previously prepared with [`crate::extract_bridge`].
    #[builder(into)]
    bridge_directory: PathBuf,
}

#[repr(C)]
struct Packet {
    size: u32,
    version: u32,
    command: u32,
    runtime_major: u32,
    session: u64,
}

const _: () = assert!(size_of::<Packet>() == 24 && std::mem::offset_of!(Packet, session) == 16);

type ActiveLease = ScopeGuard<(), fn(())>;

fn acquire_lease() -> Result<ActiveLease, Error> {
    ACTIVE
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_observed_active| Error::AlreadyActive)?;
    Ok(guard((), |_| ACTIVE.store(false, Ordering::Release)))
}

/// A single active bridge lifetime on the calling thread.
///
/// This type is intentionally neither Send nor Sync: future plugin UI work must not silently
/// migrate between threads. Stopping releases the ACT session, not the process-wide CLR.
pub struct Host {
    entry: ManagedFunctionWithDefaultSignature,
    session: u64,
    running: bool,
    // Keep the native library and delegate loader alive for every managed function call.
    _loader: AssemblyDelegateLoader,
    _context: HostfxrContext<InitializedForRuntimeConfig>,
    _lease: ActiveLease,
    _thread: PhantomData<Rc<()>>,
}

impl Host {
    /// Loads the installed runtime and starts the bundled managed session.
    ///
    /// Both paths must contain trusted application/runtime files. An incompatible CLR already
    /// loaded by another host is reported as a hosting error; no fallback runtime is started.
    pub fn start(config: HostConfig) -> Result<Self, Error> {
        let lease = acquire_lease()?;
        let root = canonicalize(&config.dotnet_root)?;
        let bridge = canonicalize(&config.bridge_directory)?;
        let fxr = find_hostfxr(&root)?;
        let hostfxr = process_hostfxr(fxr)?;
        let context = hostfxr.initialize_for_runtime_config_with_dotnet_root(
            path_string(&bridge.join("Bxivact.Bridge.runtimeconfig.json"))?,
            path_string(&root)?,
        )?;
        let loader = context
            .get_delegate_loader_for_assembly(path_string(&bridge.join("Bxivact.Bridge.dll"))?)?;
        let entry = loader.get_function_with_default_signature(
            pdcstr!("Bxivact.Bridge.EntryPoint, Bxivact.Bridge"),
            pdcstr!("Invoke"),
        )?;
        let mut host = Self {
            entry,
            session: 0,
            running: false,
            _loader: loader,
            _context: context,
            _lease: lease,
            _thread: PhantomData,
        };
        let packet = host.call(1)?;
        host.session = packet.session;
        host.running = true;
        if packet.runtime_major != 10 {
            return Err(Error::RuntimeMismatch(packet.runtime_major));
        }
        tracing::debug!(session = host.session, "managed host started");
        Ok(host)
    }

    /// Confirms that this session still owns the managed host and returns its identity.
    pub fn session_id(&self) -> Result<u64, Error> {
        self.call(2).map(|packet| packet.session)
    }

    /// Stops the managed session. The CLR and loaded assemblies may remain in the process.
    pub fn stop(mut self) -> Result<(), Error> {
        self.stop_inner()
    }

    fn stop_inner(&mut self) -> Result<(), Error> {
        if self.running {
            self.call(3)?;
            self.running = false;
            tracing::debug!(session = self.session, "managed host stopped");
        }
        Ok(())
    }

    fn call(&self, command: u32) -> Result<Packet, Error> {
        let mut packet = Packet {
            size: 24,
            version: 1,
            command,
            runtime_major: 0,
            session: self.session,
        };
        // SAFETY: Invoke has the standard ComponentEntryPoint signature. The aligned packet
        // is initialized, exclusively borrowed for the call, and never retained by the bridge.
        let status = unsafe { (self.entry)((&raw mut packet).cast(), 24) };
        if status != 0 {
            return Err(Error::Managed { command, status });
        }
        Ok(packet)
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Err(error) = self.stop_inner() {
            tracing::error!(%error, "managed host shutdown failed");
        }
    }
}

fn canonicalize(path: &Path) -> Result<PathBuf, Error> {
    fs::canonicalize(path).map_err(|e| io_error(path, e))
}

fn process_hostfxr(path: PathBuf) -> Result<Hostfxr, Error> {
    if let Some((loaded, library)) = HOSTFXR.get() {
        if *loaded != path {
            return Err(Error::RuntimeAlreadyLoaded {
                loaded: loaded.clone(),
                requested: path,
            });
        }
        return Ok(library.clone());
    }
    let library = Hostfxr::load_from_path(&path)?;
    // ActiveLease serializes initialization, so this is the only writer in this crate instance.
    HOSTFXR
        .set((path, library.clone()))
        .map_err(|_existing| Error::AlreadyActive)?;
    Ok(library)
}

fn path_string(path: &Path) -> Result<PdCString, Error> {
    PdCString::from_os_str(path.as_os_str()).map_err(|source| Error::InvalidPath {
        path: path.to_owned(),
        source,
    })
}

fn stable_net10_version(name: &str) -> Option<[u32; 3]> {
    let mut parts = name.split('.');
    let version = [
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ];
    if parts.next().is_some() {
        return None;
    }
    (version[0] == 10).then_some(version)
}

fn find_hostfxr(root: &Path) -> Result<PathBuf, Error> {
    #[cfg(windows)]
    const LIBRARY: &str = "hostfxr.dll";
    #[cfg(not(windows))]
    const LIBRARY: &str = "libhostfxr.so";

    let directory = root.join("host/fxr");
    let entries = fs::read_dir(&directory).map_err(|e| io_error(&directory, e))?;
    let mut selected = None;
    for entry in entries {
        let entry = entry.map_err(|e| io_error(&directory, e))?;
        let Some(version) = entry.file_name().to_str().and_then(stable_net10_version) else {
            continue;
        };
        let library = entry.path().join(LIBRARY);
        if library.is_file()
            && selected
                .as_ref()
                .is_none_or(|(current, _)| version > *current)
        {
            selected = Some((version, library));
        }
    }
    selected
        .map(|(_, path)| path)
        .ok_or_else(|| Error::RuntimeNotFound(root.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_stable_net10_host_versions_are_candidates() {
        assert_eq!(stable_net10_version("10.0.13"), Some([10, 0, 13]));
        for name in ["9.0.13", "11.0.0", "10.0.0-preview.1", "10.0", "10.0.13.1"] {
            assert_eq!(stable_net10_version(name), None);
        }
    }
}
