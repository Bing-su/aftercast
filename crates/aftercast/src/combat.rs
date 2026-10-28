use bytes::{Buf, BufMut, BytesMut};

use crate::runtime::Request;
use crate::snapshots::{decode_result, graph_time};
use crate::{CombatantSnapshot, Error, HostHandle, LogTimestamp};

const HEADER: usize = 48;
const MAX_BYTES: usize = 64 * 1024 * 1024;

/// Stable identity of an in-memory encounter in one host session; invalid after restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EncounterId {
    pub(crate) session: u64,
    pub(crate) value: u64,
}

/// Current encounter lifecycle; imports remain Importing until their whole operation completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncounterStatus {
    Active,
    Ended,
    Importing,
    PartialImport,
}

/// Owned metadata and totals for an encounter, including an active encounter.
#[derive(Debug, Clone)]
pub struct EncounterSummary {
    pub id: EncounterId,
    pub is_import: bool,
    pub status: EncounterStatus,
    pub zone: String,
    pub title: String,
    pub character: String,
    pub start_time: LogTimestamp,
    pub end_time: LogTimestamp,
    /// ACT duration in 100 ns ticks, measured from combat records rather than the UI clock.
    pub duration_ticks: i64,
    pub damage: i64,
    pub healed: i64,
    pub combatants: u32,
}

/// Owned participant totals captured with the encounter summary under the combat lock.
#[derive(Debug, Clone)]
pub struct EncounterDetail {
    pub summary: EncounterSummary,
    pub combatants: Vec<CombatantSnapshot>,
}

/// An offset page of the current graph, not a frozen cursor or a durable history archive.
#[derive(Debug, Clone)]
pub struct EncounterPage<T> {
    pub total: u32,
    pub items: Vec<T>,
}

/// One ACT category/attack row. `All` is an aggregate row; categories can overlap.
#[derive(Debug, Clone)]
pub struct AttackSummary {
    pub combatant: String,
    pub category: String,
    pub attack: String,
    pub outgoing: bool,
    pub damage: i64,
    pub duration_ticks: i64,
    pub dps: f64,
    pub enc_dps: f64,
    pub swings: i32,
    pub hits: i32,
    pub crit_hits: i32,
    pub misses: i32,
    pub blocked: i32,
}

/// Link within the originating encounter; ambiguous or omitted logs never select a guessed line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceLogLink {
    Missing,
    Ambiguous,
    Retained(u32),
}

/// One canonical outgoing action; categories are not summed or duplicated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncounterAction {
    pub time: LogTimestamp,
    pub time_sorter: i32,
    pub attacker: String,
    pub victim: String,
    pub attack: String,
    pub damage: String,
    pub source: SourceLogLink,
    /// Original Dnum integer, including ACT negative outcome sentinels.
    pub amount: i64,
    pub swing_type: i32,
    pub critical: bool,
    pub special: Option<String>,
    pub damage_type: Option<String>,
    pub raw_label: Option<String>,
    pub raw_suffix: Option<String>,
}

/// Frozen action rows from one host session. A later capture invalidates this handle.
#[derive(Debug, Clone, Copy)]
pub struct ActionSnapshot {
    session: u64,
    revision: u64,
    pub total: u32,
}

/// Registration membership at capture time; both flags may be true or false.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedAction {
    pub action: EncounterAction,
    pub registered_damage: bool,
    pub registered_healing: bool,
}

/// Retained encounter text, independent of the live subscription buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncounterLog {
    pub time: LogTimestamp,
    pub time_sorter: i32,
    pub log_type: i32,
    /// Uniqueness and provenance checked across the whole encounter in the same read as this text.
    pub source: SourceLogLink,
    pub text: String,
}

/// Current page and exact automatic capacity omissions; disabled recording is not counted as loss.
#[derive(Debug, Clone)]
pub struct EncounterLogs {
    pub dropped: u64,
    pub page: EncounterPage<EncounterLog>,
}

impl HostHandle {
    /// Reads current live combat without ending it. Returns None while idle or importing.
    ///
    /// Poll at a modest rate (e.g. 5 Hz). Only already-applied actions are
    /// included; this call does not wait for queued actions or plugin-private work.
    pub async fn live_combat(&self) -> Result<Option<EncounterDetail>, Error> {
        let payload = self.query_combat(1, None, 0, 1).await?;
        decode_detail(&payload, self.session)
    }

    /// Lists retained individual encounters, including imports and active combat; excludes zone All.
    ///
    /// `limit` must be 1..=256. Ordering follows ACT zone/encounter order, with imports first.
    /// Graph changes between calls can move offsets; IDs remain stable while an encounter exists.
    pub async fn encounters(
        &self,
        offset: u32,
        limit: u32,
    ) -> Result<EncounterPage<EncounterSummary>, Error> {
        let payload = self.query_combat(2, None, offset, limit).await?;
        let mut reader = Reader(&payload);
        let page = reader.page(limit, offset, |reader| reader.summary(self.session))?;
        reader.finish()?;
        Ok(page)
    }

    /// Reads a retained encounter and its participants. None means the graph no longer contains it.
    /// Results own their data after later actions, imports and shutdown; they are fresh graph reads.
    pub async fn encounter_detail(
        &self,
        id: EncounterId,
    ) -> Result<Option<EncounterDetail>, Error> {
        let payload = self.query_combat(3, Some(id), 0, 1).await?;
        let detail = decode_detail(&payload, self.session)?;
        if detail
            .as_ref()
            .is_some_and(|detail| detail.summary.id != id)
        {
            return Err(invalid("encounter identity mismatch"));
        }
        Ok(detail)
    }

    /// Reads category/attack aggregates in ACT combatant/category/attack key order.
    ///
    /// `limit` must be 1..=256. None means the encounter was removed. Empty pages are valid.
    /// Preserve category/direction when displaying rows; summing every row double-counts actions.
    pub async fn encounter_attacks(
        &self,
        id: EncounterId,
        offset: u32,
        limit: u32,
    ) -> Result<Option<EncounterPage<AttackSummary>>, Error> {
        let payload = self.query_combat(4, Some(id), offset, limit).await?;
        let mut reader = Reader(&payload);
        let page = if reader.present()? {
            Some(reader.page(limit, offset, Reader::attack)?)
        } else {
            None
        };
        reader.finish()?;
        Ok(page)
    }

    /// Pages canonical outgoing actions and their encounter-scoped source-log links.
    /// Like other graph queries, pages are current reads, not a frozen cursor. Limit is 1..=256.
    pub async fn encounter_actions(
        &self,
        id: EncounterId,
        offset: u32,
        limit: u32,
    ) -> Result<Option<EncounterPage<EncounterAction>>, Error> {
        let payload = self.query_combat(5, Some(id), offset, limit).await?;
        let mut reader = Reader(&payload);
        let page = if reader.present()? {
            Some(reader.page(limit, offset, Reader::action)?)
        } else {
            None
        };
        reader.finish()?;
        Ok(page)
    }

    /// Captures at most 16384 actions and registration membership under the combat lock.
    pub async fn capture_action_snapshot(
        &self,
        id: EncounterId,
    ) -> Result<Option<ActionSnapshot>, Error> {
        let payload = self.query_combat(7, Some(id), 0, 1).await?;
        let mut reader = Reader(&payload);
        let snapshot = if reader.present()? {
            let revision = reader.u64()?;
            let total = reader.u32()?;
            if revision == 0 || total > 16384 {
                return Err(invalid("invalid action snapshot"));
            }
            Some(ActionSnapshot {
                session: self.session,
                revision,
                total,
            })
        } else {
            None
        };
        reader.finish()?;
        Ok(snapshot)
    }

    /// Reads frozen rows; source indices address `action_snapshot_logs` in the same snapshot.
    pub async fn action_snapshot_page(
        &self,
        snapshot: ActionSnapshot,
        offset: u32,
        limit: u32,
    ) -> Result<EncounterPage<ClassifiedAction>, Error> {
        let id = EncounterId {
            session: snapshot.session,
            value: snapshot.revision,
        };
        let payload = self.query_combat(8, Some(id), offset, limit).await?;
        let mut reader = Reader(&payload);
        let page = reader.page(limit, offset, |reader| {
            let action = reader.action()?;
            let flags = reader.u32()?;
            if flags & !3 != 0 {
                return Err(invalid("invalid classification flags"));
            }
            Ok(ClassifiedAction {
                action,
                registered_damage: flags & 1 != 0,
                registered_healing: flags & 2 != 0,
            })
        })?;
        if page.total != snapshot.total {
            return Err(invalid("action snapshot total changed"));
        }
        reader.finish()?;
        Ok(page)
    }

    /// Reads the same frozen log list referenced by action snapshot source indices.
    pub async fn action_snapshot_logs(
        &self,
        snapshot: ActionSnapshot,
        offset: u32,
        limit: u32,
    ) -> Result<EncounterLogs, Error> {
        let id = EncounterId {
            session: snapshot.session,
            value: snapshot.revision,
        };
        let payload = self.query_combat(9, Some(id), offset, limit).await?;
        decode_logs(&payload, offset, limit)?.ok_or_else(|| invalid("missing action snapshot logs"))
    }

    /// Pages retained text and capacity omissions. Missing encounters return None; empty logs are valid.
    /// Source indices are offsets in this encounter, never timestamps or live subscription sequences.
    pub async fn encounter_logs(
        &self,
        id: EncounterId,
        offset: u32,
        limit: u32,
    ) -> Result<Option<EncounterLogs>, Error> {
        let payload = self.query_combat(6, Some(id), offset, limit).await?;
        decode_logs(&payload, offset, limit)
    }

    async fn query_combat(
        &self,
        mode: u32,
        id: Option<EncounterId>,
        offset: u32,
        limit: u32,
    ) -> Result<BytesMut, Error> {
        if !(1..=256).contains(&limit)
            || offset > i32::MAX as u32
            || id.is_some_and(|id| id.session != self.session)
        {
            return Err(Error::InvalidCombatRequest(
                "invalid page or encounter belongs to another session".into(),
            ));
        }
        let mut capacity = 64 * 1024;
        loop {
            let mut packet = BytesMut::with_capacity(capacity);
            packet.put_u32_le(capacity as u32);
            packet.put_u32_le(1);
            packet.put_u64_le(self.session);
            packet.put_u32_le(mode);
            packet.put_u32_le(offset);
            packet.put_u64_le(id.map_or(0, |id| id.value));
            packet.put_u32_le(limit);
            packet.resize(capacity, 0);
            let status = self.request(Request::Combat, &mut packet).await?;
            let mut fields = Reader(
                packet
                    .get(36..HEADER)
                    .ok_or_else(|| invalid("missing query header"))?,
            );
            let used = fields.u32()? as usize;
            let required = fields.u32()? as usize;
            if fields.u32()? != 0 {
                return Err(invalid("reserved query header"));
            }
            if status == 0 && (required != 0 || used > MAX_BYTES || used > capacity - HEADER) {
                return Err(invalid("invalid query payload length"));
            }
            if status == 0 {
                packet.truncate(HEADER + used);
                return Ok(packet.split_off(HEADER));
            }
            if used != 0 || required <= capacity || required > HEADER + MAX_BYTES {
                return Err(invalid("invalid query capacity"));
            }
            capacity = required
                .max(capacity.saturating_mul(2))
                .min(HEADER + MAX_BYTES);
        }
    }
}

fn invalid(message: &str) -> Error {
    Error::InvalidCombatResponse(message.into())
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let (value, rest) = self
            .0
            .split_at_checked(length)
            .ok_or_else(|| invalid("truncated combat query"))?;
        self.0 = rest;
        Ok(value)
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(self.take(4)?.get_u32_le())
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(self.take(8)?.get_u64_le())
    }
    fn i32(&mut self) -> Result<i32, Error> {
        Ok(self.take(4)?.get_i32_le())
    }
    fn i64(&mut self) -> Result<i64, Error> {
        Ok(self.take(8)?.get_i64_le())
    }
    fn f64(&mut self) -> Result<f64, Error> {
        Ok(self.take(8)?.get_f64_le())
    }
    fn text(&mut self) -> Result<String, Error> {
        let length = self.u32()? as usize;
        std::str::from_utf8(self.take(length)?)
            .map(str::to_owned)
            .map_err(|_encoding| invalid("invalid combat UTF-8"))
    }
    fn optional_text(&mut self) -> Result<Option<String>, Error> {
        let length = self.u32()?;
        if length == u32::MAX {
            return Ok(None);
        }
        std::str::from_utf8(self.take(length as usize)?)
            .map(|value| Some(value.to_owned()))
            .map_err(|_encoding| invalid("invalid combat UTF-8"))
    }
    fn time(&mut self) -> Result<LogTimestamp, Error> {
        graph_time(self.i64()?, self.u32()?).map_err(|_time| invalid("invalid combat timestamp"))
    }
    fn present(&mut self) -> Result<bool, Error> {
        match self.u32()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(invalid("invalid presence flag")),
        }
    }
    fn finish(&self) -> Result<(), Error> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(invalid("trailing combat bytes"))
        }
    }
    fn page<T>(
        &mut self,
        limit: u32,
        offset: u32,
        mut read: impl FnMut(&mut Self) -> Result<T, Error>,
    ) -> Result<EncounterPage<T>, Error> {
        let total = self.u32()?;
        let count = self.u32()?;
        if total > i32::MAX as u32
            || count > 256
            || count != total.saturating_sub(offset).min(limit)
        {
            return Err(invalid("invalid combat page counts"));
        }
        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            items.push(read(self)?);
        }
        Ok(EncounterPage { total, items })
    }
    fn summary(&mut self, session: u64) -> Result<EncounterSummary, Error> {
        let value = self.u64()?;
        let flags = self.u32()?;
        let is_import = flags & 1 != 0;
        let status = match flags >> 1 {
            0 => EncounterStatus::Active,
            1 => EncounterStatus::Ended,
            2 => EncounterStatus::Importing,
            3 => EncounterStatus::PartialImport,
            _ => return Err(invalid("invalid combat status")),
        };
        if value == 0
            || !is_import
                && matches!(
                    status,
                    EncounterStatus::Importing | EncounterStatus::PartialImport
                )
        {
            return Err(invalid("invalid combat identity/status"));
        }
        Ok(EncounterSummary {
            id: EncounterId { session, value },
            is_import,
            status,
            zone: self.text()?,
            title: self.text()?,
            character: self.text()?,
            start_time: self.time()?,
            end_time: self.time()?,
            duration_ticks: self.i64()?,
            damage: self.i64()?,
            healed: self.i64()?,
            combatants: self.u32()?,
        })
    }
    fn action(&mut self) -> Result<EncounterAction, Error> {
        let time = self.time()?;
        let time_sorter = self.i32()?;
        let attacker = self.text()?;
        let victim = self.text()?;
        let attack = self.text()?;
        let damage = self.text()?;
        let source = self.source()?;
        let amount = self.i64()?;
        let swing_type = self.i32()?;
        let flags = self.u32()?;
        if flags & !1 != 0 {
            return Err(invalid("invalid action flags"));
        }
        let special = self.optional_text()?;
        let damage_type = self.optional_text()?;
        let raw_label = self.optional_text()?;
        let raw_suffix = self.optional_text()?;
        Ok(EncounterAction {
            time,
            time_sorter,
            attacker,
            victim,
            attack,
            damage,
            source,
            amount,
            swing_type,
            critical: flags & 1 != 0,
            special,
            damage_type,
            raw_label,
            raw_suffix,
        })
    }
    fn source(&mut self) -> Result<SourceLogLink, Error> {
        match self.i32()? {
            -1 => Ok(SourceLogLink::Missing),
            -2 => Ok(SourceLogLink::Ambiguous),
            index if index >= 0 => Ok(SourceLogLink::Retained(index.cast_unsigned())),
            _ => Err(invalid("invalid source log link")),
        }
    }
    fn log(&mut self) -> Result<EncounterLog, Error> {
        Ok(EncounterLog {
            time: self.time()?,
            time_sorter: self.i32()?,
            log_type: self.i32()?,
            source: self.source()?,
            text: self.text()?,
        })
    }
    fn attack(&mut self) -> Result<AttackSummary, Error> {
        Ok(AttackSummary {
            combatant: self.text()?,
            category: self.text()?,
            attack: self.text()?,
            outgoing: self.present()?,
            damage: self.i64()?,
            duration_ticks: self.i64()?,
            dps: self.f64()?,
            enc_dps: self.f64()?,
            swings: self.i32()?,
            hits: self.i32()?,
            crit_hits: self.i32()?,
            misses: self.i32()?,
            blocked: self.i32()?,
        })
    }
}

fn decode_logs(payload: &[u8], offset: u32, limit: u32) -> Result<Option<EncounterLogs>, Error> {
    let mut reader = Reader(payload);
    let page = if reader.present()? {
        let dropped = reader.u64()?;
        if dropped > i64::MAX as u64 {
            return Err(invalid("invalid omitted log count"));
        }
        let page = reader.page(limit, offset, Reader::log)?;
        for (position, line) in page.items.iter().enumerate() {
            if let SourceLogLink::Retained(index) = line.source
                && index != offset + position as u32
            {
                return Err(invalid("log source index mismatch"));
            }
        }
        Some(EncounterLogs { dropped, page })
    } else {
        None
    };
    reader.finish()?;
    Ok(page)
}

fn decode_detail(payload: &[u8], session: u64) -> Result<Option<EncounterDetail>, Error> {
    let mut reader = Reader(payload);
    let detail = if reader.present()? {
        let summary = reader.summary(session)?;
        let length = reader.u32()? as usize;
        // Reuse ACT value validation; this graph read publishes no snapshot revision or completion state.
        let snapshot = decode_result(reader.take(length)?, summary.combatants as usize)
            .map_err(|_snapshot| invalid("invalid encounter participant snapshot"))?;
        Some(EncounterDetail {
            summary,
            combatants: snapshot.combatants,
        })
    } else {
        None
    };
    reader.finish()?;
    Ok(detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(wire: &mut BytesMut, value: &str) {
        wire.put_u32_le(value.len() as u32);
        wire.extend_from_slice(value.as_bytes());
    }

    fn attack_page() -> BytesMut {
        // Build independent wire bytes to check Unicode and ACT's signed/nonfinite values across the ABI.
        let mut wire = BytesMut::new();
        wire.put_u32_le(1);
        wire.put_u32_le(1);
        for value in ["Source 한글\0", "Damage", "Strike"] {
            text(&mut wire, value);
        }
        wire.put_u32_le(1);
        wire.put_i64_le(i64::MIN);
        wire.put_i64_le(-17);
        wire.put_u64_le(0xfff8_0000_0000_1234);
        wire.put_f64_le(f64::INFINITY);
        for value in [i32::MIN, 2, 3, -4, 5] {
            wire.put_i32_le(value);
        }
        wire
    }

    #[rstest::rstest]
    fn attack_pages_preserve_values_and_reject_every_truncated_prefix() {
        let wire = attack_page();
        let mut reader = Reader(&wire);
        let page = reader
            .page(256, 0, Reader::attack)
            .expect("valid attack page");
        reader.finish().expect("exact page framing");
        let row = &page.items[0];
        assert_eq!(row.combatant, "Source 한글\0");
        assert!(row.outgoing);
        assert_eq!(row.damage, i64::MIN);
        assert_eq!(row.duration_ticks, -17);
        assert_eq!(row.dps.to_bits(), 0xfff8_0000_0000_1234);
        assert_eq!(row.enc_dps.to_bits(), f64::INFINITY.to_bits());
        assert_eq!(row.swings, i32::MIN);
        for length in 0..wire.len() {
            Reader(wire.get(..length).expect("prefix"))
                .page(256, 0, Reader::attack)
                .expect_err("reject truncated attack page");
        }
        Reader(&wire)
            .page(1, 1, Reader::attack)
            .expect_err("reject wrong page count");
        let mut oversize = BytesMut::new();
        oversize.put_u32_le(u32::MAX);
        oversize.put_u32_le(u32::MAX);
        Reader(&oversize)
            .page(u32::MAX, 0, Reader::attack)
            .expect_err("reject oversized count");
        decode_detail(&2u32.to_le_bytes(), 1).expect_err("reject invalid presence");
        decode_detail(&[0; 5], 1).expect_err("reject trailing bytes");
        assert!(decode_detail(&[0; 4], 1).expect("absent detail").is_none());
    }

    #[test]
    fn action_and_log_pages_reject_truncation_and_invalid_links_or_loss() {
        let mut action = BytesMut::new();
        action.put_i64_le(0);
        action.put_u32_le(0);
        action.put_i32_le(42);
        for value in ["Source", "Target", "Strike", "100"] {
            text(&mut action, value);
        }
        action.put_i32_le(7);
        let source_end = action.len();
        action.put_i64_le(-9);
        action.put_i32_le(3);
        action.put_u32_le(1);
        text(&mut action, "special");
        text(&mut action, "element");
        action.put_i32_le(-1);
        text(&mut action, "suffix");
        let decoded = Reader(&action).action().expect("action projection");
        assert_eq!(
            (decoded.amount, decoded.swing_type, decoded.critical),
            (-9, 3, true)
        );
        assert_eq!(
            (decoded.raw_label, decoded.raw_suffix),
            (None, Some("suffix".into()))
        );
        for amount in [i64::MIN, i64::MAX, -1, -2, -3, -4, -5, -9, -10, 0] {
            let mut candidate = action.clone();
            candidate[source_end..source_end + 8].copy_from_slice(&amount.to_le_bytes());
            assert_eq!(
                Reader(&candidate).action().expect("raw amount").amount,
                amount
            );
        }
        let mut invalid_flags = action.clone();
        invalid_flags[source_end + 12..source_end + 16].copy_from_slice(&2u32.to_le_bytes());
        Reader(&invalid_flags).action().expect_err("reserved flags");
        let mut invalid_utf8 = action.clone();
        invalid_utf8[source_end + 20] = 0xff;
        Reader(&invalid_utf8)
            .action()
            .expect_err("invalid optional UTF-8");
        let mut oversized = action.clone();
        oversized[source_end + 16..source_end + 20].copy_from_slice(&(u32::MAX - 1).to_le_bytes());
        Reader(&oversized)
            .action()
            .expect_err("overstated optional length");
        assert_eq!(
            Reader(&action).action().expect("action").source,
            SourceLogLink::Retained(7)
        );
        for length in 0..action.len() {
            Reader(&action[..length])
                .action()
                .expect_err("truncated action");
        }
        action[source_end - 4..source_end].copy_from_slice(&(-3_i32).to_le_bytes());
        Reader(&action).action().expect_err("invalid source link");
        let mut logs = BytesMut::new();
        logs.put_u32_le(1);
        logs.put_u64_le(3);
        logs.put_u32_le(1);
        logs.put_u32_le(1);
        logs.put_i64_le(0);
        logs.put_u32_le(0);
        logs.put_i32_le(42);
        logs.put_i32_le(12);
        logs.put_i32_le(0);
        text(&mut logs, "한글\0 source");
        let page = decode_logs(&logs, 0, 1)
            .expect("log page")
            .expect("present");
        assert_eq!(page.dropped, 3);
        assert_eq!(page.page.items[0].text, "한글\0 source");
        for length in 0..logs.len() {
            decode_logs(&logs[..length], 0, 1).expect_err("truncated logs");
        }
        let mut wrong_index = logs.clone();
        wrong_index[40..44].copy_from_slice(&1_i32.to_le_bytes());
        decode_logs(&wrong_index, 0, 1).expect_err("mismatched log position");
        logs[4..12].copy_from_slice(&u64::MAX.to_le_bytes());
        decode_logs(&logs, 0, 1).expect_err("invalid loss count");
    }

    #[rstest::rstest]
    fn summary_validates_identity_status_text_and_time() {
        let mut wire = BytesMut::new();
        wire.put_u64_le(42);
        wire.put_u32_le(7);
        for value in ["지역", "Target", "Source"] {
            text(&mut wire, value);
        }
        for _ in 0..2 {
            wire.put_i64_le(0);
            wire.put_u32_le(1);
        }
        for value in [-10, 400, 20] {
            wire.put_i64_le(value);
        }
        wire.put_u32_le(2);
        let summary = Reader(&wire).summary(7).expect("valid partial import");
        assert_eq!(
            summary.id,
            EncounterId {
                session: 7,
                value: 42
            }
        );
        assert!(summary.is_import);
        assert_eq!(summary.status, EncounterStatus::PartialImport);
        assert_eq!(summary.zone, "지역");
        for length in 0..wire.len() {
            Reader(wire.get(..length).expect("prefix"))
                .summary(7)
                .expect_err("reject truncated summary");
        }
        for (offset, value) in [(0, 0), (8, 6), (8, 8), (12, u32::MAX)] {
            let mut bad = wire.clone();
            bad.get_mut(offset..offset + 4)
                .expect("field")
                .copy_from_slice(&value.to_le_bytes());
            Reader(&bad)
                .summary(7)
                .expect_err("reject invalid summary field");
        }
        Reader(&[255]).text().expect_err("reject short length");
        Reader(&[1, 0, 0, 0, 255])
            .text()
            .expect_err("reject invalid UTF-8");
        Reader(&[0xff; 12]).time().expect_err("reject invalid time");
    }
}
