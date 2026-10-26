use bytes::{Buf, BufMut, BytesMut};

use crate::runtime::Request;
use crate::{Error, HostHandle};

pub(crate) const DIAGNOSTIC_CAPACITY: usize = 32 + 1024 * 1024;

/// An owned snapshot of host errors and backlog warnings, independent of command failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostDiagnostics {
    /// Total diagnostic entries recorded in the current managed host environment.
    pub total: u64,
    /// The most recent entries in recording order, capped at 32 bounded messages.
    pub recent: Vec<String>,
}

impl HostDiagnostics {
    /// Number of older entries excluded from this snapshot's bounded message history.
    pub fn omitted(&self) -> u64 {
        self.total.saturating_sub(self.recent.len() as u64)
    }
}

pub(crate) fn decode_diagnostics(packet: &[u8]) -> Result<HostDiagnostics, Error> {
    let invalid = |message: &str| Error::InvalidDiagnosticResponse(message.to_owned());
    let mut header = packet
        .get(16..32)
        .ok_or_else(|| invalid("missing diagnostic header"))?;
    let total = header.get_u64_le();
    let count = header.get_u32_le();
    let used = header.get_u32_le();
    if count > 32 || total > i64::MAX as u64 || total < u64::from(count) {
        return Err(invalid("invalid diagnostic count"));
    }
    let mut payload = packet
        .get(32..)
        .and_then(|bytes| bytes.get(..used as usize))
        .ok_or_else(|| invalid("diagnostic payload exceeds capacity"))?;
    let mut recent = Vec::with_capacity(count as usize);
    for _ in 0..count {
        if payload.remaining() < 4 {
            return Err(invalid("missing diagnostic message length"));
        }
        let length = payload.get_u32_le() as usize;
        if length > 3 * 8192 {
            return Err(invalid("diagnostic message exceeds bounds"));
        }
        let content = payload
            .get(..length)
            .ok_or_else(|| invalid("truncated diagnostic message"))?;
        let text = std::str::from_utf8(content)
            .map_err(|_invalid_utf8| invalid("diagnostic message is not UTF-8"))?;
        recent.push(text.to_owned());
        payload.advance(length);
    }
    if payload.has_remaining() {
        return Err(invalid("unexpected trailing diagnostic bytes"));
    }
    Ok(HostDiagnostics { total, recent })
}

impl HostHandle {
    /// Reads bounded host-error and backlog-warning history without consuming it or requiring a pump.
    ///
    /// Compare `total` between calls, and inspect `omitted()` to detect older errors outside the
    /// retained history. This reports host diagnostics, not errors caught privately by a plugin.
    pub async fn diagnostics(&self) -> Result<HostDiagnostics, Error> {
        let mut packet = BytesMut::with_capacity(DIAGNOSTIC_CAPACITY);
        packet.put_u32_le(DIAGNOSTIC_CAPACITY as u32);
        packet.put_u32_le(1);
        packet.put_u64_le(self.session);
        packet.resize(DIAGNOSTIC_CAPACITY, 0);
        self.request(Request::Diagnostics, &mut packet).await?;
        decode_diagnostics(&packet)
    }
}

#[cfg(test)]
mod tests {
    use bytes::{BufMut, BytesMut};

    use super::*;

    fn packet(total: u64, messages: &[&str]) -> BytesMut {
        let mut packet = BytesMut::zeroed(16);
        packet.put_u64_le(total);
        packet.put_u32_le(messages.len() as u32);
        packet.put_u32_le(0);
        for text in messages {
            packet.put_u32_le(text.len() as u32);
            packet.extend_from_slice(text.as_bytes());
        }
        let used = (packet.len() - 32) as u32;
        packet[28..32].copy_from_slice(&used.to_le_bytes());
        packet
    }

    #[rstest::rstest]
    fn snapshots_preserve_unicode_nul_and_omitted_counts() {
        let result = decode_diagnostics(&packet(40, &["한글\0first", "second\nline"]))
            .expect("decode owned diagnostics");
        assert_eq!(result.total, 40);
        assert_eq!(result.recent, ["한글\0first", "second\nline"]);
        assert_eq!(result.omitted(), 38);
        assert_eq!(
            decode_diagnostics(&packet(0, &[]))
                .expect("empty snapshot")
                .omitted(),
            0
        );
    }

    #[rstest::rstest]
    fn malformed_snapshots_are_rejected_before_slicing() {
        let valid = packet(1, &["message"]);
        for length in 0..valid.len() {
            assert!(matches!(
                decode_diagnostics(&valid[..length]),
                Err(Error::InvalidDiagnosticResponse(_))
            ));
        }
        for (offset, value) in [(24, 33), (28, u32::MAX), (32, u32::MAX), (32, 3 * 8192 + 1)] {
            let mut malformed = valid.clone();
            malformed[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(matches!(
                decode_diagnostics(&malformed),
                Err(Error::InvalidDiagnosticResponse(_))
            ));
        }
        let mut malformed = valid.clone();
        malformed[16..24].copy_from_slice(&0_u64.to_le_bytes());
        assert!(matches!(
            decode_diagnostics(&malformed),
            Err(Error::InvalidDiagnosticResponse(_))
        ));
        let mut malformed = valid;
        malformed[36] = 0xff;
        assert!(matches!(
            decode_diagnostics(&malformed),
            Err(Error::InvalidDiagnosticResponse(_))
        ));
        assert!(matches!(
            decode_diagnostics(&packet(0, &["unexpected"])),
            Err(Error::InvalidDiagnosticResponse(_))
        ));
        let mut trailing = packet(1, &["valid"]);
        trailing[24..28].copy_from_slice(&0_u32.to_le_bytes());
        assert!(matches!(
            decode_diagnostics(&trailing),
            Err(Error::InvalidDiagnosticResponse(_))
        ));
    }
}
