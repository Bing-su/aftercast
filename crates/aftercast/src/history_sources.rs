//! Explicit history source proposals and one-shot captured-byte replay.
use bytes::BytesMut;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    CompanionSource, Error, HistoryId, HistoryRevision, HostHandle, ImportJob, ImportRange,
    ImportTextOptions,
    companions::{WireSource, WireText},
    runtime::Request,
};
const CAPACITY: usize = 4 * 1024 * 1024;
const MAXIMUM_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySourceQuery {
    pub paths: Vec<Utf8PathBuf>,
    pub roots: Vec<Utf8PathBuf>,
    pub filter: String,
}
impl Default for HistorySourceQuery {
    fn default() -> Self {
        Self {
            paths: vec![],
            roots: vec![],
            filter: "*.log".into(),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[repr(u32)]
#[serde(try_from = "u32")]
pub enum HistorySourceState {
    Available,
    Missing,
    Unavailable,
}
impl TryFrom<u32> for HistorySourceState {
    type Error = &'static str;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Available),
            1 => Ok(Self::Missing),
            2 => Ok(Self::Unavailable),
            _ => Err("invalid source state"),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u32")]
pub enum HistorySourceChange {
    New,
    MetadataUnchanged,
    MetadataChanged,
    Replaced,
    Moved,
}
impl TryFrom<u32> for HistorySourceChange {
    type Error = &'static str;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::New),
            1 => Ok(Self::MetadataUnchanged),
            2 => Ok(Self::MetadataChanged),
            3 => Ok(Self::Replaced),
            4 => Ok(Self::Moved),
            _ => Err("invalid source change"),
        }
    }
}
/// Metadata-only proposal: these stamps do not prove historical source identity.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistorySourceCandidate {
    pub path: Utf8PathBuf,
    pub aliases: Vec<Utf8PathBuf>,
    pub origin: u32,
    pub state: HistorySourceState,
    pub change: HistorySourceChange,
    pub moved_from: Option<Utf8PathBuf>,
    pub length: Option<u64>,
    pub modified_ticks: Option<u64>,
    pub character_hint_matches: bool,
    pub folder_hint_matches: bool,
    pub written_after_end: bool,
    pub message: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistorySourceDiagnostic {
    pub path: Utf8PathBuf,
    pub state: HistorySourceState,
    pub message: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistorySourceSnapshot {
    pub index_revision: String,
    pub history_id: String,
    pub start: String,
    pub end: String,
    pub character_hint: String,
    pub folder_hint: String,
    pub candidates: Vec<HistorySourceCandidate>,
    pub diagnostics: Vec<HistorySourceDiagnostic>,
    pub ambiguous: bool,
    pub missing: bool,
    pub incomplete: bool,
    pub requires_explicit_selection: bool,
}
/// Session-owned discovery. Drop does not cancel: release explicitly or shut down the Host.
#[derive(Debug)]
pub struct HistorySourceProposal {
    session: u64,
    owner: u64,
    revision: String,
    history: String,
    expected_range: ImportRange,
    query: HistorySourceQuery,
    snapshot: Option<HistorySourceSnapshot>,
}
impl HistorySourceProposal {
    pub fn snapshot(&self) -> Option<&HistorySourceSnapshot> {
        self.snapshot.as_ref()
    }
}
#[derive(Debug, Clone)]
pub struct HistoryCaptureOptions {
    pub text: Option<ImportTextOptions>,
    pub companions: Vec<CompanionSource>,
    pub maximum_bytes: u64,
}
impl Default for HistoryCaptureOptions {
    fn default() -> Self {
        Self {
            text: None,
            companions: vec![],
            maximum_bytes: MAXIMUM_BYTES,
        }
    }
}
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryCaptureProgress {
    pub bytes_copied: u64,
    pub total_bytes: u64,
    pub files_copied: u32,
    pub files_total: u32,
}
#[derive(Debug, Clone)]
pub struct CapturedHistoryFile {
    pub selected_name: String,
    pub path: Utf8PathBuf,
    pub name: String,
    pub length: u64,
    pub sha256: String,
    pub text: Option<ImportTextOptions>,
}
/// One pending/ready capture, distinct from metadata. Drop retains managed ownership until Host shutdown.
#[derive(Debug)]
pub struct HistorySourceCapture {
    session: u64,
    owner: u64,
    path: Utf8PathBuf,
    options: HistoryCaptureOptions,
    expected_range: ImportRange,
    files: Option<Vec<CapturedHistoryFile>>,
    range: Option<ImportRange>,
}
impl HistorySourceCapture {
    pub fn files(&self) -> Option<&[CapturedHistoryFile]> {
        self.files.as_deref()
    }
    pub fn range(&self) -> Option<ImportRange> {
        self.range
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Admission {
    owner: u64,
    pending: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProposalReply {
    owner: u64,
    pending: bool,
    snapshot: Option<HistorySourceSnapshot>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireFile {
    selected_name: String,
    path: Utf8PathBuf,
    name: String,
    length: u64,
    sha256: String,
    text: Option<WireText>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CaptureReply {
    owner: u64,
    pending: bool,
    progress: Option<HistoryCaptureProgress>,
    files: Option<Vec<WireFile>>,
    start_ticks: Option<u64>,
    end_ticks: Option<u64>,
}

pub(crate) fn packet(
    session: u64,
    command: u32,
    value: &impl Serialize,
) -> Result<BytesMut, Error> {
    let input = serde_json::to_vec(value).map_err(invalid)?;
    if input.len() > 128 * 1024 {
        return Err(invalid("source request exceeds 128 KiB"));
    }
    let mut packet = crate::history::poll_packet(session);
    packet
        .get_mut(16..20)
        .ok_or_else(|| invalid("missing command"))?
        .copy_from_slice(&command.to_le_bytes());
    packet
        .get_mut(20..24)
        .ok_or_else(|| invalid("missing input size"))?
        .copy_from_slice(&(input.len() as u32).to_le_bytes());
    packet
        .get_mut(32..32 + input.len())
        .ok_or_else(|| invalid("missing input body"))?
        .copy_from_slice(&input);
    Ok(packet)
}
pub(crate) fn reply<T: DeserializeOwned>(
    packet: &[u8],
    session: u64,
    command: u32,
) -> Result<T, Error> {
    if packet.len() != 32 + CAPACITY
        || packet.get(..4) != Some(((32 + CAPACITY) as u32).to_le_bytes().as_slice())
        || packet.get(4..8) != Some(1_u32.to_le_bytes().as_slice())
        || packet.get(8..16) != Some(session.to_le_bytes().as_slice())
        || packet.get(16..20) != Some(command.to_le_bytes().as_slice())
        || packet.get(28..32) != Some([0; 4].as_slice())
    {
        return Err(invalid("source response envelope mismatch"));
    }
    let length = u32::from_le_bytes(
        packet
            .get(24..28)
            .ok_or_else(|| invalid("missing output size"))?
            .try_into()
            .map_err(invalid)?,
    ) as usize;
    if length == 0 || length > CAPACITY {
        return Err(invalid("invalid source response length"));
    }
    serde_json::from_slice(
        packet
            .get(32..32 + length)
            .ok_or_else(|| invalid("missing output body"))?,
    )
    .map_err(invalid)
}
fn invalid(value: impl std::fmt::Display) -> Error {
    Error::InvalidHistoryRequest(value.to_string())
}
fn path_valid(path: &Utf8Path) -> bool {
    path.is_absolute()
        && path.as_str().len() <= 8192
        && !path.as_str().chars().any(char::is_control)
}
fn normalize_path(path: &Utf8Path) -> Utf8PathBuf {
    let mut normalized = Utf8PathBuf::new();
    for component in path.components() {
        match component {
            camino::Utf8Component::CurDir => {}
            camino::Utf8Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_str()),
        }
    }
    normalized
}
fn admitted(reply: Admission) -> Result<u64, Error> {
    if reply.owner == 0 || !reply.pending {
        return Err(invalid("invalid source admission"));
    }
    Ok(reply.owner)
}
impl HostHandle {
    async fn history_source_call<T: DeserializeOwned>(
        &self,
        command: u32,
        input: &impl Serialize,
    ) -> Result<T, Error> {
        let mut data = packet(self.session, command, input)?;
        if self.request(Request::History, &mut data).await? != 0 {
            return Err(invalid("unexpected source pending status"));
        }
        reply(&data, self.session, command)
    }
    pub async fn propose_history_sources(
        &self,
        revision: &HistoryRevision,
        id: &HistoryId,
        mut query: HistorySourceQuery,
    ) -> Result<HistorySourceProposal, Error> {
        if revision.session != self.session
            || id.session != self.session
            || id.digest != revision.digest
            || query.paths.len() > 16
            || query.roots.len() > 8
            || query
                .paths
                .iter()
                .chain(&query.roots)
                .any(|path| !path_valid(path))
            || query.filter.is_empty()
            || query.filter.len() > 256
            || query
                .filter
                .chars()
                .any(|c| c.is_control() || "/\\:".contains(c))
        {
            return Err(invalid("invalid source selection or bounded search scope"));
        }
        // Match Path.GetFullPath without reading the filesystem; missing candidates remain valid.
        query.paths = query
            .paths
            .iter()
            .map(|path| normalize_path(path))
            .collect();
        query.roots = query
            .roots
            .iter()
            .map(|path| normalize_path(path))
            .collect();
        let owner = admitted(self.history_source_call(3, &serde_json::json!({"selection":{"indexRevision":revision.digest,"ids":[id.file]},"query":query})).await?)?;
        Ok(HistorySourceProposal {
            session: self.session,
            owner,
            revision: revision.digest.clone(),
            history: id.file.clone(),
            expected_range: ImportRange::new(id.start_ticks, id.end_ticks)?,
            query,
            snapshot: None,
        })
    }
    pub async fn poll_history_sources(
        &self,
        proposal: &mut HistorySourceProposal,
    ) -> Result<bool, Error> {
        if proposal.session != self.session || proposal.owner == 0 {
            return Err(invalid("proposal belongs to another session"));
        }
        let response: ProposalReply = self
            .history_source_call(4, &serde_json::json!({"owner":proposal.owner}))
            .await?;
        if response.owner != proposal.owner || response.pending == response.snapshot.is_some() {
            return Err(invalid("contradictory proposal response"));
        }
        if let Some(snapshot) = response.snapshot {
            validate_snapshot(&snapshot, proposal)?;
            proposal.snapshot = Some(snapshot);
            Ok(true)
        } else {
            Ok(false)
        }
    }
    /// A failed release preserves the proposal so callers can release its capture and retry.
    pub async fn release_history_sources(
        &self,
        proposal: &mut HistorySourceProposal,
    ) -> Result<(), Error> {
        self.release_source(5, proposal.session, proposal.owner)
            .await?;
        proposal.owner = 0;
        Ok(())
    }
    async fn release_source(&self, command: u32, session: u64, owner: u64) -> Result<(), Error> {
        if session != self.session || owner == 0 {
            return Err(invalid(
                "source handle belongs to another session or is retired",
            ));
        }
        let released: bool = self
            .history_source_call(command, &serde_json::json!({"owner":owner}))
            .await?;
        if !released {
            return Err(invalid("source release was not acknowledged"));
        }
        Ok(())
    }
    pub async fn capture_history_source(
        &self,
        proposal: &HistorySourceProposal,
        path: impl AsRef<Utf8Path>,
        options: HistoryCaptureOptions,
    ) -> Result<HistorySourceCapture, Error> {
        let path = normalize_path(path.as_ref());
        let path = path.as_path();
        if proposal.session != self.session
            || options.maximum_bytes == 0
            || options.maximum_bytes > MAXIMUM_BYTES
            || !proposal.snapshot.as_ref().is_some_and(|snapshot| {
                snapshot.candidates.iter().any(|candidate| {
                    candidate.state == HistorySourceState::Available
                        && (candidate.path == path
                            || candidate.aliases.iter().any(|alias| alias == path))
                })
            })
        {
            return Err(invalid(
                "capture requires an explicitly selected available candidate and valid byte budget",
            ));
        }
        crate::companions::validate_sources(&options.companions, 64)?;
        let owner = admitted(self.history_source_call(6, &serde_json::json!({"owner":proposal.owner,"path":path,"text":options.text.map(WireText::from),"companions":options.companions.iter().map(WireSource::from).collect::<Vec<_>>(),"maximumBytes":options.maximum_bytes})).await?)?;
        if owner == proposal.owner {
            return Err(invalid("capture reused proposal owner"));
        }
        Ok(HistorySourceCapture {
            session: self.session,
            owner,
            path: path.to_owned(),
            options,
            expected_range: proposal.expected_range,
            files: None,
            range: None,
        })
    }
    /// None means ready. Some contains coalesced copy progress; failure remains explicitly releasable.
    pub async fn poll_history_capture(
        &self,
        capture: &mut HistorySourceCapture,
    ) -> Result<Option<HistoryCaptureProgress>, Error> {
        if capture.session != self.session || capture.owner == 0 {
            return Err(invalid("capture belongs to another session"));
        }
        let response: CaptureReply = self
            .history_source_call(7, &serde_json::json!({"owner":capture.owner}))
            .await?;
        validate_capture(response, capture)
    }
    /// Cancels pending capture or disposes a ready image, joining input cleanup before replying.
    /// Busy/foreign-session rejection preserves this handle. A managed terminal release failure
    /// is reported after ownership retirement; an ensuing stale-owner response is expected.
    pub async fn release_history_capture(
        &self,
        capture: &mut HistorySourceCapture,
    ) -> Result<(), Error> {
        self.release_source(8, capture.session, capture.owner)
            .await?;
        capture.owner = 0;
        Ok(())
    }
    /// Transfers a ready capture once. A failed preflight preserves this handle for release/retry.
    /// Dropped admission futures retain admitted work in Host; Stop drains it.
    pub async fn reparse_history_capture(
        &self,
        capture: &mut HistorySourceCapture,
    ) -> Result<ImportJob, Error> {
        if capture.session != self.session || capture.owner == 0 || capture.files.is_none() {
            return Err(invalid(
                "reparse requires a ready capture from this session",
            ));
        }
        let job = self.start_captured_import(capture.owner).await?;
        capture.owner = 0;
        Ok(job)
    }
}
fn validate_snapshot(
    snapshot: &HistorySourceSnapshot,
    proposal: &HistorySourceProposal,
) -> Result<(), Error> {
    let authorized = |path: &Utf8Path| {
        proposal.query.paths.iter().any(|p| p == path)
            || path
                .parent()
                .is_some_and(|parent| proposal.query.roots.iter().any(|root| root == parent))
    };
    let mut seen = std::collections::HashSet::new();
    if snapshot.index_revision != proposal.revision
        || snapshot.history_id != proposal.history
        || !snapshot.requires_explicit_selection
        || snapshot.candidates.len() > 64
        || snapshot.diagnostics.len() > 8
    {
        return Err(invalid("source snapshot identity or bounds mismatch"));
    }
    let available = snapshot
        .candidates
        .iter()
        .filter(|item| item.state == HistorySourceState::Available)
        .count();
    let incomplete = snapshot
        .diagnostics
        .iter()
        .any(|item| item.state == HistorySourceState::Unavailable)
        || snapshot
            .candidates
            .iter()
            .any(|item| item.state == HistorySourceState::Unavailable);
    if snapshot.ambiguous != (available > 1)
        || snapshot.incomplete != incomplete
        || snapshot.missing != (!incomplete && available == 0)
    {
        return Err(invalid("contradictory source snapshot flags"));
    }
    let mut paths = 0;
    for candidate in &snapshot.candidates {
        paths += 1 + candidate.aliases.len();
        if paths > 64
            || !(1..=3).contains(&candidate.origin)
            || candidate.state == HistorySourceState::Available
                && (candidate.length.is_none() || candidate.modified_ticks.is_none())
        {
            return Err(invalid("invalid candidate metadata"));
        }
        if candidate.message.encode_utf16().count() > 2048
            || candidate
                .length
                .is_some_and(|value| value > i64::MAX as u64)
            || candidate
                .modified_ticks
                .is_some_and(|value| value > 3_155_378_975_999_999_999)
            || (candidate.state != HistorySourceState::Available
                && (candidate.length.is_some() || candidate.modified_ticks.is_some()))
            || candidate
                .moved_from
                .as_ref()
                .is_some_and(|path| !path_valid(path) || !authorized(path))
            || (candidate.change == HistorySourceChange::Moved) != candidate.moved_from.is_some()
        {
            return Err(invalid("invalid source observation"));
        }
        for path in std::iter::once(&candidate.path).chain(&candidate.aliases) {
            if !path_valid(path) || !authorized(path) || !seen.insert(path) {
                return Err(invalid("source candidate outside scope or duplicated"));
            }
        }
    }
    for item in &snapshot.diagnostics {
        if item.message.encode_utf16().count() > 2048
            || !proposal.query.roots.contains(&item.path)
            || item.state == HistorySourceState::Available
        {
            return Err(invalid("invalid source diagnostic"));
        }
    }
    Ok(())
}
fn validate_capture(
    response: CaptureReply,
    capture: &mut HistorySourceCapture,
) -> Result<Option<HistoryCaptureProgress>, Error> {
    if response.owner != capture.owner {
        return Err(invalid("capture owner mismatch"));
    }
    if response.pending {
        let progress = response
            .progress
            .ok_or_else(|| invalid("missing capture progress"))?;
        if response.files.is_some()
            || response.start_ticks.is_some()
            || response.end_ticks.is_some()
            || progress.bytes_copied > progress.total_bytes
            || progress.total_bytes > capture.options.maximum_bytes
            || progress.files_copied > progress.files_total
            || progress.files_total as usize > capture.options.companions.len() + 1
        {
            return Err(invalid("invalid capture progress"));
        }
        return Ok(Some(progress));
    }
    if response.progress.is_some() {
        return Err(invalid("ready capture has progress"));
    }
    let range = ImportRange::new(
        response
            .start_ticks
            .ok_or_else(|| invalid("missing start"))?,
        response.end_ticks.ok_or_else(|| invalid("missing end"))?,
    )?;
    if range != capture.expected_range {
        return Err(invalid("captured history range mismatch"));
    }
    let files = response
        .files
        .ok_or_else(|| invalid("missing capture files"))?;
    if files.len() != capture.options.companions.len() + 1 {
        return Err(invalid("captured source count mismatch"));
    }
    let mut total = 0_u64;
    let mut names = std::collections::HashSet::new();
    let mut output = Vec::new();
    for (index, file) in files.into_iter().enumerate() {
        total = total
            .checked_add(file.length)
            .ok_or_else(|| invalid("capture size overflow"))?;
        if total > capture.options.maximum_bytes {
            return Err(invalid("captured byte budget exceeded"));
        }
        output.push(decode_file(file, index, capture, &mut names)?);
    }
    capture.range = Some(range);
    capture.files = Some(output);
    Ok(None)
}

fn decode_file(
    file: WireFile,
    index: usize,
    capture: &HistorySourceCapture,
    names: &mut std::collections::HashSet<String>,
) -> Result<CapturedHistoryFile, Error> {
    let expected = if index == 0 {
        if !file.name.is_empty() || !file.selected_name.is_empty() || file.path != capture.path {
            return Err(invalid("captured main identity mismatch"));
        }
        capture.options.text
    } else {
        let selected = capture
            .options
            .companions
            .iter()
            .find(|selected| selected.name == file.selected_name)
            .ok_or_else(|| invalid("unexpected captured companion"))?;
        if !names.insert(selected.name.to_ascii_lowercase()) {
            return Err(invalid("duplicate captured companion"));
        }
        let parent = capture
            .path
            .parent()
            .ok_or_else(|| invalid("missing source parent"))?
            .join("CompanionLogs");
        // The managed catalog owns OrdinalIgnoreCase prefix/extension matching, including Unicode.
        // Validate the actual companion component and authorized sibling directory, as live sources do.
        let physical_stem = if capture
            .path
            .extension()
            .is_some_and(|value| !value.is_empty())
        {
            file.path.file_stem()
        } else {
            file.path.file_name()
        }
        .and_then(|stem| stem.rsplit_once('.'));
        if file.path.parent() != Some(parent.as_path())
            || !physical_stem.is_some_and(|(main, name)| !main.is_empty() && name == file.name)
        {
            return Err(invalid("captured companion path/name mismatch"));
        }
        selected.text
    };
    if !path_valid(&file.path) || !crate::history::valid_digest(&file.sha256) {
        return Err(invalid("invalid captured file"));
    }
    let text = file.text.map(ImportTextOptions::try_from).transpose()?;
    if text != expected {
        return Err(invalid("captured encoding mismatch"));
    }
    Ok(CapturedHistoryFile {
        selected_name: file.selected_name,
        path: file.path,
        name: file.name,
        length: file.length,
        sha256: file.sha256,
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn range() -> ImportRange {
        ImportRange::new(10, 20).expect("range")
    }
    fn capture() -> HistorySourceCapture {
        HistorySourceCapture {
            session: 1,
            owner: 2,
            path: "/tmp/main.log".into(),
            expected_range: range(),
            options: HistoryCaptureOptions {
                companions: vec![CompanionSource {
                    name: "ALPHA".into(),
                    text: None,
                }],
                ..Default::default()
            },
            files: None,
            range: None,
        }
    }
    fn ready() -> serde_json::Value {
        serde_json::json!({"owner":2,"pending":false,"startTicks":10,"endTicks":20,"files":[
            {"selectedName":"","path":"/tmp/main.log","name":"","length":10,"sha256":"a".repeat(64),"text":null},
            {"selectedName":"ALPHA","path":"/tmp/CompanionLogs/main.Alpha.log","name":"Alpha","length":20,"sha256":"b".repeat(64),"text":null}]})
    }
    #[test]
    fn capture_replies_reject_wrong_owner_range_path_selection_codec_and_bounds() {
        let decode =
            |value| validate_capture(serde_json::from_value(value).expect("wire"), &mut capture());
        assert!(matches!(decode(ready()), Ok(None)));
        for (pointer, value) in [
            ("/owner", serde_json::json!(3)),
            ("/startTicks", serde_json::json!(11)),
            ("/pending", serde_json::json!(true)),
            ("/files/0/name", serde_json::json!("Alpha")),
            ("/files/1/selectedName", serde_json::json!("BETA")),
            (
                "/files/1/path",
                serde_json::json!("/tmp/else/main.Alpha.log"),
            ),
            (
                "/files/1/path",
                serde_json::json!("/tmp/CompanionLogs/main.Beta.log"),
            ),
            ("/files/1/sha256", serde_json::json!("x".repeat(64))),
            ("/files/1/length", serde_json::json!(MAXIMUM_BYTES)),
            (
                "/files/1/text",
                serde_json::json!({"codePage":65001,"replaceInvalid":false,"detectBom":true}),
            ),
        ] {
            let mut bad = ready();
            *bad.pointer_mut(pointer).expect("field") = value;
            assert!(decode(bad).is_err(), "accepted {pointer}");
        }
        let pending = serde_json::json!({"owner":2,"pending":true,"progress":{"bytesCopied":1,"totalBytes":30,"filesCopied":0,"filesTotal":2}});
        assert!(matches!(decode(pending.clone()), Ok(Some(_))));
        for (field, value) in [("bytesCopied", 31), ("filesTotal", 3), ("filesCopied", 3)] {
            let mut bad = pending.clone();
            bad["progress"][field] = serde_json::json!(value);
            decode(bad).expect_err("invalid response");
        }
        // ACT's extensionless companion has a final dot component but no main extension.
        let mut no_extension = capture();
        no_extension.path = "/tmp/main".into();
        let mut wire = ready();
        wire["files"][0]["path"] = serde_json::json!("/tmp/main");
        wire["files"][1]["path"] = serde_json::json!("/tmp/CompanionLogs/main.Alpha");
        validate_capture(
            serde_json::from_value(wire).expect("extensionless"),
            &mut no_extension,
        )
        .expect("extensionless captured path");
    }
    #[test]
    fn response_envelope_rejects_stale_session_command_truncation_and_invalid_json() {
        let mut data = packet(1, 7, &serde_json::json!({"owner":2})).expect("packet");
        let wire = serde_json::to_vec(&ready()).expect("json");
        data[24..28].copy_from_slice(&(wire.len() as u32).to_le_bytes());
        data[32..32 + wire.len()].copy_from_slice(&wire);
        reply::<CaptureReply>(&data, 1, 7).expect("valid reply");
        for offset in [0, 4, 8, 16, 28] {
            let mut bad = data.clone();
            bad[offset] ^= 1;
            assert!(reply::<CaptureReply>(&bad, 1, 7).is_err());
        }
        let mut bad = data.clone();
        bad[24..28].copy_from_slice(&((CAPACITY + 1) as u32).to_le_bytes());
        assert!(reply::<CaptureReply>(&bad, 1, 7).is_err());
        assert!(reply::<CaptureReply>(&data[..data.len() - 1], 1, 7).is_err());
        data[32] = 0xff;
        assert!(reply::<CaptureReply>(&data, 1, 7).is_err());
    }
    #[test]
    fn proposals_reject_unrequested_paths_foreign_history_and_contradictory_flags() {
        let proposal = HistorySourceProposal {
            session: 1,
            owner: 1,
            revision: "a".repeat(64),
            history: format!("{}.act", "b".repeat(64)),
            expected_range: range(),
            query: HistorySourceQuery {
                paths: vec!["/tmp/main.log".into()],
                ..Default::default()
            },
            snapshot: None,
        };
        let wire = serde_json::json!({"indexRevision":proposal.revision,"historyId":proposal.history,"start":"start","end":"end","characterHint":"","folderHint":"","candidates":[{
            "path":"/tmp/main.log","aliases":[],"origin":1,"state":0,"change":0,"movedFrom":null,"length":10,"modifiedTicks":10,"characterHintMatches":false,"folderHintMatches":false,"writtenAfterEnd":false,"message":""}],"diagnostics":[],"ambiguous":false,"missing":false,"incomplete":false,"requiresExplicitSelection":true});
        let decode =
            |value| validate_snapshot(&serde_json::from_value(value).expect("snapshot"), &proposal);
        decode(wire.clone()).expect("valid snapshot");
        for (pointer, value) in [
            ("/historyId", serde_json::json!("foreign.act")),
            ("/ambiguous", serde_json::json!(true)),
            ("/requiresExplicitSelection", serde_json::json!(false)),
            ("/candidates/0/path", serde_json::json!("/etc/foreign.log")),
            ("/candidates/0/length", serde_json::json!(u64::MAX)),
        ] {
            let mut bad = wire.clone();
            *bad.pointer_mut(pointer).expect("field") = value;
            decode(bad).expect_err("invalid response");
        }
    }
}
