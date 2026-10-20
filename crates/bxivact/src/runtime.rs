use std::{
    fs,
    marker::PhantomData,
    rc::Rc,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use bon::Builder;
use bytes::{BufMut, BytesMut};
use camino::{Utf8Path, Utf8PathBuf};
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
static HOSTFXR: OnceLock<(Utf8PathBuf, Hostfxr)> = OnceLock::new();

/// UTF-8 paths are explicit so a library never silently selects or downloads a runtime.
#[derive(Debug, Builder)]
pub struct HostConfig {
    /// Trusted .NET installation containing `host/fxr/10.*` and the .NET 10 shared runtime.
    #[builder(into)]
    dotnet_root: Utf8PathBuf,
    /// Trusted directory previously prepared with [`crate::extract_bridge`].
    #[builder(into)]
    bridge_directory: Utf8PathBuf,
}

/// Constructor-only loading of a trusted ACT plugin; no initialization or parsing occurs.
#[derive(Debug, Builder)]
pub struct PluginConfig {
    /// Plugin assembly path; dependencies are resolved from its deployment directory.
    #[builder(into)]
    path: Utf8PathBuf,
    /// Fully qualified type name. Omit only when exactly one eligible plugin type exists.
    #[builder(into)]
    type_name: Option<String>,
}

/// Session-scoped identity of a constructed instance, not an initialized plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PluginId {
    session: u64,
    instance: u64,
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
    read_error: ManagedFunctionWithDefaultSignature,
    create_plugin: ManagedFunctionWithDefaultSignature,
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
        let read_error = loader.get_function_with_default_signature(
            pdcstr!("Bxivact.Bridge.EntryPoint, Bxivact.Bridge"),
            pdcstr!("ReadError"),
        )?;
        let create_plugin = loader.get_function_with_default_signature(
            pdcstr!("Bxivact.Bridge.EntryPoint, Bxivact.Bridge"),
            pdcstr!("CreatePlugin"),
        )?;
        let mut host = Self {
            entry,
            read_error,
            create_plugin,
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

    /// Constructs a trusted plugin and retains it until this host stops.
    ///
    /// Constructors execute arbitrary code in this process and may register global handlers.
    /// No sandbox or assembly unloading is provided. `InitPlugin` and `DeInitPlugin` are not
    /// called. On Linux only signature binding for `TabPage`/`Label` is available; creating UI
    /// controls fails explicitly. For example, select a type with
    /// `PluginConfig::builder().path("plugin.dll").type_name("Example.Plugin").build()`.
    pub fn create_plugin(&mut self, config: PluginConfig) -> Result<PluginId, Error> {
        let path = canonicalize(&config.path)?;
        let mut packet = plugin_packet(self.session, path.as_str(), config.type_name.as_deref())?;
        let length = i32::try_from(packet.len())
            .map_err(|error| Error::InvalidPluginRequest(error.to_string()))?;
        // SAFETY: CreatePlugin uses ComponentEntryPoint, borrows this initialized buffer only
        // during the call, checks all ranges, and writes its result within the 40-byte header.
        let status = unsafe { (self.create_plugin)(packet.as_mut_ptr().cast(), length) };
        if status != 0 {
            return Err(Error::Managed {
                command: 4,
                status,
                message: self.error_message(),
            });
        }
        let bytes: [u8; 8] = packet
            .get(32..40)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| Error::InvalidPluginRequest("missing plugin result".to_owned()))?;
        let instance = u64::from_le_bytes(bytes);
        if instance == 0 {
            return Err(Error::InvalidPluginRequest("zero plugin result".to_owned()));
        }
        Ok(PluginId {
            session: self.session,
            instance,
        })
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
            return Err(Error::Managed {
                command,
                status,
                message: self.error_message(),
            });
        }
        Ok(packet)
    }

    fn error_message(&self) -> String {
        let mut output = BytesMut::zeroed(8192);
        // SAFETY: ReadError uses ComponentEntryPoint, writes only within capacity, and retains
        // no pointer. All 8192 output bytes are initialized and exclusively borrowed for the call.
        let length = unsafe { (self.read_error)(output.as_mut_ptr().cast(), 8192) };
        let Ok(length) = usize::try_from(length) else {
            return "Managed diagnostic retrieval failed".to_owned();
        };
        let Some(bytes) = output.get(..length) else {
            return format!("Managed diagnostic length exceeds capacity: {length}");
        };
        match std::str::from_utf8(bytes) {
            Ok(text) if !text.is_empty() => text.to_owned(),
            Ok(_) => "Managed bridge returned no diagnostic text".to_owned(),
            Err(error) => format!("Invalid UTF-8 in managed diagnostics: {error}"),
        }
    }
}

fn plugin_packet(session: u64, path: &str, type_name: Option<&str>) -> Result<BytesMut, Error> {
    let type_name = type_name.unwrap_or_default();
    if path.contains('\0') || type_name.contains('\0') || type_name.len() > 4096 {
        return Err(Error::InvalidPluginRequest(
            "NUL or oversized type name".to_owned(),
        ));
    }
    let length = 40_usize
        .checked_add(path.len())
        .and_then(|size| size.checked_add(type_name.len()));
    let size = length
        .filter(|size| *size <= 65536)
        .ok_or_else(|| Error::InvalidPluginRequest("request exceeds 65536 bytes".to_owned()))?;
    let mut packet = BytesMut::with_capacity(size);
    // These lengths are bounded by the 64 KiB ABI limit before conversion.
    packet.put_u32_le(size as u32);
    packet.put_u32_le(1);
    packet.put_u64_le(session);
    packet.put_u32_le(40);
    packet.put_u32_le(path.len() as u32);
    packet.put_u32_le(40 + path.len() as u32);
    packet.put_u32_le(type_name.len() as u32);
    packet.put_u64_le(0);
    packet.extend_from_slice(path.as_bytes());
    packet.extend_from_slice(type_name.as_bytes());
    Ok(packet)
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Err(error) = self.stop_inner() {
            tracing::error!(%error, "managed host shutdown failed");
        }
    }
}

fn canonicalize(path: &Utf8Path) -> Result<Utf8PathBuf, Error> {
    // UTF-8 symlink names can resolve to non-UTF-8 targets; reject these without lossy conversion.
    path.canonicalize_utf8().map_err(|e| io_error(path, e))
}

fn process_hostfxr(path: Utf8PathBuf) -> Result<Hostfxr, Error> {
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

fn path_string(path: &Utf8Path) -> Result<PdCString, Error> {
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

fn find_hostfxr(root: &Utf8Path) -> Result<Utf8PathBuf, Error> {
    #[cfg(windows)]
    const LIBRARY: &str = "hostfxr.dll";
    #[cfg(not(windows))]
    const LIBRARY: &str = "libhostfxr.so";

    let directory = root.join("host/fxr");
    let entries = fs::read_dir(&directory).map_err(|e| io_error(&directory, e))?;
    let mut selected = None;
    for entry in entries {
        let entry = entry.map_err(|e| io_error(&directory, e))?;
        let name = entry.file_name();
        // Unrelated non-UTF-8 entries cannot be runtime versions and must not block discovery.
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(version) = stable_net10_version(name) else {
            continue;
        };
        let library = directory.join(name).join(LIBRARY);
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
    fn plugin_requests_bound_lengths_and_preserve_utf8() {
        let packet =
            plugin_packet(9, "/플러그인/測試.dll", Some("Example.Plugin")).expect("encode request");
        assert_eq!(packet.get(8..16), Some(9_u64.to_le_bytes().as_slice()));
        assert_eq!(
            packet.get(40..),
            Some("/플러그인/測試.dllExample.Plugin".as_bytes())
        );
        for (path, name) in [
            ("/bad\0path", "Plugin".to_owned()),
            ("/plugin", "bad\0type".to_owned()),
            ("/plugin", "x".repeat(4097)),
        ] {
            assert!(matches!(
                plugin_packet(1, path, Some(&name)),
                Err(Error::InvalidPluginRequest(_))
            ));
        }
        assert_eq!(
            plugin_packet(1, &"x".repeat(65496), None)
                .expect("maximum request")
                .len(),
            65536
        );
        assert!(matches!(
            plugin_packet(1, &"x".repeat(65497), None),
            Err(Error::InvalidPluginRequest(_))
        ));
    }

    #[test]
    fn only_stable_net10_host_versions_are_candidates() {
        assert_eq!(stable_net10_version("10.0.13"), Some([10, 0, 13]));
        for name in ["9.0.13", "11.0.0", "10.0.0-preview.1", "10.0", "10.0.13.1"] {
            assert_eq!(stable_net10_version(name), None);
        }
    }

    #[test]
    fn runtime_discovery_preserves_utf8_paths_and_ignores_unrelated_entries() {
        let temporary = tempfile::tempdir().expect("create runtime discovery fixture");
        let root = Utf8Path::from_path(temporary.path())
            .expect("UTF-8 test directory")
            .join("runtime 日本語");
        let directory = root.join("host/fxr");
        let name = if cfg!(windows) {
            "hostfxr.dll"
        } else {
            "libhostfxr.so"
        };
        for version in ["10.0.1", "10.0.13", "9.0.99", "10.0.20-preview.1"] {
            let path = directory.join(version);
            fs::create_dir_all(&path).expect("create runtime version");
            fs::write(path.join(name), []).expect("create discovery marker");
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let unrelated = std::ffi::OsString::from_vec(vec![0xff]);
            fs::create_dir(directory.as_std_path().join(unrelated))
                .expect("create non-UTF-8 unrelated entry");
        }
        assert_eq!(
            find_hostfxr(&root).expect("select stable runtime"),
            directory.join("10.0.13").join(name)
        );
    }

    #[test]
    #[cfg(unix)]
    fn canonicalization_rejects_non_utf8_symlink_targets_without_losing_the_request() {
        use std::os::unix::{ffi::OsStringExt, fs::symlink};
        let temporary = tempfile::tempdir().expect("create symlink fixture");
        let target = temporary
            .path()
            .join(std::ffi::OsString::from_vec(vec![0xff]));
        fs::write(&target, []).expect("create non-UTF-8 target");
        let link = Utf8Path::from_path(temporary.path())
            .expect("UTF-8 test directory")
            .join("link 한글");
        symlink(&target, &link).expect("create UTF-8 symlink");
        match canonicalize(&link) {
            Err(Error::Io { path, source }) => {
                assert_eq!(path, link);
                assert_eq!(source.kind(), std::io::ErrorKind::InvalidData);
            }
            other => panic!("expected a non-UTF-8 canonical target error, received {other:?}"),
        }
    }

    #[test]
    fn utf8_paths_still_reject_nul_at_the_native_boundary() {
        assert!(matches!(
            path_string(Utf8Path::new("invalid\0path")),
            Err(Error::InvalidPath { .. })
        ));
    }

    #[test]
    #[ignore = "requires .NET 10; set DOTNET_ROOT and run mise run test:hosting"]
    fn managed_errors_keep_status_and_diagnostic_text() {
        let root =
            std::env::var("DOTNET_ROOT").expect("set a UTF-8 DOTNET_ROOT for managed testing");
        let temporary = tempfile::tempdir().expect("create private test fixture");
        let directory = Utf8Path::from_path(temporary.path())
            .expect("UTF-8 test directory")
            .join("bridge");
        crate::extract_bridge(&directory).expect("extract managed component");
        let host = Host::start(
            HostConfig::builder()
                .dotnet_root(root)
                .bridge_directory(directory)
                .build(),
        )
        .expect("start managed component");
        match host.call(99) {
            Err(Error::Managed {
                command,
                status,
                message,
            }) => {
                assert_eq!(command, 99);
                assert_eq!(status, 6);
                assert_eq!(message, "Unknown bridge command: 99");
            }
            _ => panic!("unknown command must return its managed diagnostic"),
        }
        assert_ne!(
            host.session_id()
                .expect("command failure must preserve the session"),
            0
        );
        host.stop().expect("stop managed component");
    }
}
