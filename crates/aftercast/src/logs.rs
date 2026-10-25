use std::sync::Arc;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use camino::Utf8Path;

use crate::runtime::Request;
use crate::{Error, Host};

pub(crate) const LOG_HEADER: usize = 64;
pub(crate) const MAX_LOG_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_LOG_PACKET: usize = LOG_HEADER + MAX_LOG_BYTES;
const RECORD_HEADER: usize = 56;
const MAX_TICKS: i64 = 3_155_378_975_999_999_999;

/// Limits for the single original-log subscription owned by a host.
#[derive(Debug, bon::Builder)]
pub struct LogSubscriptionConfig {
    /// Pending record limit, from 1 through 65,536. Overflow discards oldest records.
    #[builder(default = 4096)]
    max_records: u32,
    /// Pending encoded bytes, including record headers, from 56 through 64 MiB.
    #[builder(default = 8 * 1024 * 1024)]
    max_bytes: u32,
}

/// Session-scoped identity; a new subscription starts a new sequence at one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LogSubscription {
    pub(crate) session: u64,
    pub(crate) instance: u64,
}

/// Original CLR timestamp interpretation, preserved without assuming a timezone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogTimeKind {
    Unspecified,
    Utc,
    Local,
}

/// CLR ticks are 100 ns intervals since January 1, year 1, rather than Unix time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogTimestamp {
    pub ticks: u64,
    /// Known UTC ticks when available; unspecified values never acquire an inferred offset.
    pub utc_ticks: Option<u64>,
    pub kind: LogTimeKind,
}

/// Original text of a nonempty record selected for ACT hooks, captured before mutable parser hooks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRecord {
    /// Host subscription sequence, independent of ACT's writable time sorter.
    pub sequence: u64,
    pub is_import: bool,
    pub timestamp: LogTimestamp,
    pub time_sorter: i32,
    /// Owned immutable path; consecutive equal paths in a batch share one allocation.
    pub source_path: Arc<Utf8Path>,
    /// Valid UTF-8, including any embedded NUL; shares the owned batch allocation.
    pub original_line: Bytes,
}

/// Exact contiguous range omitted from this subscription, independent of core parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogLoss {
    pub first_sequence: u64,
    pub last_sequence: u64,
    pub count: u64,
}

/// Owned records and loss information. Retaining or cloning records never borrows CLR memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogBatch {
    pub records: Vec<LogRecord>,
    pub dropped: Option<LogLoss>,
}

pub(crate) fn log_packet(session: u64, instance: u64, command: u32, capacity: usize) -> BytesMut {
    let mut packet = BytesMut::with_capacity(capacity);
    packet.put_u32_le(capacity as u32);
    packet.put_u32_le(1);
    packet.put_u64_le(session);
    packet.put_u64_le(instance);
    packet.put_u32_le(command);
    packet.resize(capacity, 0);
    packet
}

pub(crate) fn subscribe_packet(
    session: u64,
    config: LogSubscriptionConfig,
) -> Result<BytesMut, Error> {
    if !(1..=65536).contains(&config.max_records)
        || !(RECORD_HEADER as u32..=MAX_LOG_BYTES as u32).contains(&config.max_bytes)
    {
        return Err(Error::InvalidLogRequest(
            "subscription limits are outside supported bounds".to_owned(),
        ));
    }
    let mut packet = BytesMut::with_capacity(LOG_HEADER);
    packet.put_u32_le(LOG_HEADER as u32);
    packet.put_u32_le(1);
    packet.put_u64_le(session);
    packet.put_u64_le(0);
    packet.put_u32_le(1);
    packet.put_u32_le(config.max_records);
    packet.resize(56, 0);
    packet.put_u32_le(config.max_bytes);
    packet.put_u32_le(0);
    Ok(packet)
}

pub(crate) fn subscription_result(packet: &[u8], session: u64) -> Result<LogSubscription, Error> {
    let mut result = packet
        .get(16..24)
        .ok_or_else(|| invalid("missing subscription identity"))?;
    let instance = result.get_u64_le();
    if instance == 0 {
        return Err(invalid("zero subscription identity"));
    }
    Ok(LogSubscription { session, instance })
}

pub(crate) fn larger_log_capacity(packet: &[u8]) -> Result<usize, Error> {
    let mut field = packet
        .get(60..64)
        .ok_or_else(|| invalid("missing required capacity"))?;
    let required = field.get_u32_le() as usize;
    if required <= packet.len() || required > MAX_LOG_PACKET {
        return Err(invalid("invalid required capacity"));
    }
    // Doubling bounds retries even when a concurrent producer replaces the head with larger records.
    Ok(required
        .max(packet.len().saturating_mul(2))
        .min(MAX_LOG_PACKET))
}

fn invalid(message: &str) -> Error {
    Error::InvalidLogResponse(message.to_owned())
}

fn decode_timestamp(flags: u32, ticks: i64, utc: i64) -> Result<LogTimestamp, Error> {
    let kind = match flags >> 1 {
        0 => LogTimeKind::Unspecified,
        1 => LogTimeKind::Utc,
        2 => LogTimeKind::Local,
        _ => return Err(invalid("unknown timestamp kind")),
    };
    if !(0..=MAX_TICKS).contains(&ticks)
        || match kind {
            LogTimeKind::Unspecified => utc != -1,
            LogTimeKind::Utc => utc != ticks,
            LogTimeKind::Local => !(0..=MAX_TICKS).contains(&utc),
        }
    {
        return Err(invalid("invalid timestamp ticks"));
    }
    Ok(LogTimestamp {
        ticks: u64::try_from(ticks).map_err(|_negative| invalid("negative timestamp"))?,
        utc_ticks: (utc != -1)
            .then(|| u64::try_from(utc))
            .transpose()
            .map_err(|_negative| invalid("negative UTC timestamp"))?,
        kind,
    })
}

pub(crate) fn decode_logs(packet: Bytes) -> Result<LogBatch, Error> {
    let mut header = packet
        .get(28..64)
        .ok_or_else(|| invalid("missing batch header"))?;
    let count = header.get_u32_le() as usize;
    let first = header.get_u64_le();
    let last = header.get_u64_le();
    let dropped = header.get_u64_le();
    let used = header.get_u32_le() as usize;
    let required = header.get_u32_le();
    let payload = packet
        .get(LOG_HEADER..)
        .and_then(|bytes| bytes.get(..used))
        .ok_or_else(|| invalid("batch payload exceeds capacity"))?;
    if required != 0 || used > MAX_LOG_BYTES || count > 65536 || count > used / RECORD_HEADER {
        return Err(invalid("invalid batch counts"));
    }
    let loss = match dropped {
        0 if first == 0 && last == 0 => None,
        _ if dropped > 0 && first > 0 && last >= first && last - first == dropped - 1 => {
            Some(LogLoss {
                first_sequence: first,
                last_sequence: last,
                count: dropped,
            })
        }
        _ => return Err(invalid("invalid missing sequence range")),
    };
    let mut records: Vec<LogRecord> = Vec::with_capacity(count);
    let mut offset = 0;
    let mut previous = loss.map(|loss| loss.last_sequence);
    for _ in 0..count {
        let mut fields = payload
            .get(offset..)
            .and_then(|bytes| bytes.get(..RECORD_HEADER))
            .ok_or_else(|| invalid("truncated record header"))?;
        let length = fields.get_u32_le() as usize;
        let flags = fields.get_u32_le();
        let sequence = fields.get_u64_le();
        let ticks = fields.get_i64_le();
        let utc = fields.get_i64_le();
        let time_sorter = fields.get_i32_le();
        let path_offset = fields.get_u32_le() as usize;
        let path_length = fields.get_u32_le() as usize;
        let line_offset = fields.get_u32_le() as usize;
        let line_length = fields.get_u32_le() as usize;
        let reserved = fields.get_u32_le();
        if sequence == 0 || previous.is_some_and(|prior| prior.checked_add(1) != Some(sequence)) {
            return Err(invalid("noncontiguous record sequence"));
        }
        if length < RECORD_HEADER
            || reserved != 0
            || path_offset != RECORD_HEADER
            || path_offset.checked_add(path_length) != Some(line_offset)
            || line_offset.checked_add(line_length) != Some(length)
        {
            return Err(invalid("invalid record ranges"));
        }
        let end = offset
            .checked_add(length)
            .ok_or_else(|| invalid("record length overflow"))?;
        let record = payload
            .get(offset..end)
            .ok_or_else(|| invalid("truncated record payload"))?;
        let path = record
            .get(path_offset..line_offset)
            .ok_or_else(|| invalid("invalid path range"))?;
        let path = std::str::from_utf8(path).map_err(|_encoding| invalid("path is not UTF-8"))?;
        let line = record
            .get(line_offset..)
            .ok_or_else(|| invalid("invalid line range"))?;
        std::str::from_utf8(line).map_err(|_encoding| invalid("original line is not UTF-8"))?;
        let timestamp = decode_timestamp(flags, ticks, utc)?;
        // Canonical ranges above prove this shared slice stays inside the immutable owned packet.
        let absolute_line = LOG_HEADER + offset + line_offset;
        let original_line = packet.slice(absolute_line..LOG_HEADER + end);
        let path = Utf8Path::new(path);
        let source_path = match records.last() {
            Some(previous) if previous.source_path.as_ref() == path => {
                Arc::clone(&previous.source_path)
            }
            _ => Arc::from(path),
        };
        records.push(LogRecord {
            sequence,
            is_import: flags & 1 != 0,
            timestamp,
            time_sorter,
            source_path,
            original_line,
        });
        previous = Some(sequence);
        offset = end;
    }
    if offset != used {
        return Err(invalid("unexpected trailing batch bytes"));
    }
    Ok(LogBatch {
        records,
        dropped: loss,
    })
}

impl Host {
    /// Starts this host's single bounded subscription to nonempty original text selected for ACT hooks.
    ///
    /// Slow consumers lose oldest pending records, with exact loss ranges returned by
    /// [`Self::drain_logs`]. Core parsing continues. This is not a durable recording or a parsed
    /// record count. Unsubscribe before configuring another subscription.
    pub fn subscribe_logs(
        &mut self,
        config: LogSubscriptionConfig,
    ) -> Result<LogSubscription, Error> {
        let mut packet = subscribe_packet(self.session.id(), config)?;
        if self.session.request(Request::Logs, &mut packet)? != 0 {
            return Err(Error::InvalidLogResponse(
                "unexpected subscription status".to_owned(),
            ));
        }
        subscription_result(&packet, self.session.id())
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
            let mut packet = log_packet(self.session.id(), subscription.instance, 2, capacity);
            match self.session.request(Request::Logs, &mut packet)? {
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
        let mut packet = log_packet(self.session.id(), subscription.instance, 3, LOG_HEADER);
        if self.session.request(Request::Logs, &mut packet)? != 0 {
            return Err(Error::InvalidLogResponse(
                "unexpected unsubscribe status".to_owned(),
            ));
        }
        Ok(())
    }

    fn check_log_subscription(&self, subscription: LogSubscription) -> Result<(), Error> {
        if subscription.session != self.session.id() {
            return Err(Error::InvalidLogRequest(
                "subscription belongs to another host session".to_owned(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32_field(packet: &mut [u8], offset: usize, value: u32) {
        packet
            .get_mut(offset..offset + 4)
            .expect("fixture field")
            .copy_from_slice(&value.to_le_bytes());
    }
    fn u64_field(packet: &mut [u8], offset: usize, value: u64) {
        packet
            .get_mut(offset..offset + 8)
            .expect("fixture field")
            .copy_from_slice(&value.to_le_bytes());
    }
    fn batch() -> BytesMut {
        batch_with_paths("/로그", "/log")
    }

    fn batch_with_paths(first: &str, second: &str) -> BytesMut {
        let mut packet = log_packet(9, 4, 2, LOG_HEADER);
        for (sequence, path, line) in [(1_u64, first, "first\0"), (2, second, "second")] {
            packet.put_u32_le((RECORD_HEADER + path.len() + line.len()) as u32);
            packet.put_u32_le(3);
            packet.put_u64_le(sequence);
            packet.put_i64_le(123456);
            packet.put_i64_le(123456);
            packet.put_i32_le(-10);
            packet.put_u32_le(RECORD_HEADER as u32);
            packet.put_u32_le(path.len() as u32);
            packet.put_u32_le((RECORD_HEADER + path.len()) as u32);
            packet.put_u32_le(line.len() as u32);
            packet.put_u32_le(0);
            packet.extend_from_slice(path.as_bytes());
            packet.extend_from_slice(line.as_bytes());
        }
        let size = packet.len();
        u32_field(&mut packet, 0, size as u32);
        u32_field(&mut packet, 28, 2);
        u32_field(&mut packet, 56, (size - LOG_HEADER) as u32);
        packet
    }

    #[rstest::rstest]
    fn batches_preserve_metadata_and_share_owned_line_bytes() {
        let wire = batch().freeze();
        let result = decode_logs(wire.clone()).expect("decode valid batch");
        let first = result.records.first().expect("first record");
        assert_eq!(first.sequence, 1);
        assert_eq!(first.source_path.as_ref(), Utf8Path::new("/로그"));
        assert_eq!(first.time_sorter, -10);
        assert!(first.is_import);
        assert_eq!(
            first.timestamp,
            LogTimestamp {
                ticks: 123456,
                utc_ticks: Some(123456),
                kind: LogTimeKind::Utc
            }
        );
        let original = first.original_line.clone();
        let line_offset = LOG_HEADER + RECORD_HEADER + "/로그".len();
        assert_eq!(
            original.as_ptr(),
            wire.get(line_offset..).expect("wire line").as_ptr()
        );
        drop(result);
        drop(wire);
        assert_eq!(original.as_ref(), b"first\0");
    }

    #[rstest::rstest]
    fn consecutive_records_share_owned_source_paths() {
        let wire = batch_with_paths("/same/한글.log", "/same/한글.log").freeze();
        let batch = decode_logs(wire).expect("valid shared-path batch");
        let first = batch.records.first().expect("first record");
        let second = batch.records.last().expect("second record");
        assert!(Arc::ptr_eq(&first.source_path, &second.source_path));
        let retained = first.source_path.clone();
        drop(batch);
        assert_eq!(retained.as_ref(), Utf8Path::new("/same/한글.log"));
    }

    #[rstest::rstest]
    fn empty_loss_batches_and_timestamp_kinds_are_explicit() {
        let mut empty = log_packet(9, 4, 2, LOG_HEADER);
        u64_field(&mut empty, 32, 3);
        u64_field(&mut empty, 40, 5);
        u64_field(&mut empty, 48, 3);
        assert_eq!(
            decode_logs(empty.freeze()).expect("loss without records"),
            LogBatch {
                records: vec![],
                dropped: Some(LogLoss {
                    first_sequence: 3,
                    last_sequence: 5,
                    count: 3
                }),
            }
        );
        let mut with_loss = batch();
        u64_field(&mut with_loss, 32, 1);
        u64_field(&mut with_loss, 40, 2);
        u64_field(&mut with_loss, 48, 2);
        u64_field(&mut with_loss, LOG_HEADER + 8, 3);
        let second = LOG_HEADER + RECORD_HEADER + "/로그".len() + "first\0".len();
        u64_field(&mut with_loss, second + 8, 4);
        u32_field(&mut with_loss, LOG_HEADER + 4, 1);
        u64_field(&mut with_loss, LOG_HEADER + 24, u64::MAX);
        u32_field(&mut with_loss, second + 4, 4);
        u64_field(&mut with_loss, second + 24, 120000);
        let result =
            decode_logs(with_loss.freeze()).expect("explicit local/unspecified timestamps");
        assert_eq!(
            result.records.first().expect("first").timestamp.utc_ticks,
            None
        );
        let local = result.records.last().expect("last");
        assert!(!local.is_import);
        assert_eq!(
            local.timestamp,
            LogTimestamp {
                ticks: 123456,
                utc_ticks: Some(120000),
                kind: LogTimeKind::Local
            }
        );
    }

    #[rstest::rstest]
    fn malformed_batches_fail_before_unchecked_slices_or_allocations() {
        let valid = batch();
        for length in 0..valid.len() {
            let prefix = Bytes::copy_from_slice(valid.get(..length).expect("fixture prefix"));
            assert!(matches!(
                decode_logs(prefix),
                Err(Error::InvalidLogResponse(_))
            ));
        }
        for (offset, value) in [
            (28, 65537),
            (28, 0),
            (56, u32::MAX),
            (60, 1),
            (LOG_HEADER, 0),
            (LOG_HEADER, u32::MAX),
            (LOG_HEADER + 4, 6),
            (LOG_HEADER + 36, 0),
            (LOG_HEADER + 40, u32::MAX),
            (LOG_HEADER + 44, 0),
            (LOG_HEADER + 48, u32::MAX),
            (LOG_HEADER + 52, 1),
        ] {
            let mut invalid = valid.clone();
            u32_field(&mut invalid, offset, value);
            assert!(matches!(
                decode_logs(invalid.freeze()),
                Err(Error::InvalidLogResponse(_))
            ));
        }
        for (offset, value) in [
            (LOG_HEADER + 8, 0),
            (LOG_HEADER + 16, u64::MAX),
            (LOG_HEADER + 24, 0),
            (32, 1),
            (48, 1),
        ] {
            let mut invalid = valid.clone();
            u64_field(&mut invalid, offset, value);
            assert!(matches!(
                decode_logs(invalid.freeze()),
                Err(Error::InvalidLogResponse(_))
            ));
        }
        for offset in [
            LOG_HEADER + RECORD_HEADER,
            LOG_HEADER + RECORD_HEADER + "/로그".len(),
        ] {
            let mut invalid = valid.clone();
            *invalid.get_mut(offset).expect("fixture text") = 255;
            assert!(matches!(
                decode_logs(invalid.freeze()),
                Err(Error::InvalidLogResponse(_))
            ));
        }
        let mut invalid = valid;
        u64_field(&mut invalid, 32, 1);
        u64_field(&mut invalid, 40, 1);
        // A nonzero range with zero drops must fail rather than underflow while validating it.
        assert!(matches!(
            decode_logs(invalid.freeze()),
            Err(Error::InvalidLogResponse(_))
        ));
    }

    #[rstest::rstest]
    fn configuration_and_capacity_retries_are_bounded() {
        for (records, bytes) in [(0, 56), (65537, 56), (1, 55), (1, MAX_LOG_BYTES as u32 + 1)] {
            assert!(matches!(
                subscribe_packet(
                    9,
                    LogSubscriptionConfig::builder()
                        .max_records(records)
                        .max_bytes(bytes)
                        .build()
                ),
                Err(Error::InvalidLogRequest(_))
            ));
        }
        let mut packet = subscribe_packet(9, LogSubscriptionConfig::builder().build())
            .expect("default subscription");
        assert!(matches!(
            subscription_result(&packet, 9),
            Err(Error::InvalidLogResponse(_))
        ));
        u64_field(&mut packet, 16, 7);
        assert_eq!(
            subscription_result(&packet, 9)
                .expect("subscription identity")
                .instance,
            7
        );
        for required in [0, 64, MAX_LOG_PACKET as u32 + 1] {
            u32_field(&mut packet, 60, required);
            assert!(matches!(
                larger_log_capacity(&packet),
                Err(Error::InvalidLogResponse(_))
            ));
        }
        u32_field(&mut packet, 60, 200);
        assert_eq!(
            larger_log_capacity(&packet).expect("larger head without consumption"),
            200
        );
    }
}
