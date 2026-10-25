use bytes::{Buf, BufMut, BytesMut};

use crate::runtime::Request;
use crate::{Error, Host, LogTimeKind, LogTimestamp};

pub(crate) const SNAPSHOT_HEADER: usize = 64;
const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_PACKET: usize = SNAPSHOT_HEADER + MAX_BYTES;
const RESULT_HEADER: usize = 80;
const ACTOR_HEADER: usize = 120;
const MAX_TICKS: i64 = 3_155_378_975_999_999_999;

/// Session-scoped version of the latest combat result, including import status updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CombatRevision {
    pub(crate) session: u64,
    pub(crate) sequence: u64,
}

impl CombatRevision {
    /// Monotonic state version, not a count of encounters or parsed records.
    pub fn sequence(self) -> u64 {
        self.sequence
    }
}

/// Core completion state; none of these values proves plugin-private parser acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatSnapshotStatus {
    /// Combat ended and any enclosing import completed without a known host failure.
    Ended,
    /// Combat ended while its enclosing file import is still running.
    Importing,
    /// Partial history retained after a known input, hook or aggregation failure during import.
    PartialImport,
}

/// Actual ACT counters for one participant; signed values and nonfinite rates are preserved.
#[derive(Debug, Clone)]
pub struct CombatantSnapshot {
    pub name: String,
    pub is_ally: bool,
    pub damage: i64,
    pub healed: i64,
    pub damage_taken: i64,
    pub heals_taken: i64,
    /// Signed ACT duration in 100 ns ticks, not an unsigned Rust Duration.
    pub duration_ticks: i64,
    pub dps: f64,
    pub enc_dps: f64,
    pub enc_hps: f64,
    /// Hit percentage, including ACT's NaN result for empty data.
    pub to_hit: f32,
    pub swings: i32,
    pub hits: i32,
    pub crit_hits: i32,
    pub heals: i32,
    pub misses: i32,
    pub blocked: i32,
    pub kills: i32,
    pub deaths: i32,
}

/// An owned view of the latest ended encounter, captured consistently with ACT aggregation.
#[derive(Debug, Clone)]
pub struct CombatSnapshot {
    pub revision: CombatRevision,
    /// Intermediate state versions superseded since the supplied revision; not lost combat input.
    pub superseded_versions: u64,
    pub is_import: bool,
    pub status: CombatSnapshotStatus,
    pub zone: String,
    pub title: String,
    pub character: String,
    /// ACT graph times preserve ticks/kind; local times have no reconstructed UTC offset.
    pub start_time: LogTimestamp,
    pub end_time: LogTimestamp,
    pub duration_ticks: i64,
    pub damage: i64,
    pub healed: i64,
    pub combatants: Vec<CombatantSnapshot>,
}

// Borrow wire metadata until a caller needs ownership, e.g. encounter detail only retains combatants.
pub(crate) struct DecodedResult<'a> {
    pub(crate) zone: &'a str,
    pub(crate) title: &'a str,
    pub(crate) character: &'a str,
    pub(crate) start_time: LogTimestamp,
    pub(crate) end_time: LogTimestamp,
    pub(crate) duration_ticks: i64,
    pub(crate) damage: i64,
    pub(crate) healed: i64,
    pub(crate) combatants: Vec<CombatantSnapshot>,
}

pub(crate) fn snapshot_packet(session: u64, after: u64, capacity: usize) -> BytesMut {
    let mut packet = BytesMut::with_capacity(capacity);
    packet.put_u32_le(capacity as u32);
    packet.put_u32_le(1);
    packet.put_u64_le(session);
    packet.put_u64_le(after);
    packet.resize(capacity, 0);
    packet
}

fn invalid(message: &str) -> Error {
    Error::InvalidSnapshotResponse(message.to_owned())
}

pub(crate) fn larger_snapshot_capacity(packet: &[u8]) -> Result<usize, Error> {
    let mut field = packet
        .get(44..48)
        .ok_or_else(|| invalid("missing required snapshot capacity"))?;
    let required = field.get_u32_le() as usize;
    if required <= packet.len() || required > MAX_PACKET {
        return Err(invalid("invalid required snapshot capacity"));
    }
    Ok(required.max(packet.len().saturating_mul(2)).min(MAX_PACKET))
}

pub(crate) fn graph_time(ticks: i64, kind: u32) -> Result<LogTimestamp, Error> {
    if !(0..=MAX_TICKS).contains(&ticks) {
        return Err(invalid("invalid graph timestamp"));
    }
    let kind = match kind {
        0 => LogTimeKind::Unspecified,
        1 => LogTimeKind::Utc,
        2 => LogTimeKind::Local,
        _ => return Err(invalid("unknown graph timestamp kind")),
    };
    let ticks = u64::try_from(ticks).map_err(|_negative| invalid("negative graph timestamp"))?;
    // The ACT graph retains no historical timezone offset; only UTC kind establishes a UTC instant here.
    Ok(LogTimestamp {
        ticks,
        utc_ticks: (kind == LogTimeKind::Utc).then_some(ticks),
        kind,
    })
}

fn text(payload: &[u8], start: usize, end: usize) -> Result<&str, Error> {
    let bytes = payload
        .get(start..end)
        .ok_or_else(|| invalid("snapshot text exceeds capacity"))?;
    std::str::from_utf8(bytes).map_err(|_encoding| invalid("snapshot text is not UTF-8"))
}

fn actors(
    payload: &[u8],
    mut offset: usize,
    count: usize,
) -> Result<Vec<CombatantSnapshot>, Error> {
    let mut actors = Vec::with_capacity(count);
    for _ in 0..count {
        let mut fields = payload
            .get(offset..)
            .and_then(|bytes| bytes.get(..ACTOR_HEADER))
            .ok_or_else(|| invalid("truncated combatant header"))?;
        let length = fields.get_u32_le() as usize;
        let flags = fields.get_u32_le();
        let damage = fields.get_i64_le();
        let healed = fields.get_i64_le();
        let damage_taken = fields.get_i64_le();
        let heals_taken = fields.get_i64_le();
        let duration_ticks = fields.get_i64_le();
        let dps = fields.get_f64_le();
        let enc_dps = fields.get_f64_le();
        let enc_hps = fields.get_f64_le();
        let to_hit = fields.get_f32_le();
        let swings = fields.get_i32_le();
        let hits = fields.get_i32_le();
        let crit_hits = fields.get_i32_le();
        let heals = fields.get_i32_le();
        let misses = fields.get_i32_le();
        let blocked = fields.get_i32_le();
        let kills = fields.get_i32_le();
        let deaths = fields.get_i32_le();
        let name_offset = fields.get_u32_le() as usize;
        let name_length = fields.get_u32_le() as usize;
        let reserved = fields.get_u32_le();
        if flags > 1
            || reserved != 0
            || name_offset != ACTOR_HEADER
            || name_offset.checked_add(name_length) != Some(length)
        {
            return Err(invalid("invalid combatant ranges/flags"));
        }
        let name_start = offset
            .checked_add(name_offset)
            .ok_or_else(|| invalid("combatant offset overflow"))?;
        let end = offset
            .checked_add(length)
            .ok_or_else(|| invalid("combatant length overflow"))?;
        let name = text(payload, name_start, end)?.to_owned();
        actors.push(CombatantSnapshot {
            name,
            is_ally: flags == 1,
            damage,
            healed,
            damage_taken,
            heals_taken,
            duration_ticks,
            dps,
            enc_dps,
            enc_hps,
            to_hit,
            swings,
            hits,
            crit_hits,
            heals,
            misses,
            blocked,
            kills,
            deaths,
        });
        offset = end;
    }
    if offset != payload.len() {
        return Err(invalid("unexpected trailing combatant bytes"));
    }
    Ok(actors)
}

pub(crate) fn decode_snapshot(
    packet: &[u8],
    session: u64,
    after: u64,
) -> Result<Option<CombatSnapshot>, Error> {
    let mut header = packet
        .get(24..64)
        .ok_or_else(|| invalid("missing snapshot header"))?;
    let revision = header.get_u64_le();
    let count = header.get_u32_le() as usize;
    let flags = header.get_u32_le();
    let used = header.get_u32_le() as usize;
    let required = header.get_u32_le();
    let superseded_versions = header.get_u64_le();
    let reserved = header.get_u64_le();
    if reserved != 0 || required != 0 || used > MAX_BYTES || count > 65536 {
        return Err(invalid("invalid snapshot header fields"));
    }
    if flags == 0 {
        if revision != 0 || count != 0 || used != 0 || superseded_versions != 0 {
            return Err(invalid("nonempty unchanged snapshot"));
        }
        return Ok(None);
    }
    let status = match flags >> 2 {
        0 => CombatSnapshotStatus::Ended,
        1 => CombatSnapshotStatus::Importing,
        2 => CombatSnapshotStatus::PartialImport,
        _ => return Err(invalid("unknown snapshot status")),
    };
    let is_import = flags & 2 != 0;
    if flags & 1 == 0
        || !is_import && status != CombatSnapshotStatus::Ended
        || revision <= after
        || superseded_versions != if after == 0 { 0 } else { revision - after - 1 }
    {
        return Err(invalid("invalid snapshot revision/status"));
    }
    let payload = packet
        .get(SNAPSHOT_HEADER..)
        .and_then(|bytes| bytes.get(..used))
        .ok_or_else(|| invalid("snapshot payload exceeds capacity"))?;
    let result = decode_result(payload, count)?;
    Ok(Some(CombatSnapshot {
        revision: CombatRevision {
            session,
            sequence: revision,
        },
        superseded_versions,
        is_import,
        status,
        zone: result.zone.to_owned(),
        title: result.title.to_owned(),
        character: result.character.to_owned(),
        start_time: result.start_time,
        end_time: result.end_time,
        duration_ticks: result.duration_ticks,
        damage: result.damage,
        healed: result.healed,
        combatants: result.combatants,
    }))
}

pub(crate) fn decode_result(payload: &[u8], count: usize) -> Result<DecodedResult<'_>, Error> {
    let used = payload.len();
    if count > 65536 {
        return Err(invalid("too many combatants"));
    }
    let mut fields = payload
        .get(..RESULT_HEADER)
        .ok_or_else(|| invalid("missing result header"))?;
    let start = fields.get_i64_le();
    let end = fields.get_i64_le();
    let duration_ticks = fields.get_i64_le();
    let damage = fields.get_i64_le();
    let healed = fields.get_i64_le();
    let start_time = graph_time(start, fields.get_u32_le())?;
    let end_time = graph_time(end, fields.get_u32_le())?;
    let zone_offset = fields.get_u32_le() as usize;
    let zone_length = fields.get_u32_le() as usize;
    let title_offset = fields.get_u32_le() as usize;
    let title_length = fields.get_u32_le() as usize;
    let character_offset = fields.get_u32_le() as usize;
    let character_length = fields.get_u32_le() as usize;
    let actors_offset = fields.get_u32_le() as usize;
    if fields.get_u32_le() != 0
        || zone_offset != RESULT_HEADER
        || zone_offset.checked_add(zone_length) != Some(title_offset)
        || title_offset.checked_add(title_length) != Some(character_offset)
        || character_offset.checked_add(character_length) != Some(actors_offset)
        || actors_offset > used
        || count > (used - actors_offset) / ACTOR_HEADER
    {
        return Err(invalid("invalid snapshot string/actor ranges"));
    }
    Ok(DecodedResult {
        zone: text(payload, zone_offset, title_offset)?,
        title: text(payload, title_offset, character_offset)?,
        character: text(payload, character_offset, actors_offset)?,
        start_time,
        end_time,
        duration_ticks,
        damage,
        healed,
        combatants: actors(payload, actors_offset, count)?,
    })
}

impl Host {
    /// Copies the latest ended encounter, or returns None when no result is newer than `after`.
    ///
    /// Pass None for the latest result, then its revision to poll for changes. Intermediate state
    /// versions coalesce; this is not an encounter archive. Results own their strings and counters
    /// after host stop. Imports can retain partial results after failure; inspect `status`.
    pub fn combat_snapshot(
        &mut self,
        after: Option<CombatRevision>,
    ) -> Result<Option<CombatSnapshot>, Error> {
        if after.is_some_and(|revision| revision.session != self.session.id()) {
            return Err(Error::InvalidSnapshotRequest(
                "revision belongs to another host session".to_owned(),
            ));
        }
        let after = after.map_or(0, |revision| revision.sequence);
        let mut capacity = 64 * 1024;
        loop {
            let mut packet = snapshot_packet(self.session.id(), after, capacity);
            if self.session.request(Request::Snapshot, &mut packet)? == 0 {
                return decode_snapshot(&packet, self.session.id(), after);
            }
            capacity = larger_snapshot_capacity(&packet)?;
        }
    }
}

#[cfg(test)]
#[path = "snapshots_tests.rs"]
mod tests;
