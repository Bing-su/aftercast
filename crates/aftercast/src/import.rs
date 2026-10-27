use crate::companions::{self, CompanionSource, WireSource, WireText};
use bytes::{Buf, BufMut, BytesMut};
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::thread::Thread;
use tokio::sync::{oneshot, watch};

use crate::process::RemoteSession;
use crate::runtime::{Request, canonicalize};
use crate::{Error, HostHandle};

/// Latest input progress; read-ahead bytes can lead parsed records by the bounded input buffer.
/// Clipboard byte counts measure the admitted Unicode snapshot's UTF-8 equivalent, not disk I/O.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportProgress {
    pub lines_read: u64,
    pub records_dispatched: u64,
    pub bytes_read: u64,
    pub total_bytes: u64,
    /// Framed future companion heads excluded at the terminal main watermark, not unread suffix size.
    pub companion_heads_excluded: u64,
}

/// Explicit supported source encoding; no locale or byte-content guess is made.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum ImportEncoding {
    #[default]
    Utf8 = 65001,
    Utf16Le = 1200,
    Utf16Be = 1201,
    Utf32Le = 12000,
    Utf32Be = 12001,
    ShiftJis = 932,
    WindowsKorean = 949,
    Windows1252 = 1252,
    Ascii = 20127,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ImportInvalidBytes {
    #[default]
    Strict,
    /// Use the chosen framework encoding's standard replacement, which can change source text.
    Replace,
}
/// An owned immutable request snapshot. Default is UTF-8 strict with Unicode BOM detection.
/// This differs from an omitted option, which inherits the plugin's LogEncoding at admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportTextOptions {
    pub encoding: ImportEncoding,
    pub invalid_bytes: ImportInvalidBytes,
    /// The selected encoding's own preamble is consumed even when other BOM detection is disabled.
    pub detect_bom: bool,
}
impl Default for ImportTextOptions {
    fn default() -> Self {
        Self {
            encoding: ImportEncoding::Utf8,
            invalid_bytes: ImportInvalidBytes::Strict,
            detect_bom: true,
        }
    }
}
impl ImportTextOptions {
    fn encode(self, packet: &mut [u8]) -> Result<(), Error> {
        let mut fields = packet
            .get_mut(40..48)
            .ok_or_else(|| Error::InvalidImportRequest("missing text fields".into()))?;
        fields.put_u32_le(self.encoding as u32);
        let flags = u32::from(self.invalid_bytes == ImportInvalidBytes::Replace)
            | (u32::from(!self.detect_bom) << 1);
        fields.put_u32_le(flags);
        Ok(())
    }
}

/// Inclusive source wall-clock bounds for ACT history reparse, in 100 ns ticks since year 0001.
/// No timezone conversion is performed. Extension is explicit and defaults to false.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportRange {
    start_ticks: u64,
    end_ticks: u64,
    extend_to_combat_end: bool,
}
impl ImportRange {
    pub fn new(start_ticks: u64, end_ticks: u64) -> Result<Self, Error> {
        if start_ticks > end_ticks || end_ticks > 3_155_378_975_999_999_999 {
            return Err(Error::InvalidImportRequest(
                "invalid inclusive time range".into(),
            ));
        }
        Ok(Self {
            start_ticks,
            end_ticks,
            extend_to_combat_end: false,
        })
    }
    /// None selects the file beginning/end using ACT's idle-import MinValue/MaxValue bounds.
    pub fn from_bounds(
        start_ticks: Option<u64>,
        end_ticks: Option<u64>,
        extend_to_combat_end: bool,
    ) -> Result<Self, Error> {
        Self::new(
            start_ticks.unwrap_or(0),
            end_ticks.unwrap_or(3_155_378_975_999_999_999),
        )
        .map(|range| range.with_combat_end_extension(extend_to_combat_end))
    }
    /// Permit a post-end row only while combat is active before parsing that row.
    /// Such a row can end the old encounter and start another; this is not an encounter-ID filter.
    pub fn with_combat_end_extension(mut self, enabled: bool) -> Self {
        self.extend_to_combat_end = enabled;
        self
    }
    pub fn extend_to_combat_end(self) -> bool {
        self.extend_to_combat_end
    }
    pub fn start_ticks(self) -> u64 {
        self.start_ticks
    }
    pub fn end_ticks(self) -> u64 {
        self.end_ticks
    }
}

/// One admitted import. Dropping this handle does not cancel it; cancellation is explicit.
pub struct ImportJob {
    pub(crate) cancellation: Arc<AtomicBool>,
    pub(crate) thread: Thread,
    pub(crate) progress: watch::Receiver<ImportProgress>,
    pub(crate) result: oneshot::Receiver<Result<ImportReport, Error>>,
    pub(crate) control: Arc<crate::host::Control>,
}

impl ImportJob {
    pub fn cancel(&self) {
        self.cancellation.store(true, Ordering::Release);
        self.thread.unpark();
    }
    pub fn progress(&self) -> ImportProgress {
        *self.progress.borrow()
    }
    /// Completes only after input cleanup, action draining and live-state restoration.
    pub async fn wait(self) -> Result<ImportReport, Error> {
        self.await
    }
}

impl Future for ImportJob {
    type Output = Result<ImportReport, Error>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        // Borrowing the job as a future keeps progress/cancel available between polls, e.g. a GUI timer.
        match Pin::new(&mut self.result).poll(context) {
            Poll::Ready(Ok(result)) => Poll::Ready(result),
            Poll::Ready(Err(_closed)) => Poll::Ready(Err(self.control.error())),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Fixed text snapshot budget, measured in UTF-8 bytes; this library never reads an OS clipboard.
pub const MAX_CLIPBOARD_IMPORT_BYTES: usize = 8 * 1024 * 1024;

pub(crate) enum ImportInput {
    File(Utf8PathBuf),
    Clipboard(String),
    Captured(u64),
}

pub(crate) fn validate_clipboard(content: &str) -> Result<(), Error> {
    if content.is_empty() || content.len() > MAX_CLIPBOARD_IMPORT_BYTES {
        return Err(Error::InvalidImportRequest(
            "clipboard text must be nonempty and at most 8 MiB of UTF-8 bytes".into(),
        ));
    }
    Ok(())
}

pub(crate) struct PendingImport {
    pub(crate) range: Option<ImportRange>,
    pub(crate) text: Option<ImportTextOptions>,
    pub(crate) sources: Option<Vec<CompanionSource>>,
    pub(crate) source_owner: Option<u64>,
    pub(crate) cancellation: Arc<AtomicBool>,
    pub(crate) progress: watch::Sender<ImportProgress>,
    pub(crate) complete: oneshot::Sender<Result<ImportReport, Error>>,
}

impl PendingImport {
    pub(crate) fn begin(
        &mut self,
        session: &mut RemoteSession,
        input: ImportInput,
    ) -> Result<(), Error> {
        let path = match input {
            ImportInput::Captured(owner) => {
                let mut packet = crate::history_sources::packet(
                    session.id(),
                    9,
                    &serde_json::json!({"owner":owner}),
                )?;
                session.request(Request::History, &mut packet)?;
                let reply: SourceReply = crate::history_sources::reply(&packet, session.id(), 9)?;
                self.source_owner = Some(admission_owner(reply)?);
                return Ok(());
            }
            ImportInput::Clipboard(content) => {
                let mut packet = clipboard_packet(session.id(), &content)?;
                session.request(Request::ImportOperation, &mut packet)?;
                return Ok(());
            }
            ImportInput::File(path) => path,
        };
        let path = canonicalize(&path)?;
        if let Some(sources) = &self.sources {
            if path.as_str().len() > 8192 || path.as_str().chars().any(char::is_control) {
                return Err(companions::invalid("invalid companion import path"));
            }
            let mut packet = companions::packet(
                session.id(),
                1,
                &serde_json::json!({"path":path,"text":self.text.map(WireText::from),
                "companions":sources.iter().map(WireSource::from).collect::<Vec<_>>(),"startTicks":self.range.map(ImportRange::start_ticks),"endTicks":self.range.map(ImportRange::end_ticks),"extendToCombatEnd":self.range.map(ImportRange::extend_to_combat_end)}),
            )?;
            session.request(Request::Companions, &mut packet)?;
            let reply: SourceReply = companions::reply(&packet, session.id(), 1)?;
            self.source_owner = Some(admission_owner(reply)?);
            return Ok(());
        }
        let mut packet = import_packet(session.id(), path.as_str())?;
        if let Some(text) = self.text {
            text.encode(&mut packet)?;
        }
        if let Some(range) = self.range {
            packet
                .get_mut(24..32)
                .ok_or(Error::Closed)?
                .copy_from_slice(&range.start_ticks.to_le_bytes());
            packet
                .get_mut(32..40)
                .ok_or(Error::Closed)?
                .copy_from_slice(&range.end_ticks.to_le_bytes());
            packet
                .get_mut(60..64)
                .ok_or(Error::Closed)?
                .copy_from_slice(&u32::from(range.extend_to_combat_end).to_le_bytes());
        }
        packet
            .get_mut(56..60)
            .ok_or(Error::Closed)?
            .copy_from_slice(&(if self.range.is_some() { 4_u32 } else { 1_u32 }).to_le_bytes());
        session.request(Request::ImportOperation, &mut packet)?;
        Ok(())
    }

    pub(crate) fn advance(
        &mut self,
        session: &mut RemoteSession,
        stopping: bool,
    ) -> Option<Result<ImportReport, Error>> {
        if let Some(owner) = self.source_owner {
            return match self.advance_sources(session, owner, stopping) {
                Ok(step) => step,
                Err(error) => Some(Err(error)),
            };
        }
        let result = (|| {
            let mut packet = import_packet(session.id(), "")?;
            let command: u32 = if stopping || self.cancellation.load(Ordering::Acquire) {
                3
            } else {
                2
            };
            packet
                .get_mut(56..60)
                .ok_or(Error::Closed)?
                .copy_from_slice(&command.to_le_bytes());
            match session.request(Request::ImportOperation, &mut packet)? {
                0 => Ok(Some(decode_import(&packet))),
                2 => Ok(Some(Err(Error::Cancelled))),
                _ => {
                    let progress = decode_progress(&packet)?;
                    self.progress.send_replace(progress);
                    Ok(None)
                }
            }
        })();
        match result {
            Ok(result) => result,
            Err(error) => Some(Err(error)),
        }
    }
    fn advance_sources(
        &self,
        session: &mut RemoteSession,
        owner: u64,
        stopping: bool,
    ) -> Result<Option<Result<ImportReport, Error>>, Error> {
        let command = if stopping || self.cancellation.load(Ordering::Acquire) {
            3
        } else {
            2
        };
        let mut packet =
            companions::packet(session.id(), command, &serde_json::json!({"owner":owner}))?;
        session.request(Request::Companions, &mut packet)?;
        let reply = companions::reply(&packet, session.id(), command)?;
        match source_step(reply, owner)? {
            SourceStep::Progress(progress) => {
                self.progress.send_replace(progress);
                Ok(None)
            }
            SourceStep::Complete(report) => Ok(Some(Ok(report))),
            SourceStep::Cancelled => Ok(Some(Err(Error::Cancelled))),
        }
    }
}

fn decode_progress(packet: &[u8]) -> Result<ImportProgress, Error> {
    let mut fields = packet
        .get(24..56)
        .ok_or_else(|| Error::InvalidImportResponse("missing progress".into()))?;
    let progress = ImportProgress {
        lines_read: fields.get_u64_le(),
        records_dispatched: fields.get_u64_le(),
        bytes_read: fields.get_u64_le(),
        total_bytes: fields.get_u64_le(),
        companion_heads_excluded: 0,
    };
    if progress.records_dispatched > progress.lines_read
        || progress.bytes_read > progress.total_bytes
        || progress.total_bytes > i64::MAX as u64
    {
        return Err(Error::InvalidImportResponse(
            "invalid progress counters".into(),
        ));
    }
    Ok(progress)
}

/// An owned summary of one completed file import, after accepted actions have drained.
///
/// Dispatch counts describe transport, not plugin acceptance. A plugin can catch its own errors;
/// inspect its diagnostics before treating these totals as a validated gameplay analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportReport {
    /// File records read, including empty records.
    pub lines_read: u64,
    /// Nonempty records delivered to import hooks.
    pub records_dispatched: u64,
    /// New individual encounters retained by this import.
    pub encounters: u32,
    /// Sum of combatant counts in those encounters, including repeated participants.
    pub combatants: u32,
    /// Allied damage across those encounters, using plugin-registered categories.
    pub damage: i64,
    /// Allied healing across those encounters, using plugin-registered categories.
    pub healed: i64,
    /// Known rejected heads only; unread companion suffix records are not counted.
    pub companion_heads_excluded: u64,
}

fn clipboard_packet(session: u64, content: &str) -> Result<BytesMut, Error> {
    validate_clipboard(content)?;
    let mut packet = import_packet(session, "")?;
    packet
        .get_mut(..4)
        .ok_or(Error::Closed)?
        .copy_from_slice(&((64 + content.len()) as u32).to_le_bytes());
    packet
        .get_mut(20..24)
        .ok_or(Error::Closed)?
        .copy_from_slice(&(content.len() as u32).to_le_bytes());
    packet
        .get_mut(56..60)
        .ok_or(Error::Closed)?
        .copy_from_slice(&5_u32.to_le_bytes());
    packet.extend_from_slice(content.as_bytes());
    Ok(packet)
}

pub(crate) fn import_packet(session: u64, path: &str) -> Result<BytesMut, Error> {
    let size = 64_usize
        .checked_add(path.len())
        .filter(|size| *size <= 65536);
    let size = size.filter(|_| !path.contains('\0')).ok_or_else(|| {
        Error::InvalidImportRequest("NUL or request exceeds 65536 bytes".to_owned())
    })?;
    let mut packet = BytesMut::with_capacity(size);
    packet.put_u32_le(size as u32);
    packet.put_u32_le(1);
    packet.put_u64_le(session);
    packet.put_u32_le(64);
    packet.put_u32_le(path.len() as u32);
    packet.resize(64, 0);
    packet.extend_from_slice(path.as_bytes());
    Ok(packet)
}

pub(crate) fn decode_import(packet: &[u8]) -> Result<ImportReport, Error> {
    let invalid = |message: &str| Error::InvalidImportResponse(message.to_owned());
    let mut result = packet
        .get(24..64)
        .ok_or_else(|| invalid("missing import result"))?;
    let lines_read = u64::try_from(result.get_i64_le())
        .map_err(|_negative_count| invalid("negative line count"))?;
    let records_dispatched = u64::try_from(result.get_i64_le())
        .map_err(|_negative_count| invalid("negative dispatch count"))?;
    let encounters = u32::try_from(result.get_i32_le())
        .map_err(|_negative_count| invalid("negative encounter count"))?;
    let combatants = u32::try_from(result.get_i32_le())
        .map_err(|_negative_count| invalid("negative combatant count"))?;
    let damage = result.get_i64_le();
    let healed = result.get_i64_le();
    if records_dispatched > lines_read {
        return Err(invalid("invalid import counts"));
    }
    Ok(ImportReport {
        lines_read,
        records_dispatched,
        encounters,
        combatants,
        damage,
        healed,
        companion_heads_excluded: 0,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceReply {
    owner: u64,
    pending: bool,
    cancelled: bool,
    progress: Option<ImportProgress>,
    result: Option<ImportReport>,
}
fn admission_owner(reply: SourceReply) -> Result<u64, Error> {
    if reply.owner == 0
        || !reply.pending
        || reply.cancelled
        || reply.progress.is_some()
        || reply.result.is_some()
    {
        return Err(companions::invalid("invalid source import admission"));
    }
    Ok(reply.owner)
}
enum SourceStep {
    Progress(ImportProgress),
    Complete(ImportReport),
    Cancelled,
}
fn source_step(reply: SourceReply, owner: u64) -> Result<SourceStep, Error> {
    if reply.owner != owner || owner == 0 {
        return Err(companions::invalid("source import owner mismatch"));
    }
    match (reply.pending, reply.cancelled, reply.progress, reply.result) {
        (true, false, Some(progress), None) => {
            validate_source_counts(
                progress.lines_read,
                progress.records_dispatched,
                progress.companion_heads_excluded,
            )?;
            if progress.bytes_read > progress.total_bytes || progress.total_bytes > i64::MAX as u64
            {
                return Err(companions::invalid("invalid source byte progress"));
            }
            Ok(SourceStep::Progress(progress))
        }
        (false, false, None, Some(report)) => {
            validate_source_counts(
                report.lines_read,
                report.records_dispatched,
                report.companion_heads_excluded,
            )?;
            if report.encounters > i32::MAX as u32 || report.combatants > i32::MAX as u32 {
                return Err(companions::invalid("invalid source graph counts"));
            }
            Ok(SourceStep::Complete(report))
        }
        (false, true, None, None) => Ok(SourceStep::Cancelled),
        _ => Err(companions::invalid("contradictory source import reply")),
    }
}
fn validate_source_counts(lines: u64, dispatched: u64, excluded: u64) -> Result<(), Error> {
    if lines > i64::MAX as u64
        || dispatched > lines
        || excluded > 64
        || excluded > lines - dispatched
    {
        return Err(companions::invalid("invalid source import counters"));
    }
    Ok(())
}

impl HostHandle {
    /// Imports an explicit Unicode snapshot without reading a clipboard or applying file options.
    /// Uses StringReader framing and pathless provenance within the fixed UTF-8 byte budget.
    pub async fn import_clipboard_text(
        &self,
        content: impl AsRef<str>,
    ) -> Result<ImportReport, Error> {
        self.start_clipboard_import(content).await?.wait().await
    }

    pub async fn import_sources(
        &self,
        path: impl AsRef<Utf8Path>,
        options: crate::SourceImportOptions,
    ) -> Result<ImportReport, Error> {
        self.start_import_sources(path, options).await?.wait().await
    }
    /// Reparse the selected inclusive time range through initialized plugins.
    /// Without combat extension, stops at the first later timestamp; same-time rows remain distinct.
    /// Shares the full import
    /// reader, cancellation, partial-result retention and live-state restoration contracts.
    pub async fn import_log_range(
        &self,
        path: impl AsRef<Utf8Path>,
        range: ImportRange,
    ) -> Result<ImportReport, Error> {
        self.start_import_range(path, range).await?.wait().await
    }

    /// Imports captured file contents through initialized plugins using their LogEncoding (default strict UTF-8).
    ///
    /// Call while live combat is idle. Live file delivery is paused and
    /// restored, including after failure. Appends after opening are excluded. Strict decoding errors,
    /// records over 8 MiB and premature truncation fail visibly. Partial imported history remains
    /// after failure. Hook delivery alone does not establish that a plugin accepted the records.
    pub async fn import_log(&self, path: impl AsRef<Utf8Path>) -> Result<ImportReport, Error> {
        self.start_import(path).await?.wait().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_packets_bound_utf8_and_preserve_text_instead_of_path_rules() {
        for text in ["한글日本語😀\0\r\n", "\u{feff}line", " "] {
            let packet = clipboard_packet(7, text).expect("Unicode snapshot");
            assert_eq!(packet.get(64..), Some(text.as_bytes()));
            assert_eq!(packet.get(56..60), Some(5_u32.to_le_bytes().as_slice()));
            assert_eq!(packet.get(24..56), Some([0_u8; 32].as_slice()));
        }
        let maximum = "x".repeat(MAX_CLIPBOARD_IMPORT_BYTES);
        assert_eq!(
            clipboard_packet(1, &maximum).expect("exact budget").len(),
            64 + MAX_CLIPBOARD_IMPORT_BYTES
        );
        for text in [
            String::new(),
            maximum + "x",
            "한".repeat(MAX_CLIPBOARD_IMPORT_BYTES / 3 + 1),
        ] {
            assert!(matches!(
                clipboard_packet(1, &text),
                Err(Error::InvalidImportRequest(_))
            ));
        }
    }

    #[test]
    fn owned_source_steps_reject_stale_contradictory_or_unbounded_replies() {
        let progress = serde_json::json!({"owner":1,"pending":true,"cancelled":false,"progress":{
            "linesRead":3,"recordsDispatched":2,"bytesRead":10,"totalBytes":20,"companionHeadsExcluded":1},"result":null});
        let decode = |value| source_step(serde_json::from_value(value).expect("wire step"), 1);
        assert!(matches!(
            decode(progress.clone()),
            Ok(SourceStep::Progress(_))
        ));
        for (field, value) in [
            ("owner", serde_json::json!(2)),
            ("cancelled", serde_json::json!(true)),
            ("pending", serde_json::json!(false)),
        ] {
            let mut bad = progress.clone();
            bad[field] = value;
            assert!(matches!(
                decode(bad),
                Err(Error::InvalidCompanionRequest(_))
            ));
        }
        for (field, value) in [
            ("recordsDispatched", 4),
            ("companionHeadsExcluded", 65),
            ("bytesRead", 21),
        ] {
            let mut bad = progress.clone();
            bad["progress"][field] = serde_json::json!(value);
            assert!(matches!(
                decode(bad),
                Err(Error::InvalidCompanionRequest(_))
            ));
        }
        let complete = serde_json::json!({"owner":1,"pending":false,"cancelled":false,"progress":null,
            "result":{"linesRead":3,"recordsDispatched":2,"encounters":1,"combatants":2,"damage":-1,"healed":0,"companionHeadsExcluded":1}});
        assert!(matches!(
            decode(complete.clone()),
            Ok(SourceStep::Complete(_))
        ));
        let mut bad = complete;
        bad["result"]["combatants"] = serde_json::json!(u32::MAX);
        assert!(matches!(
            decode(bad),
            Err(Error::InvalidCompanionRequest(_))
        ));
    }

    #[test]
    fn source_ranges_validate_inclusive_wall_clock_bounds() {
        let maximum = 3_155_378_975_999_999_999;
        for (start, end) in [(0, 0), (0, maximum), (maximum, maximum)] {
            let range = ImportRange::new(start, end).expect("valid inclusive bounds");
            assert_eq!((range.start_ticks(), range.end_ticks()), (start, end));
        }
        ImportRange::new(2, 1).expect_err("reversed range");
        ImportRange::new(0, maximum + 1).expect_err("out of DateTime range");
    }

    #[test]
    fn file_bounds_and_combat_extension_are_explicit_value_snapshots() {
        let maximum = 3_155_378_975_999_999_999;
        let full = ImportRange::from_bounds(None, None, false).expect("effective file bounds");
        assert_eq!((full.start_ticks(), full.end_ticks()), (0, maximum));
        assert!(!full.extend_to_combat_end());
        let extended = full.with_combat_end_extension(true);
        assert!(extended.extend_to_combat_end());
        assert!(!full.extend_to_combat_end());
        assert_eq!(extended.with_combat_end_extension(false), full);
        assert_eq!(
            ImportRange::from_bounds(Some(7), None, true)
                .expect("file end")
                .end_ticks(),
            maximum
        );
        assert_eq!(
            ImportRange::from_bounds(None, Some(7), true)
                .expect("file start")
                .start_ticks(),
            0
        );
        ImportRange::from_bounds(Some(8), Some(7), true).expect_err("reversed bounds");
        ImportRange::from_bounds(None, Some(maximum + 1), false).expect_err("invalid maximum");
        assert!(
            !ImportRange::new(0, 0)
                .expect("equal inclusive bounds")
                .extend_to_combat_end()
        );
    }

    #[rstest::rstest]
    fn requests_bound_paths_and_preserve_unicode() {
        let packet = import_packet(7, "/로그/fixture.log").expect("encode UTF-8 import path");
        assert_eq!(packet.get(8..16), Some(7_u64.to_le_bytes().as_slice()));
        assert_eq!(packet.get(64..), Some("/로그/fixture.log".as_bytes()));
        assert_eq!(
            import_packet(1, &"x".repeat(65472))
                .expect("maximum path")
                .len(),
            65536
        );
        for path in ["invalid\0path".to_owned(), "x".repeat(65473)] {
            assert!(matches!(
                import_packet(1, &path),
                Err(Error::InvalidImportRequest(_))
            ));
        }
    }

    #[rstest::rstest]
    fn results_validate_counts_without_changing_signed_totals() {
        let mut packet = BytesMut::zeroed(24);
        packet.put_i64_le(8);
        packet.put_i64_le(7);
        packet.put_i32_le(1);
        packet.put_i32_le(2);
        packet.put_i64_le(i64::MIN);
        packet.put_i64_le(20);
        let report = decode_import(&packet).expect("decode import summary");
        assert_eq!(report.lines_read, 8);
        assert_eq!(report.records_dispatched, 7);
        assert_eq!((report.encounters, report.combatants), (1, 2));
        assert_eq!((report.damage, report.healed), (i64::MIN, 20));
        for length in 0..64 {
            assert!(matches!(
                decode_import(packet.get(..length).expect("fixture prefix")),
                Err(Error::InvalidImportResponse(_))
            ));
        }
        for (offset, value) in [(24, -1_i64), (32, -1), (32, 9)] {
            let mut invalid = packet.clone();
            invalid
                .get_mut(offset..offset + 8)
                .expect("fixture count")
                .copy_from_slice(&value.to_le_bytes());
            assert!(matches!(
                decode_import(&invalid),
                Err(Error::InvalidImportResponse(_))
            ));
        }
        for offset in [40, 44] {
            let mut invalid = packet.clone();
            invalid
                .get_mut(offset..offset + 4)
                .expect("fixture count")
                .copy_from_slice(&(-1_i32).to_le_bytes());
            assert!(matches!(
                decode_import(&invalid),
                Err(Error::InvalidImportResponse(_))
            ));
        }
    }
}
