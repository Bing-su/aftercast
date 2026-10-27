use bon::Builder;
use bytes::{Buf, BufMut, BytesMut};
use camino::Utf8PathBuf;

use crate::runtime::Request;
use crate::{Error, HostHandle};

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

/// Lifecycle of one instance. Failed and stopped instances cannot initialize again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    Constructed,
    Initializing,
    Initialized,
    Failed,
    Stopped,
}

/// Native settings capability, separate from whether a plugin is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginSettings {
    UnsupportedPlatform,
    NoPage,
    Available,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginStatus {
    pub state: PluginState,
    pub settings: PluginSettings,
}

impl HostHandle {
    /// Constructs a trusted plugin and retains it until this host stops.
    ///
    /// Constructors execute arbitrary code in the isolated child and may register global handlers.
    /// ACT globals and the settings directory are available before construction. The library owns
    /// the plugin thread and its Windows STA/message loop.
    /// No sandbox or assembly unloading is provided. `InitPlugin` and `DeInitPlugin` are not
    /// called by this method. Linux provides only headless status controls, without rendering
    /// or native handles. For example, select a type with
    /// `PluginConfig::builder().path("plugin.dll").type_name("Example.Plugin").build()`.
    pub async fn create_plugin(&self, config: PluginConfig) -> Result<PluginId, Error> {
        let path = self.canonicalize(&config.path).await?;
        let mut packet = plugin_packet(self.session, path.as_str(), config.type_name.as_deref())?;
        self.request(Request::CreatePlugin, &mut packet).await?;
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
    /// attempt. Failed instances cannot be retried. A new instance does not unload the assembly or
    /// undo arbitrary side effects; restart the Host unless the plugin's cleanup is known safe. Initialize prerequisites before constructing
    /// dependents that require their assembly resolvers, e.g. OverlayPlugin before cactbot.
    pub async fn initialize_plugin(&self, plugin: PluginId) -> Result<String, Error> {
        self.plugin_call(plugin, 1).await
    }

    /// Calls `DeInitPlugin` at most once, removes its registration and disposes host-owned controls.
    /// The host does not retry a failing callback during shutdown.
    pub async fn deinitialize_plugin(&self, plugin: PluginId) -> Result<(), Error> {
        self.plugin_call(plugin, 2).await.map(|_status| ())
    }

    /// Reads fresh lifecycle/settings availability. Linux explicitly reports UnsupportedPlatform.
    pub async fn plugin_status(&self, plugin: PluginId) -> Result<PluginStatus, Error> {
        let packet = self.plugin_request(plugin, 3).await?;
        decode_plugin_status(&packet)
    }

    /// Shows/selects the existing child-owned nonmodal WinForms page and focuses its shared window.
    /// User close hides the window; shutdown owns disposal. Linux and detached/removed pages fail explicitly.
    pub async fn open_plugin_settings(&self, plugin: PluginId) -> Result<(), Error> {
        self.plugin_call(plugin, 4).await.map(|_status| ())
    }

    async fn plugin_request(&self, plugin: PluginId, command: u32) -> Result<BytesMut, Error> {
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
        self.request(Request::PluginCommand, &mut request).await?;
        Ok(request)
    }

    async fn plugin_call(&self, plugin: PluginId, command: u32) -> Result<String, Error> {
        let request = self.plugin_request(plugin, command).await?;
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

fn decode_plugin_status(packet: &[u8]) -> Result<PluginStatus, Error> {
    let fields = packet
        .get(28..40)
        .ok_or_else(|| Error::InvalidPluginRequest("missing plugin status".into()))?;
    let mut fields = fields;
    if fields.get_u32_le() != 8 {
        return Err(Error::InvalidPluginRequest(
            "invalid plugin status length".into(),
        ));
    }
    let state = match fields.get_u32_le() {
        0 => PluginState::Constructed,
        1 => PluginState::Initializing,
        2 => PluginState::Initialized,
        3 => PluginState::Failed,
        4 => PluginState::Stopped,
        _ => return Err(Error::InvalidPluginRequest("invalid plugin state".into())),
    };
    let settings = match fields.get_u32_le() {
        0 => PluginSettings::UnsupportedPlatform,
        1 => PluginSettings::NoPage,
        2 => PluginSettings::Available,
        _ => {
            return Err(Error::InvalidPluginRequest(
                "invalid plugin settings capability".into(),
            ));
        }
    };
    if settings == PluginSettings::Available && state != PluginState::Initialized {
        return Err(Error::InvalidPluginRequest(
            "settings available for an inactive instance".into(),
        ));
    }
    Ok(PluginStatus { state, settings })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_status_rejects_truncated_or_impossible_capabilities() {
        let mut packet = vec![0_u8; 40];
        packet[28..32].copy_from_slice(&8_u32.to_le_bytes());
        packet[32..36].copy_from_slice(&2_u32.to_le_bytes());
        packet[36..40].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            decode_plugin_status(&packet)
                .expect("initialized settings")
                .settings,
            PluginSettings::Available
        );
        for length in 0..40 {
            decode_plugin_status(&packet[..length]).expect_err("truncated status");
        }
        for (offset, value) in [(28, 0_u32), (32, 5), (36, 3), (32, 4)] {
            let mut bad = packet.clone();
            bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            decode_plugin_status(&bad).expect_err("invalid status");
        }
    }

    #[rstest::rstest]
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
}
