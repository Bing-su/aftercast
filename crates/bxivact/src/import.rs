use bytes::{Buf, BufMut, BytesMut};

use crate::Error;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
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

    #[test]
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
