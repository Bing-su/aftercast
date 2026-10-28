use std::fs;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use bytes::BytesMut;
use camino::{Utf8Path, Utf8PathBuf};
use netcorehost::hostfxr::{
    AssemblyDelegateLoader, Hostfxr, HostfxrContext, InitializedForRuntimeConfig,
    ManagedFunctionWithDefaultSignature,
};
use netcorehost::pdcstr;
use netcorehost::pdcstring::PdCString;
use scopeguard::{ScopeGuard, guard};

use crate::plugins::plugin_packet;
use crate::runtime::{HostConfig, Packet, Request};
use crate::{Error, io_error};

static ACTIVE: AtomicBool = AtomicBool::new(false);
// CoreCLR keeps hostpolicy state after a session closes; unloading hostfxr breaks reinitialization.
static HOSTFXR: OnceLock<(Utf8PathBuf, Hostfxr)> = OnceLock::new();

type ActiveLease = ScopeGuard<(), fn(())>;

fn acquire_lease() -> Result<ActiveLease, Error> {
    ACTIVE
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_observed_active| Error::AlreadyActive)?;
    Ok(guard((), |_| ACTIVE.store(false, Ordering::Release)))
}

fn host_option_flags(config: &HostConfig) -> u64 {
    u64::from(!config.record_encounter_logs)
        | (u64::from(config.original_logline_view_logs) << 1)
        | (u64::from(!config.idle_end_enabled) << 2)
        | (u64::from(config.idle_end_seconds) << 3)
        | (u64::from(!config.idle_timer_enabled) << 13)
        | (u64::from(config.silence_cut_seconds) << 14)
        | (u64::from(!config.main_table_commas) << 20)
}

// Own native delegates and thread affinity until managed shutdown has completed.
pub(crate) struct ManagedSession {
    entry: ManagedFunctionWithDefaultSignature,
    read_error: ManagedFunctionWithDefaultSignature,
    read_diagnostics: ManagedFunctionWithDefaultSignature,
    configure_host: ManagedFunctionWithDefaultSignature,
    create_plugin: ManagedFunctionWithDefaultSignature,
    plugin_command: ManagedFunctionWithDefaultSignature,
    import_operation: ManagedFunctionWithDefaultSignature,
    log_command: ManagedFunctionWithDefaultSignature,
    read_combat_snapshot: ManagedFunctionWithDefaultSignature,
    query_combat: ManagedFunctionWithDefaultSignature,
    speech_command: ManagedFunctionWithDefaultSignature,
    archive_command: ManagedFunctionWithDefaultSignature,
    trigger_command: ManagedFunctionWithDefaultSignature,
    notification_command: ManagedFunctionWithDefaultSignature,
    timer_command: ManagedFunctionWithDefaultSignature,
    history_operation: ManagedFunctionWithDefaultSignature,
    table_command: ManagedFunctionWithDefaultSignature,
    companion_command: ManagedFunctionWithDefaultSignature,
    text_report_command: ManagedFunctionWithDefaultSignature,
    table_report_command: ManagedFunctionWithDefaultSignature,
    session: u64,
    running: bool,
    // Keep the native library and delegate loader alive for every managed function call.
    _loader: AssemblyDelegateLoader,
    _context: HostfxrContext<InitializedForRuntimeConfig>,
    _lease: ActiveLease,
    _state_lock: fs::File,
    _thread: PhantomData<Rc<()>>,
}

impl ManagedSession {
    pub(crate) fn start(config: HostConfig) -> Result<Self, Error> {
        let lease = acquire_lease()?;
        fs::create_dir_all(&config.data_directory)
            .map_err(|error| io_error(&config.data_directory, error))?;
        let lock_path = config.data_directory.join(".aftercast.lock");
        let state_lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|error| io_error(&lock_path, error))?;
        state_lock.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => Error::StateInUse(config.data_directory.clone()),
            fs::TryLockError::Error(source) => io_error(&lock_path, source),
        })?;
        let root = canonicalize(&config.dotnet_root)?;
        let bridge = canonicalize(&config.bridge_directory)?;
        let fxr = find_hostfxr(&root)?;
        let hostfxr = process_hostfxr(fxr)?;
        let context = hostfxr.initialize_for_runtime_config_with_dotnet_root(
            path_string(&bridge.join("Aftercast.Bridge.runtimeconfig.json"))?,
            path_string(&root)?,
        )?;
        let loader = context
            .get_delegate_loader_for_assembly(path_string(&bridge.join("Aftercast.Bridge.dll"))?)?;
        let function = |name| {
            loader.get_function_with_default_signature(
                pdcstr!("Aftercast.Bridge.EntryPoint, Aftercast.Bridge"),
                name,
            )
        };
        let entry = function(pdcstr!("Invoke"))?;
        let read_error = function(pdcstr!("ReadError"))?;
        let create_plugin = function(pdcstr!("CreatePlugin"))?;
        let read_diagnostics = function(pdcstr!("ReadDiagnostics"))?;
        let configure_host = function(pdcstr!("ConfigureHost"))?;
        let plugin_command = function(pdcstr!("PluginCommand"))?;
        let import_operation = function(pdcstr!("ImportOperation"))?;
        let log_command = function(pdcstr!("LogCommand"))?;
        let read_combat_snapshot = function(pdcstr!("ReadCombatSnapshot"))?;
        let query_combat = function(pdcstr!("QueryCombat"))?;
        let speech_command = function(pdcstr!("SpeechCommand"))?;
        let archive_command = function(pdcstr!("ArchiveOperation"))?;
        let trigger_command = function(pdcstr!("TriggerCommand"))?;
        let notification_command = function(pdcstr!("NotificationCommand"))?;
        let timer_command = function(pdcstr!("TimerCommand"))?;
        let history_operation = function(pdcstr!("HistoryOperation"))?;
        let table_command = function(pdcstr!("TableCommand"))?;
        let companion_command = function(pdcstr!("CompanionCommand"))?;
        let text_report_command = function(pdcstr!("TextReportCommand"))?;
        let table_report_command = function(pdcstr!("TableReportCommand"))?;
        let mut host = Self {
            entry,
            read_error,
            read_diagnostics,
            configure_host,
            create_plugin,
            plugin_command,
            import_operation,
            log_command,
            read_combat_snapshot,
            query_combat,
            speech_command,
            archive_command,
            trigger_command,
            notification_command,
            timer_command,
            history_operation,
            table_command,
            companion_command,
            text_report_command,
            table_report_command,
            session: 0,
            running: false,
            _loader: loader,
            _context: context,
            _lease: lease,
            _state_lock: state_lock,
            _thread: PhantomData,
        };
        let packet = host.call(1)?;
        host.session = packet.session;
        host.running = true;
        if packet.runtime_major != 10 {
            return Err(Error::RuntimeMismatch(packet.runtime_major));
        }
        let data_directory = canonicalize(&config.data_directory)?;
        let selective = config.selective.wire()?;
        let mut request = plugin_packet(host.session, data_directory.as_str(), Some(&selective))?;
        let recording_flags = host_option_flags(&config);
        request
            .get_mut(32..40)
            .ok_or_else(|| Error::InvalidPluginRequest("missing host recording flags".into()))?
            .copy_from_slice(&recording_flags.to_le_bytes());
        host.request(Request::ConfigureHost, &mut request)?;
        tracing::debug!(session = host.session, "managed host started");
        Ok(host)
    }

    pub(crate) fn id(&self) -> u64 {
        self.session
    }

    pub(crate) fn request(&self, request: Request, packet: &mut BytesMut) -> Result<i32, Error> {
        let (function, command) = match request {
            Request::ConfigureHost => (&self.configure_host, 5),
            Request::CreatePlugin => (&self.create_plugin, 4),
            Request::PluginCommand => (&self.plugin_command, 6),
            Request::Diagnostics => (&self.read_diagnostics, 7),
            Request::ImportOperation => (&self.import_operation, 8),
            Request::Logs => (&self.log_command, 9),
            Request::Snapshot => (&self.read_combat_snapshot, 10),
            Request::Combat => (&self.query_combat, 12),
            Request::Speech => (&self.speech_command, 11),
            Request::Archive => (&self.archive_command, 13),
            Request::Triggers => (&self.trigger_command, 14),
            Request::Notifications => (&self.notification_command, 15),
            Request::Timers => (&self.timer_command, 16),
            Request::History => (&self.history_operation, 17),
            Request::Tables => (&self.table_command, 18),
            Request::Companions => (&self.companion_command, 19),
            Request::Reports => (&self.text_report_command, 20),
            Request::TableReports => (&self.table_report_command, 21),
        };
        let length = i32::try_from(packet.len())
            .map_err(|error| Error::InvalidPluginRequest(error.to_string()))?;
        // SAFETY: Private encoders supply initialized, bounded packets. Delegates validate every
        // range, write within capacity and retain no pointer; this session owns all loader handles.
        let status = unsafe { function(packet.as_mut_ptr().cast(), length) };
        let failed = match request {
            Request::ImportOperation => !(0..=2).contains(&status),
            Request::Logs => status < 0,
            Request::Snapshot | Request::Combat | Request::Archive | Request::History => {
                status != 0 && status != 1
            }
            _ => status != 0,
        };
        if failed {
            return Err(Error::Managed {
                command,
                status,
                message: self.error_message(),
            });
        }
        Ok(status)
    }

    pub(crate) fn stop(mut self) -> Result<(), Error> {
        self.stop_inner()
    }

    fn stop_inner(&mut self) -> Result<(), Error> {
        if self.running {
            // Managed stop closes the session even when a plugin cleanup callback throws.
            self.running = false;
            self.call(3)?;
            tracing::debug!(session = self.session, "managed host stopped");
        }
        Ok(())
    }

    pub(crate) fn call(&self, command: u32) -> Result<Packet, Error> {
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
        let mut output = [0_u8; 8192];
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

impl Drop for ManagedSession {
    fn drop(&mut self) {
        if let Err(error) = self.stop_inner() {
            tracing::error!(%error, "managed host shutdown failed");
        }
    }
}

pub(crate) fn canonicalize(path: &Utf8Path) -> Result<Utf8PathBuf, Error> {
    // UTF-8 symlink names can resolve to non-UTF-8 targets; reject these without lossy conversion.
    let path = path.canonicalize_utf8().map_err(|e| io_error(path, e))?;
    // Legacy subprocess loaders (e.g. CefSharp's CLR 4 helper) cannot resolve assemblies under \\?\ paths.
    #[cfg(windows)]
    let path = Utf8Path::from_path(dunce::simplified(path.as_std_path()))
        .unwrap_or(&path)
        .to_owned();
    Ok(path)
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
    use super::ManagedSession as Host;
    use super::*;

    #[cfg(windows)]
    #[test]
    fn canonical_paths_remain_usable_by_legacy_subprocess_loaders() {
        let temporary = tempfile::tempdir().expect("temporary deployment");
        let path = Utf8Path::from_path(temporary.path())
            .expect("UTF-8 directory")
            .join("subprocess 한글.exe");
        fs::write(&path, b"fixture").expect("subprocess fixture");
        let canonical = canonicalize(&path).expect("canonical deployment path");
        assert!(!canonical.as_str().starts_with(r"\\?\"));
        assert_eq!(
            canonical.canonicalize_utf8().expect("same file"),
            path.canonicalize_utf8().expect("original file")
        );
        // A trailing dot changes Win32 semantics; retain its extended path rather than target another file.
        let special = path
            .canonicalize_utf8()
            .expect("extended path")
            .with_file_name("trailing.");
        fs::write(&special, b"special").expect("extended-only filename");
        assert!(
            canonicalize(&special)
                .expect("preserved path")
                .as_str()
                .starts_with(r"\\?\")
        );
    }

    #[rstest::rstest]
    #[case::stable("10.0.13", Some([10, 0, 13]))]
    #[case::older("9.0.13", None)]
    #[case::newer("11.0.0", None)]
    #[case::preview("10.0.0-preview.1", None)]
    #[case::missing_patch("10.0", None)]
    #[case::extra_part("10.0.13.1", None)]
    fn only_stable_net10_host_versions_are_candidates(
        #[case] name: &str,
        #[case] expected: Option<[u32; 3]>,
    ) {
        assert_eq!(stable_net10_version(name), expected);
    }

    #[rstest::rstest]
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

    #[rstest::rstest]
    #[cfg(unix)]
    fn canonicalization_rejects_non_utf8_symlink_targets_without_losing_the_request() {
        use std::os::unix::ffi::OsStringExt;
        use std::os::unix::fs::symlink;
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

    #[rstest::rstest]
    fn utf8_paths_still_reject_nul_at_the_native_boundary() {
        assert!(matches!(
            path_string(Utf8Path::new("invalid\0path")),
            Err(Error::InvalidPath { .. })
        ));
    }

    #[rstest::rstest]
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
                .data_directory(directory.join("state"))
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
            host.call(2)
                .map(|packet| packet.session)
                .expect("command failure must preserve the session"),
            0
        );
        host.stop().expect("stop managed component");
    }
}
