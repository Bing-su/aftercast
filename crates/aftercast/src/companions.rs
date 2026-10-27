//! Explicit companion source selections. Capture is owned by the Host, not by a copied handle.
use bytes::{BufMut, BytesMut};
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::time::Duration;

use crate::{
    Error, HostHandle, ImportEncoding, ImportInvalidBytes, ImportRange, ImportTextOptions,
    runtime::Request,
};

const HEADER: usize = 32;
const CAPACITY: usize = 512 * 1024;
const MAX_TICKS: u64 = 3_155_378_975_999_999_999;

/// A bounded sibling catalog proposal, not an open handle or an identity lease.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompanionCandidate {
    pub name: String,
    pub path: Utf8PathBuf,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompanionCatalog {
    pub path: Utf8PathBuf,
    pub candidates: Vec<CompanionCandidate>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionSource {
    pub name: String,
    /// None inherits the admitted main codec; this never follows later plugin reassignments.
    pub text: Option<ImportTextOptions>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceImportOptions {
    pub range: Option<ImportRange>,
    pub text: Option<ImportTextOptions>,
    pub companions: Vec<CompanionSource>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveLogOptions {
    pub text: Option<ImportTextOptions>,
    pub companions: Vec<CompanionSource>,
    pub get_current_zone: bool,
    /// Positive whole milliseconds through five minutes; per-open, never persisted.
    pub max_clock_wait: Duration,
}
impl Default for LiveLogOptions {
    fn default() -> Self {
        Self {
            text: None,
            companions: Vec::new(),
            get_current_zone: false,
            max_clock_wait: Duration::from_secs(30),
        }
    }
}
/// Copying/dropping a selection does not close capture. Old copies become stale after close/reopen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LiveLogSelection {
    session: u64,
    value: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveLogState {
    Following,
    Paused,
    WaitingForClock,
    Changing,
    Failed,
    Stopped,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LiveLogSourceStatus {
    /// Exact request identity; `name` retains the catalog's physical casing.
    pub selected_name: String,
    pub name: String,
    pub path: Utf8PathBuf,
    pub state: LiveLogState,
    pub queued: u32,
    /// Source wall-clock ticks, not an inferred UTC instant.
    pub pending_ticks: Option<u64>,
    pub generation: u64,
    pub dropped: u64,
    pub last_error: String,
    pub error_truncated: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveLogSnapshot {
    pub selection: LiveLogSelection,
    pub path: Utf8PathBuf,
    pub options: LiveLogOptions,
    pub sources: Vec<LiveLogSourceStatus>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WireText {
    code_page: u32,
    replace_invalid: bool,
    detect_bom: bool,
}
impl From<ImportTextOptions> for WireText {
    fn from(value: ImportTextOptions) -> Self {
        Self {
            code_page: value.encoding as u32,
            replace_invalid: value.invalid_bytes == ImportInvalidBytes::Replace,
            detect_bom: value.detect_bom,
        }
    }
}
impl TryFrom<WireText> for ImportTextOptions {
    type Error = Error;
    fn try_from(value: WireText) -> Result<Self, Error> {
        let encoding = match value.code_page {
            65001 => ImportEncoding::Utf8,
            1200 => ImportEncoding::Utf16Le,
            1201 => ImportEncoding::Utf16Be,
            12000 => ImportEncoding::Utf32Le,
            12001 => ImportEncoding::Utf32Be,
            932 => ImportEncoding::ShiftJis,
            949 => ImportEncoding::WindowsKorean,
            1252 => ImportEncoding::Windows1252,
            20127 => ImportEncoding::Ascii,
            _ => return Err(invalid("unknown response encoding")),
        };
        Ok(Self {
            encoding,
            invalid_bytes: if value.replace_invalid {
                ImportInvalidBytes::Replace
            } else {
                ImportInvalidBytes::Strict
            },
            detect_bom: value.detect_bom,
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WireSource {
    name: String,
    text: Option<WireText>,
}
impl From<&CompanionSource> for WireSource {
    fn from(source: &CompanionSource) -> Self {
        Self {
            name: source.name.clone(),
            text: source.text.map(Into::into),
        }
    }
}
pub(crate) fn validate_sources(sources: &[CompanionSource], maximum: usize) -> Result<(), Error> {
    if sources.len() > maximum {
        return Err(invalid("too many selected companions"));
    }
    for (index, source) in sources.iter().enumerate() {
        validate_name(&source.name)?;
        if sources
            .iter()
            .take(index)
            .any(|previous| previous.name.eq_ignore_ascii_case(&source.name))
        {
            return Err(invalid("duplicate companion selection"));
        }
    }
    // The bridge additionally enforces .NET OrdinalIgnoreCase for Unicode case variants.
    Ok(())
}
fn validate_name(name: &str) -> Result<(), Error> {
    if name.trim().is_empty()
        || name.encode_utf16().count() > 256
        || name
            .chars()
            .any(|c| c.is_control() || ".\\/:*?\"<>|".contains(c))
    {
        return Err(invalid("invalid companion name"));
    }
    Ok(())
}
fn validate_path(path: &Utf8Path, extra: usize) -> Result<(), Error> {
    if !path.is_absolute()
        || path.as_str().chars().any(char::is_control)
        || path.as_str().len() > 8192 + extra
    {
        return Err(invalid("invalid absolute companion path"));
    }
    Ok(())
}
fn clock_millis(duration: Duration) -> Result<u32, Error> {
    if !duration.subsec_nanos().is_multiple_of(1_000_000) {
        return Err(invalid("clock timeout requires whole milliseconds"));
    }
    let value = u32::try_from(duration.as_millis()).map_err(invalid)?;
    if !(1..=300_000).contains(&value) {
        return Err(invalid("clock timeout must be 1..300000 milliseconds"));
    }
    Ok(value)
}
pub(crate) fn invalid(message: impl std::fmt::Display) -> Error {
    Error::InvalidCompanionRequest(message.to_string())
}
pub(crate) fn packet(
    session: u64,
    command: u32,
    value: &impl Serialize,
) -> Result<BytesMut, Error> {
    let input = serde_json::to_vec(value).map_err(invalid)?;
    if !(2..=128 * 1024).contains(&input.len()) {
        return Err(invalid("companion input exceeds 128 KiB"));
    }
    let mut packet = BytesMut::with_capacity(HEADER + CAPACITY);
    packet.put_u32_le((HEADER + CAPACITY) as u32);
    packet.put_u32_le(1);
    packet.put_u64_le(session);
    packet.put_u32_le(command);
    packet.put_u32_le(input.len() as u32);
    packet.put_u64_le(0);
    packet.extend_from_slice(&input);
    packet.resize(HEADER + CAPACITY, 0);
    Ok(packet)
}
pub(crate) fn reply<T: DeserializeOwned>(
    packet: &[u8],
    session: u64,
    command: u32,
) -> Result<T, Error> {
    fn number<const N: usize>(data: &[u8], start: usize) -> Result<[u8; N], Error> {
        data.get(start..start + N)
            .and_then(|value| value.try_into().ok())
            .ok_or_else(|| invalid("short companion response"))
    }
    if packet.len() != HEADER + CAPACITY
        || u32::from_le_bytes(number(packet, 0)?) as usize != packet.len()
        || u32::from_le_bytes(number(packet, 4)?) != 1
        || u64::from_le_bytes(number(packet, 8)?) != session
        || u32::from_le_bytes(number(packet, 16)?) != command
        || u32::from_le_bytes(number(packet, 28)?) != 0
    {
        return Err(invalid("invalid companion reply header"));
    }
    let length = u32::from_le_bytes(number(packet, 24)?) as usize;
    if !(2..=CAPACITY).contains(&length) {
        return Err(invalid("invalid companion output length"));
    }
    serde_json::from_slice(
        packet
            .get(HEADER..HEADER + length)
            .ok_or_else(|| invalid("short companion output"))?,
    )
    .map_err(invalid)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawLive {
    owner: u64,
    path: Utf8PathBuf,
    text: Option<WireText>,
    companions: Vec<WireSource>,
    get_current_zone: bool,
    clock_wait_milliseconds: u32,
    sources: Vec<LiveLogSourceStatus>,
}
fn live_snapshot(
    raw: RawLive,
    session: u64,
    expected_owner: Option<u64>,
) -> Result<LiveLogSnapshot, Error> {
    if raw.owner == 0 || expected_owner.is_some_and(|owner| owner != raw.owner) {
        return Err(invalid("live owner mismatch"));
    }
    validate_path(&raw.path, 0)?;
    let companions = raw
        .companions
        .into_iter()
        .map(|source| {
            Ok(CompanionSource {
                name: source.name,
                text: source.text.map(TryInto::try_into).transpose()?,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    validate_sources(&companions, 8)?;
    let wait = Duration::from_millis(u64::from(raw.clock_wait_milliseconds));
    clock_millis(wait)?;
    if raw.sources.len() != companions.len() + 1 {
        return Err(invalid("live source count mismatch"));
    }
    let parent = raw
        .path
        .parent()
        .ok_or_else(|| invalid("missing main parent"))?
        .join("CompanionLogs");
    for (index, source) in raw.sources.iter().enumerate() {
        validate_path(&source.path, 1024)?;
        if source.queued > 1
            || source.generation == 0
            || source.pending_ticks.is_some_and(|ticks| ticks > MAX_TICKS)
            || source.last_error.encode_utf16().count() > 2048
        {
            return Err(invalid("invalid live source status"));
        }
        if index == 0 {
            if !source.selected_name.is_empty()
                || !source.name.is_empty()
                || source.path != raw.path
            {
                return Err(invalid("invalid main source status"));
            }
        } else {
            validate_name(&source.name)?;
            // ACT appends the companion component before the main extension, if any.
            let physical_stem = if raw
                .path
                .extension()
                .is_some_and(|extension| !extension.is_empty())
            {
                source.path.file_stem()
            } else {
                source.path.file_name()
            }
            .and_then(|stem| stem.rsplit_once('.'));
            if !companions
                .iter()
                .any(|selected| selected.name == source.selected_name)
                || !physical_stem
                    .is_some_and(|(main, name)| !main.is_empty() && name == source.name)
                || source.path.parent() != Some(parent.as_path())
                || raw.sources.iter().take(index).any(|previous| {
                    previous.selected_name == source.selected_name
                        || previous.name.eq_ignore_ascii_case(&source.name)
                        || previous.path == source.path
                })
            {
                return Err(invalid("invalid companion source identity"));
            }
        }
    }
    Ok(LiveLogSnapshot {
        selection: LiveLogSelection {
            session,
            value: raw.owner,
        },
        path: raw.path,
        options: LiveLogOptions {
            text: raw.text.map(TryInto::try_into).transpose()?,
            companions,
            get_current_zone: raw.get_current_zone,
            max_clock_wait: wait,
        },
        sources: raw.sources,
    })
}
impl HostHandle {
    /// Lists at most 64 matching siblings from at most 4096 entries; never recursively searches.
    pub async fn companion_candidates(
        &self,
        path: impl AsRef<Utf8Path>,
    ) -> Result<CompanionCatalog, Error> {
        let path = self.canonicalize(path.as_ref()).await?;
        validate_path(&path, 0)?;
        let catalog: CompanionCatalog = self
            .companion_call(7, &serde_json::json!({"path":path}))
            .await?;
        if catalog.path != path || catalog.candidates.len() > 64 {
            return Err(invalid("invalid companion catalog"));
        }
        let parent = path
            .parent()
            .ok_or_else(|| invalid("missing main parent"))?
            .join("CompanionLogs");
        for (index, candidate) in catalog.candidates.iter().enumerate() {
            validate_name(&candidate.name)?;
            validate_path(&candidate.path, 1024)?;
            if candidate.path.parent() != Some(parent.as_path())
                || catalog.candidates.iter().take(index).any(|previous| {
                    previous.name.eq_ignore_ascii_case(&candidate.name)
                        || previous.path == candidate.path
                })
            {
                return Err(invalid("invalid catalog candidate identity"));
            }
        }
        Ok(catalog)
    }
    /// Opens one explicitly selected tail group. Empty companions retain single-main behavior.
    pub async fn open_live_logs(
        &self,
        path: impl AsRef<Utf8Path>,
        options: LiveLogOptions,
    ) -> Result<LiveLogSnapshot, Error> {
        self.open_live_request(0, path.as_ref(), options).await
    }
    /// Replaces this exact selection. Once reader replacement starts, old copies become stale even on open failure.
    /// Request validation failures preserve the current selection.
    pub async fn reopen_live_logs(
        &self,
        selection: LiveLogSelection,
        path: impl AsRef<Utf8Path>,
        options: LiveLogOptions,
    ) -> Result<LiveLogSnapshot, Error> {
        self.check_live(selection)?;
        self.open_live_request(selection.value, path.as_ref(), options)
            .await
    }
    async fn open_live_request(
        &self,
        owner: u64,
        path: &Utf8Path,
        options: LiveLogOptions,
    ) -> Result<LiveLogSnapshot, Error> {
        validate_sources(&options.companions, 8)?;
        let wait = clock_millis(options.max_clock_wait)?;
        let path = self.canonicalize(path).await?;
        validate_path(&path, 0)?;
        let request = serde_json::json!({"owner":owner,"path":path,"text":options.text.map(WireText::from),
            "companions":options.companions.iter().map(WireSource::from).collect::<Vec<_>>(),"getCurrentZone":options.get_current_zone,"clockWaitMilliseconds":wait});
        let raw: RawLive = self.companion_call(4, &request).await?;
        let snapshot = live_snapshot(raw, self.session, None)?;
        if snapshot.selection.value == owner || snapshot.path != path || snapshot.options != options
        {
            return Err(invalid("opened selection snapshot mismatch"));
        }
        Ok(snapshot)
    }
    pub async fn live_logs(&self, selection: LiveLogSelection) -> Result<LiveLogSnapshot, Error> {
        self.check_live(selection)?;
        live_snapshot(
            self.companion_call(5, &serde_json::json!({"owner":selection.value}))
                .await?,
            self.session,
            Some(selection.value),
        )
    }
    /// Joins the selected readers; combat state and the Host remain available.
    pub async fn close_live_logs(&self, selection: LiveLogSelection) -> Result<(), Error> {
        self.check_live(selection)?;
        let closed: bool = self
            .companion_call(6, &serde_json::json!({"owner":selection.value}))
            .await?;
        if !closed {
            return Err(invalid("live close was not acknowledged"));
        }
        Ok(())
    }
    fn check_live(&self, selection: LiveLogSelection) -> Result<(), Error> {
        if selection.session != self.session || selection.value == 0 {
            return Err(invalid("live handle belongs to another session"));
        }
        Ok(())
    }
    async fn companion_call<T: DeserializeOwned>(
        &self,
        command: u32,
        value: &impl Serialize,
    ) -> Result<T, Error> {
        let mut packet = packet(self.session, command, value)?;
        self.request(Request::Companions, &mut packet).await?;
        reply(&packet, self.session, command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(value: serde_json::Value) -> BytesMut {
        let mut data = packet(7, 5, &serde_json::json!({"owner":1})).expect("request");
        let body = serde_json::to_vec(&value).expect("response JSON");
        data.get_mut(24..28)
            .expect("length")
            .copy_from_slice(&(body.len() as u32).to_le_bytes());
        data.get_mut(HEADER..HEADER + body.len())
            .expect("body")
            .copy_from_slice(&body);
        data
    }

    fn status() -> serde_json::Value {
        serde_json::json!({"owner":1,"path":"/tmp/main.log","text":null,"companions":[],
            "getCurrentZone":false,"clockWaitMilliseconds":30000,"sources":[{
                "selectedName":"","name":"","path":"/tmp/main.log","state":"following","queued":0,
                "pendingTicks":null,"generation":1,"dropped":0,"lastError":"","errorTruncated":false}]})
    }

    #[test]
    fn rejects_malformed_reply_before_publishing_a_selection() {
        let data = response(status());
        let parsed: RawLive = reply(&data, 7, 5).expect("valid wire snapshot");
        assert_eq!(
            live_snapshot(parsed, 7, Some(1))
                .expect("selection")
                .sources
                .len(),
            1
        );
        for (offset, bytes) in [
            (0, 1_u32.to_le_bytes()),
            (4, 2_u32.to_le_bytes()),
            (16, 4_u32.to_le_bytes()),
            (24, 0_u32.to_le_bytes()),
            (24, (CAPACITY as u32 + 1).to_le_bytes()),
            (28, 1_u32.to_le_bytes()),
        ] {
            let mut bad = data.clone();
            bad.get_mut(offset..offset + 4)
                .expect("header")
                .copy_from_slice(&bytes);
            assert!(reply::<RawLive>(&bad, 7, 5).is_err());
        }
        assert!(reply::<RawLive>(&data, 8, 5).is_err());
        assert!(reply::<RawLive>(&data[..HEADER], 7, 5).is_err());
        for (key, value) in [
            ("generation", serde_json::json!(0)),
            ("queued", serde_json::json!(2)),
            ("pendingTicks", serde_json::json!(MAX_TICKS + 1)),
            ("state", serde_json::json!("unknown")),
            ("path", serde_json::json!("/tmp/other.log")),
            ("lastError", serde_json::json!("x".repeat(2049))),
        ] {
            let mut bad = status();
            bad["sources"][0][key] = value;
            let parsed = reply::<RawLive>(&response(bad), 7, 5);
            assert!(
                parsed
                    .and_then(|raw| live_snapshot(raw, 7, Some(1)))
                    .is_err(),
                "{key}"
            );
        }
        let parsed = reply::<RawLive>(&data, 7, 5).expect("wire snapshot");
        live_snapshot(parsed, 7, Some(2)).expect_err("stale owner");
    }

    #[test]
    fn live_sources_bind_exact_request_identity_and_physical_name() {
        let mut good = status();
        good["companions"] = serde_json::json!([{"name":"ALPHA","text":null}]);
        let source = serde_json::json!({"selectedName":"ALPHA","name":"Alpha","path":"/tmp/CompanionLogs/main.Alpha.log",
            "state":"following","queued":0,"pendingTicks":null,"generation":1,"dropped":0,"lastError":"","errorTruncated":false});
        good["sources"]
            .as_array_mut()
            .expect("sources")
            .push(source);
        let decode = |value| {
            live_snapshot(
                reply::<RawLive>(&response(value), 7, 5).expect("wire"),
                7,
                Some(1),
            )
        };
        decode(good.clone()).expect("case-preserving catalog binding");
        let mut extensionless = good.clone();
        extensionless["path"] = serde_json::json!("/tmp/main");
        extensionless["sources"][0]["path"] = serde_json::json!("/tmp/main");
        extensionless["sources"][1]["path"] = serde_json::json!("/tmp/CompanionLogs/main.Alpha");
        decode(extensionless.clone()).expect("extensionless ACT companion path");
        extensionless["path"] = serde_json::json!("/tmp/main.");
        extensionless["sources"][0]["path"] = serde_json::json!("/tmp/main.");
        decode(extensionless).expect("empty extension uses .NET filename convention");
        for (key, value) in [
            ("selectedName", "Beta"),
            ("path", "/tmp/CompanionLogs/main.Beta.log"),
            ("name", "Beta"),
        ] {
            let mut bad = good.clone();
            bad["sources"][1][key] = serde_json::json!(value);
            decode(bad).expect_err("wrong selected or physical identity");
        }
        let mut duplicate = good.clone();
        duplicate["companions"]
            .as_array_mut()
            .expect("companions")
            .push(serde_json::json!({"name":"Zulu","text":null}));
        duplicate["sources"]
            .as_array_mut()
            .expect("sources")
            .push(good["sources"][1].clone());
        decode(duplicate).expect_err("duplicate selected source");
    }

    #[test]
    fn selection_limits_are_explicit_and_do_not_round_clock_timeouts() {
        for duration in [
            Duration::ZERO,
            Duration::from_nanos(1),
            Duration::from_millis(300001),
        ] {
            clock_millis(duration).expect_err("invalid timeout");
        }
        assert_eq!(clock_millis(Duration::from_millis(1)).expect("minimum"), 1);
        assert_eq!(
            clock_millis(Duration::from_secs(300)).expect("maximum"),
            300000
        );
        for name in ["", "../escape", "a.log", "a/b", "a\\b", "a\n"] {
            assert!(validate_name(name).is_err());
        }
        let one = CompanionSource {
            name: "Alpha".into(),
            text: None,
        };
        assert!(
            validate_sources(
                &[
                    one.clone(),
                    CompanionSource {
                        name: "alpha".into(),
                        text: None
                    }
                ],
                8
            )
            .is_err()
        );
        assert!(validate_sources(&vec![one; 9], 8).is_err());
        ImportTextOptions::try_from(WireText {
            code_page: 42,
            replace_invalid: false,
            detect_bom: true,
        })
        .expect_err("unknown code page");
    }
}
