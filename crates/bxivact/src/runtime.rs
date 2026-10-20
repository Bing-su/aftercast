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

use crate::{
    CombatRevision, CombatSnapshot, Error, HostDiagnostics, ImportReport, LogBatch,
    LogSubscription, LogSubscriptionConfig,
    diagnostics::{DIAGNOSTIC_CAPACITY, decode_diagnostics},
    import::{decode_import, import_packet},
    io_error,
    logs::{
        LOG_HEADER, decode_logs, larger_log_capacity, log_packet, subscribe_packet,
        subscription_result,
    },
    snapshots::{decode_snapshot, larger_snapshot_capacity, snapshot_packet},
};

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
    /// Private writable ACT state directory, separate from the deployed managed binaries.
    #[builder(into)]
    data_directory: Utf8PathBuf,
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
    // Lifecycle commands return the runtime major; command 4 returns the number of callbacks run.
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
    read_diagnostics: ManagedFunctionWithDefaultSignature,
    create_plugin: ManagedFunctionWithDefaultSignature,
    plugin_command: ManagedFunctionWithDefaultSignature,
    import_log: ManagedFunctionWithDefaultSignature,
    log_command: ManagedFunctionWithDefaultSignature,
    read_combat_snapshot: ManagedFunctionWithDefaultSignature,
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
    /// Runtime and bridge paths must contain trusted files. The application-controlled data
    /// directory is created without overwriting existing contents. An incompatible CLR already
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
        let function = |name| {
            loader.get_function_with_default_signature(
                pdcstr!("Bxivact.Bridge.EntryPoint, Bxivact.Bridge"),
                name,
            )
        };
        let entry = function(pdcstr!("Invoke"))?;
        let read_error = function(pdcstr!("ReadError"))?;
        let create_plugin = function(pdcstr!("CreatePlugin"))?;
        let read_diagnostics = function(pdcstr!("ReadDiagnostics"))?;
        let configure_host = function(pdcstr!("ConfigureHost"))?;
        let plugin_command = function(pdcstr!("PluginCommand"))?;
        let import_log = function(pdcstr!("ImportLog"))?;
        let log_command = function(pdcstr!("LogCommand"))?;
        let read_combat_snapshot = function(pdcstr!("ReadCombatSnapshot"))?;
        let mut host = Self {
            entry,
            read_error,
            read_diagnostics,
            create_plugin,
            plugin_command,
            import_log,
            log_command,
            read_combat_snapshot,
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
        fs::create_dir_all(&config.data_directory)
            .map_err(|error| io_error(&config.data_directory, error))?;
        let data_directory = canonicalize(&config.data_directory)?;
        let mut request = plugin_packet(host.session, data_directory.as_str(), None)?;
        let length = i32::try_from(request.len())
            .map_err(|error| Error::InvalidPluginRequest(error.to_string()))?;
        // SAFETY: ConfigureHost borrows this bounded, initialized request for the call only.
        let status = unsafe { (configure_host)(request.as_mut_ptr().cast(), length) };
        if status != 0 {
            return Err(Error::Managed {
                command: 5,
                status,
                message: host.error_message(),
            });
        }
        tracing::debug!(session = host.session, "managed host started");
        Ok(host)
    }

    /// Confirms that this session still owns the managed host and returns its identity.
    pub fn session_id(&self) -> Result<u64, Error> {
        self.call(2).map(|packet| packet.session)
    }

    /// Runs queued Linux control callbacks on this host's creation thread.
    ///
    /// Call regularly from your application loop, for example before draining output. Each call
    /// starts at most 64 callbacks within a 5 ms scheduling budget; a callback itself can take
    /// longer. Windows UI dispatch remains the caller's native message-loop responsibility.
    pub fn pump(&mut self) -> Result<u32, Error> {
        self.call(4).map(|packet| packet.runtime_major)
    }

    /// Ends active combat, drains admitted actions and publishes its latest result.
    ///
    /// Live file callbacks are quiesced and resumed around completion. Call before importing while
    /// live combat is active, or to read a final result without waiting for another log timestamp.
    /// This is a no-op when idle; it does not flush a plugin's private deferred events or writer.
    pub fn end_combat(&mut self) -> Result<(), Error> {
        self.call(5).map(|_packet| ())
    }

    /// Reads bounded host-error and backlog-warning history without consuming it or requiring a pump.
    ///
    /// Compare `total` between calls, and inspect `omitted()` to detect older errors outside the
    /// retained history. This reports host diagnostics, not errors caught privately by a plugin.
    pub fn diagnostics(&self) -> Result<HostDiagnostics, Error> {
        let mut packet = BytesMut::with_capacity(DIAGNOSTIC_CAPACITY);
        packet.put_u32_le(DIAGNOSTIC_CAPACITY as u32);
        packet.put_u32_le(1);
        packet.put_u64_le(self.session);
        packet.resize(DIAGNOSTIC_CAPACITY, 0);
        // SAFETY: ReadDiagnostics borrows this initialized allocation for the call, validates the
        // header, and writes at most its fixed capacity. No managed memory escapes the call.
        let status = unsafe {
            (self.read_diagnostics)(packet.as_mut_ptr().cast(), DIAGNOSTIC_CAPACITY as i32)
        };
        if status != 0 {
            return Err(Error::Managed {
                command: 7,
                status,
                message: self.error_message(),
            });
        }
        decode_diagnostics(&packet)
    }

    /// Imports the initial contents of a UTF-8 file through initialized plugins and drains actions.
    ///
    /// Call on the creation thread while live combat is idle. Live file delivery is paused and
    /// restored, including after failure. Appends after opening are excluded; malformed UTF-8,
    /// records over 8 MiB and premature truncation fail visibly. Partial imported history remains
    /// after failure. Hook delivery alone does not establish that a plugin accepted the records.
    pub fn import_log(&mut self, path: impl AsRef<Utf8Path>) -> Result<ImportReport, Error> {
        let path = canonicalize(path.as_ref())?;
        let mut packet = import_packet(self.session, path.as_str())?;
        // The encoder bounds this request to 64 KiB before conversion.
        let length = packet.len() as i32;
        // SAFETY: ImportLog borrows this initialized allocation for the call, validates every
        // range before importing, and writes only the fixed result in its 64-byte header.
        let status = unsafe { (self.import_log)(packet.as_mut_ptr().cast(), length) };
        if status != 0 {
            return Err(Error::Managed {
                command: 8,
                status,
                message: self.error_message(),
            });
        }
        decode_import(&packet)
    }

    /// Copies the latest ended encounter, or returns None when no result is newer than `after`.
    ///
    /// Pass None for the latest result, then its revision to poll for changes. Intermediate state
    /// versions coalesce; this is not an encounter archive. Results own their strings and counters
    /// after host stop. Imports can retain partial results after failure; inspect `status`.
    pub fn combat_snapshot(
        &mut self,
        after: Option<CombatRevision>,
    ) -> Result<Option<CombatSnapshot>, Error> {
        if after.is_some_and(|revision| revision.session != self.session) {
            return Err(Error::InvalidSnapshotRequest(
                "revision belongs to another host session".to_owned(),
            ));
        }
        let after = after.map_or(0, |revision| revision.sequence);
        let mut capacity = 64 * 1024;
        loop {
            let mut packet = snapshot_packet(self.session, after, capacity);
            // SAFETY: ReadCombatSnapshot borrows this initialized, bounded allocation only during
            // this call. The bridge validates its header and never retains managed or Rust pointers.
            let status =
                unsafe { (self.read_combat_snapshot)(packet.as_mut_ptr().cast(), capacity as i32) };
            match status {
                0 => return decode_snapshot(&packet, self.session, after),
                1 => capacity = larger_snapshot_capacity(&packet)?,
                status => {
                    return Err(Error::Managed {
                        command: 10,
                        status,
                        message: self.error_message(),
                    });
                }
            }
        }
    }

    /// Starts this host's single bounded subscription to nonempty original text selected for ACT hooks.
    ///
    /// Slow consumers lose oldest pending records, with exact loss ranges returned by
    /// [`Self::drain_logs`]. Core parsing continues. This is not a durable recording or a parsed
    /// record count. Unsubscribe before configuring another subscription.
    pub fn subscribe_logs(
        &mut self,
        config: LogSubscriptionConfig,
    ) -> Result<LogSubscription, Error> {
        let mut packet = subscribe_packet(self.session, config)?;
        if self.run_log_command(&mut packet)? != 0 {
            return Err(Error::InvalidLogResponse(
                "unexpected subscription status".to_owned(),
            ));
        }
        subscription_result(&packet, self.session)
    }

    /// Drains an owned batch without waiting for new records or borrowing managed memory.
    ///
    /// Capacity retries do not consume pending records or loss state. Read regularly, for example
    /// after [`Self::pump`], and drain until `records` is empty. Cloned line bytes share only Rust
    /// memory. An oversized record discards the pending prefix and itself, preserving an exact
    /// missing range; inspect `dropped` even for an empty batch.
    pub fn drain_logs(&mut self, subscription: LogSubscription) -> Result<LogBatch, Error> {
        self.check_log_subscription(subscription)?;
        let mut capacity = 64 * 1024;
        loop {
            let mut packet = log_packet(self.session, subscription.instance, 2, capacity);
            match self.run_log_command(&mut packet)? {
                0 => return decode_logs(packet.freeze()),
                1 => capacity = larger_log_capacity(&packet)?,
                _ => {
                    return Err(Error::InvalidLogResponse(
                        "unexpected drain status".to_owned(),
                    ));
                }
            }
        }
    }

    /// Ends a subscription and explicitly discards its unread records and loss state.
    /// Host stop also discards unread subscription data.
    pub fn unsubscribe_logs(&mut self, subscription: LogSubscription) -> Result<(), Error> {
        self.check_log_subscription(subscription)?;
        let mut packet = log_packet(self.session, subscription.instance, 3, LOG_HEADER);
        if self.run_log_command(&mut packet)? != 0 {
            return Err(Error::InvalidLogResponse(
                "unexpected unsubscribe status".to_owned(),
            ));
        }
        Ok(())
    }

    fn check_log_subscription(&self, subscription: LogSubscription) -> Result<(), Error> {
        if subscription.session != self.session {
            return Err(Error::InvalidLogRequest(
                "subscription belongs to another host session".to_owned(),
            ));
        }
        Ok(())
    }

    fn run_log_command(&self, packet: &mut BytesMut) -> Result<i32, Error> {
        // Requests are constructed within the private 64 MiB ABI limit before this conversion.
        let length = packet.len() as i32;
        // SAFETY: LogCommand borrows this initialized allocation for this call only. It validates
        // headers and writes at most capacity, never retaining the pointer or invoking Rust.
        let status = unsafe { (self.log_command)(packet.as_mut_ptr().cast(), length) };
        if status < 0 {
            return Err(Error::Managed {
                command: 9,
                status,
                message: self.error_message(),
            });
        }
        Ok(status)
    }

    /// Constructs a trusted plugin and retains it until this host stops.
    ///
    /// Constructors execute arbitrary code in this process and may register global handlers.
    /// No sandbox or assembly unloading is provided. `InitPlugin` and `DeInitPlugin` are not
    /// called by this method. Linux provides only headless status controls, without rendering
    /// or native handles. For example, select a type with
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

    /// Calls `InitPlugin` once and returns its bounded status text, not proof of parser readiness.
    ///
    /// Same-thread exceptions reported through ACT's `WriteExceptionLog` during the callback fail this
    /// attempt. Failed instances cannot be retried; construct another instance instead. Arbitrary
    /// plugin side effects cannot be rolled back. Windows requires a caller-owned STA/UI loop.
    pub fn initialize_plugin(&mut self, plugin: PluginId) -> Result<String, Error> {
        self.plugin_call(plugin, 1)
    }

    /// Calls `DeInitPlugin` at most once, removes its registration and disposes host-owned controls.
    /// The host does not retry a failing callback during shutdown.
    pub fn deinitialize_plugin(&mut self, plugin: PluginId) -> Result<(), Error> {
        self.plugin_call(plugin, 2).map(|_status| ())
    }

    fn plugin_call(&mut self, plugin: PluginId, command: u32) -> Result<String, Error> {
        if plugin.session != self.session {
            return Err(Error::InvalidPluginRequest(
                "plugin belongs to another host session".to_owned(),
            ));
        }
        let mut request = BytesMut::with_capacity(8224);
        request.put_u32_le(8224);
        request.put_u32_le(1);
        request.put_u64_le(self.session);
        request.put_u64_le(plugin.instance);
        request.put_u32_le(command);
        request.put_u32_le(0);
        request.resize(8224, 0);
        // SAFETY: PluginCommand requires exactly this initialized allocation, retains no pointer,
        // and validates the header before writing bounded UTF-8 status output after byte 32.
        let status = unsafe { (self.plugin_command)(request.as_mut_ptr().cast(), 8224) };
        if status != 0 {
            return Err(Error::Managed {
                command: 6,
                status,
                message: self.error_message(),
            });
        }
        let length: [u8; 4] = request
            .get(28..32)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| {
                Error::InvalidPluginRequest("missing plugin status length".to_owned())
            })?;
        let length = u32::from_le_bytes(length) as usize;
        let bytes = request
            .get(32..)
            .and_then(|bytes| bytes.get(..length))
            .ok_or_else(|| {
                Error::InvalidPluginRequest("plugin status exceeds capacity".to_owned())
            })?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|error| Error::InvalidPluginRequest(error.to_string()))
    }

    /// Stops the managed session. The CLR and loaded assemblies may remain in the process.
    pub fn stop(mut self) -> Result<(), Error> {
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
            host.session_id()
                .expect("command failure must preserve the session"),
            0
        );
        host.stop().expect("stop managed component");
    }
}
