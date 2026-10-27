use bytes::{Buf, BufMut, BytesMut};
use camino::Utf8Path;
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
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ImportProgress {
    pub lines_read: u64,
    pub records_dispatched: u64,
    pub bytes_read: u64,
    pub total_bytes: u64,
}

/// Inclusive source wall-clock bounds for ACT history reparse, in 100 ns ticks since year 0001.
/// No timezone conversion or extension to combat end is performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportRange {
    start_ticks: u64,
    end_ticks: u64,
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
        })
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

pub(crate) struct PendingImport {
    pub(crate) range: Option<ImportRange>,
    pub(crate) cancellation: Arc<AtomicBool>,
    pub(crate) progress: watch::Sender<ImportProgress>,
    pub(crate) complete: oneshot::Sender<Result<ImportReport, Error>>,
}

impl PendingImport {
    pub(crate) fn begin(&self, session: &mut RemoteSession, path: &Utf8Path) -> Result<(), Error> {
        let path = canonicalize(path)?;
        let mut packet = import_packet(session.id(), path.as_str())?;
        if let Some(range) = self.range {
            packet
                .get_mut(24..32)
                .ok_or(Error::Closed)?
                .copy_from_slice(&range.start_ticks.to_le_bytes());
            packet
                .get_mut(32..40)
                .ok_or(Error::Closed)?
                .copy_from_slice(&range.end_ticks.to_le_bytes());
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    })
}

impl HostHandle {
    /// Reparse the selected inclusive time range through initialized plugins.
    /// Stops at the first later timestamp; same-time rows remain distinct. Shares the full import
    /// reader, cancellation, partial-result retention and live-state restoration contracts.
    pub async fn import_log_range(
        &self,
        path: impl AsRef<Utf8Path>,
        range: ImportRange,
    ) -> Result<ImportReport, Error> {
        self.start_import_range(path, range).await?.wait().await
    }

    /// Imports the initial contents of a UTF-8 file through initialized plugins and drains actions.
    ///
    /// Call while live combat is idle. Live file delivery is paused and
    /// restored, including after failure. Appends after opening are excluded; malformed UTF-8,
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
    fn source_ranges_validate_inclusive_wall_clock_bounds() {
        let maximum = 3_155_378_975_999_999_999;
        for (start, end) in [(0, 0), (0, maximum), (maximum, maximum)] {
            let range = ImportRange::new(start, end).expect("valid inclusive bounds");
            assert_eq!((range.start_ticks(), range.end_ticks()), (start, end));
        }
        ImportRange::new(2, 1).expect_err("reversed range");
        ImportRange::new(0, maximum + 1).expect_err("out of DateTime range");
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
