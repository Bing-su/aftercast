use bon::Builder;
use bytes::{BufMut, BytesMut};
use camino::Utf8PathBuf;

use crate::runtime::{Request, canonicalize};
use crate::{Error, Host};

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

impl Host {
    /// Constructs a trusted plugin and retains it until this host stops.
    ///
    /// Constructors execute arbitrary code in this process and may register global handlers.
    /// ACT globals and the settings directory are available before construction. Windows requires
    /// a caller-owned STA thread from this operation onward.
    /// No sandbox or assembly unloading is provided. `InitPlugin` and `DeInitPlugin` are not
    /// called by this method. Linux provides only headless status controls, without rendering
    /// or native handles. For example, select a type with
    /// `PluginConfig::builder().path("plugin.dll").type_name("Example.Plugin").build()`.
    pub fn create_plugin(&mut self, config: PluginConfig) -> Result<PluginId, Error> {
        let path = canonicalize(&config.path)?;
        let mut packet = plugin_packet(
            self.session.id(),
            path.as_str(),
            config.type_name.as_deref(),
        )?;
        self.session.request(Request::CreatePlugin, &mut packet)?;
        let bytes: [u8; 8] = packet
            .get(32..40)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| Error::InvalidPluginRequest("missing plugin result".to_owned()))?;
        let instance = u64::from_le_bytes(bytes);
        if instance == 0 {
            return Err(Error::InvalidPluginRequest("zero plugin result".to_owned()));
        }
        Ok(PluginId {
            session: self.session.id(),
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
        if plugin.session != self.session.id() {
            return Err(Error::InvalidPluginRequest(
                "plugin belongs to another host session".to_owned(),
            ));
        }
        let mut request = BytesMut::with_capacity(8224);
        request.put_u32_le(8224);
        request.put_u32_le(1);
        request.put_u64_le(self.session.id());
        request.put_u64_le(plugin.instance);
        request.put_u32_le(command);
        request.put_u32_le(0);
        request.resize(8224, 0);
        self.session.request(Request::PluginCommand, &mut request)?;
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
}

pub(crate) fn plugin_packet(
    session: u64,
    path: &str,
    type_name: Option<&str>,
) -> Result<BytesMut, Error> {
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
