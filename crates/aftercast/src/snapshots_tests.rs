use super::*;

const ZONE: &str = "영역\0🙂";
const TITLE: &str = "Title\0é";
const CHARACTER: &str = "Player𐐀";
const NAME: &str = "이름\0🙂";
const NAN_BITS: u64 = 0xfff8_0000_0000_1234;
const HIT_BITS: u32 = 0xffc0_1234;

fn u32_field(packet: &mut [u8], offset: usize, value: u32) {
    packet
        .get_mut(offset..offset + 4)
        .expect("fixture u32 field")
        .copy_from_slice(&value.to_le_bytes());
}

fn u64_field(packet: &mut [u8], offset: usize, value: u64) {
    packet
        .get_mut(offset..offset + 8)
        .expect("fixture u64 field")
        .copy_from_slice(&value.to_le_bytes());
}

fn actor_offset() -> usize {
    SNAPSHOT_HEADER + RESULT_HEADER + ZONE.len() + TITLE.len() + CHARACTER.len()
}

fn last_tick() -> u64 {
    u64::try_from(MAX_TICKS).expect("positive CLR maximum timestamp")
}

fn fixture() -> BytesMut {
    // Deliberately construct independent ABI bytes, including embedded NUL and nonfinite bit patterns.
    let mut payload = BytesMut::new();
    for value in [0, MAX_TICKS, i64::MIN, i64::MAX, -1234] {
        payload.put_i64_le(value);
    }
    payload.put_u32_le(2);
    payload.put_u32_le(1);
    let mut offset = RESULT_HEADER;
    for value in [ZONE, TITLE, CHARACTER] {
        payload.put_u32_le(offset as u32);
        payload.put_u32_le(value.len() as u32);
        offset += value.len();
    }
    payload.put_u32_le(offset as u32);
    payload.put_u32_le(0);
    for value in [ZONE, TITLE, CHARACTER] {
        payload.extend_from_slice(value.as_bytes());
    }
    payload.put_u32_le((ACTOR_HEADER + NAME.len()) as u32);
    payload.put_u32_le(1);
    for value in [i64::MIN, i64::MAX, -1, 0, -12345] {
        payload.put_i64_le(value);
    }
    for value in [
        NAN_BITS,
        f64::INFINITY.to_bits(),
        f64::NEG_INFINITY.to_bits(),
    ] {
        payload.put_u64_le(value);
    }
    payload.put_u32_le(HIT_BITS);
    for value in [i32::MIN, i32::MAX, -3, 4, -5, 6, -7, 8] {
        payload.put_i32_le(value);
    }
    payload.put_u32_le(ACTOR_HEADER as u32);
    payload.put_u32_le(NAME.len() as u32);
    payload.put_u32_le(0);
    payload.extend_from_slice(NAME.as_bytes());
    let mut packet = snapshot_packet(7, 2, SNAPSHOT_HEADER + payload.len());
    u64_field(&mut packet, 24, 5);
    u32_field(&mut packet, 32, 1);
    u32_field(&mut packet, 36, 3);
    u32_field(&mut packet, 40, payload.len() as u32);
    u64_field(&mut packet, 48, 2);
    packet
        .get_mut(SNAPSHOT_HEADER..)
        .expect("fixture payload")
        .copy_from_slice(&payload);
    packet
}

fn decoded(packet: &[u8]) -> CombatSnapshot {
    decode_snapshot(packet, 7, 2)
        .expect("valid snapshot")
        .expect("changed snapshot")
}

#[rstest::rstest]
fn decoded_result_borrows_metadata_but_owns_combatants() {
    let wire = fixture();
    let payload = wire.get(SNAPSHOT_HEADER..).expect("result payload");
    let result = decode_result(payload, 1).expect("validated result");
    let mut offset = RESULT_HEADER;
    for (actual, expected) in [
        (result.zone, ZONE),
        (result.title, TITLE),
        (result.character, CHARACTER),
    ] {
        assert_eq!(actual, expected);
        assert_eq!(
            actual.as_ptr(),
            payload.get(offset..).expect("wire text").as_ptr()
        );
        offset += expected.len();
    }
    let actors = result.combatants;
    drop(wire);
    assert_eq!(actors.first().expect("owned actor").name, NAME);
}

#[rstest::rstest]
fn owned_snapshot_preserves_unicode_nul_signed_values_and_float_bits() {
    let wire = fixture();
    let result = decoded(&wire);
    drop(wire);
    assert_eq!((result.zone.as_str(), result.title.as_str()), (ZONE, TITLE));
    assert_eq!(result.character, CHARACTER);
    assert_eq!(result.revision.session, 7);
    assert_eq!(result.revision.sequence(), 5);
    assert_eq!(result.superseded_versions, 2);
    assert!(result.is_import);
    assert_eq!(result.status, CombatSnapshotStatus::Ended);
    assert_eq!(
        (result.duration_ticks, result.damage, result.healed),
        (i64::MIN, i64::MAX, -1234)
    );
    let actor = result.combatants.first().expect("one owned actor");
    assert_eq!(actor.name, NAME);
    assert!(actor.is_ally);
    assert_eq!(
        (
            actor.damage,
            actor.healed,
            actor.damage_taken,
            actor.heals_taken
        ),
        (i64::MIN, i64::MAX, -1, 0)
    );
    assert_eq!(actor.duration_ticks, -12345);
    assert_eq!(actor.dps.to_bits(), NAN_BITS);
    assert_eq!(actor.enc_dps.to_bits(), f64::INFINITY.to_bits());
    assert_eq!(actor.enc_hps.to_bits(), f64::NEG_INFINITY.to_bits());
    assert_eq!(actor.to_hit.to_bits(), HIT_BITS);
    assert_eq!(
        [
            actor.swings,
            actor.hits,
            actor.crit_hits,
            actor.heals,
            actor.misses,
            actor.blocked,
            actor.kills,
            actor.deaths
        ],
        [i32::MIN, i32::MAX, -3, 4, -5, 6, -7, 8]
    );
    let mut negative_zero = fixture();
    u64_field(
        &mut negative_zero,
        actor_offset() + 48,
        (-0.0_f64).to_bits(),
    );
    u32_field(
        &mut negative_zero,
        actor_offset() + 72,
        f32::INFINITY.to_bits(),
    );
    let result = decoded(&negative_zero);
    let actor = result.combatants.first().expect("negative zero actor");
    assert_eq!(actor.dps.to_bits(), (-0.0_f64).to_bits());
    assert_eq!(actor.to_hit.to_bits(), f32::INFINITY.to_bits());
}

#[rstest::rstest]
#[case::unspecified(0, LogTimeKind::Unspecified, None)]
#[case::utc(1, LogTimeKind::Utc, Some(123456))]
#[case::local(2, LogTimeKind::Local, None)]
fn graph_times_retain_kind_without_inventing_local_utc_offsets(
    #[case] kind: u32,
    #[case] expected: LogTimeKind,
    #[case] utc_ticks: Option<u64>,
) {
    let mut packet = fixture();
    u64_field(&mut packet, SNAPSHOT_HEADER, 123456);
    u32_field(&mut packet, SNAPSHOT_HEADER + 40, kind);
    let result = decoded(&packet);
    assert_eq!(
        result.start_time,
        LogTimestamp {
            ticks: 123456,
            utc_ticks,
            kind: expected
        }
    );
    assert_eq!(
        result.end_time,
        LogTimestamp {
            ticks: last_tick(),
            utc_ticks: Some(last_tick()),
            kind: LogTimeKind::Utc
        }
    );
}

#[rstest::rstest]
fn status_revision_and_unchanged_results_distinguish_coalescing_from_loss() {
    for (flags, is_import, status) in [
        (1, false, CombatSnapshotStatus::Ended),
        (3, true, CombatSnapshotStatus::Ended),
        (7, true, CombatSnapshotStatus::Importing),
        (11, true, CombatSnapshotStatus::PartialImport),
    ] {
        let mut packet = fixture();
        u32_field(&mut packet, 36, flags);
        let result = decoded(&packet);
        assert_eq!((result.is_import, result.status), (is_import, status));
    }
    let mut initial = fixture();
    u64_field(&mut initial, 48, 0);
    let result = decode_snapshot(&initial, 7, 0)
        .expect("initial snapshot")
        .expect("present");
    assert_eq!(result.superseded_versions, 0);
    let mut adjacent = fixture();
    u64_field(&mut adjacent, 24, 3);
    u64_field(&mut adjacent, 48, 0);
    assert_eq!(decoded(&adjacent).superseded_versions, 0);
    let unchanged = snapshot_packet(7, 5, SNAPSHOT_HEADER);
    assert!(
        decode_snapshot(&unchanged, 7, 5)
            .expect("unchanged query")
            .is_none()
    );
    let mut padded = fixture();
    padded.resize(padded.len() + 32, 0xa5);
    assert_eq!(decoded(&padded).combatants.len(), 1);
}

#[rstest::rstest]
fn every_truncated_prefix_is_rejected_without_panicking() {
    let packet = fixture();
    for length in 0..packet.len() {
        assert!(matches!(
            decode_snapshot(packet.get(..length).expect("fixture prefix"), 7, 2),
            Err(Error::InvalidSnapshotResponse(_))
        ));
    }
}

#[rstest::rstest]
fn malformed_headers_ranges_and_text_cannot_masquerade_as_snapshot_data() {
    let valid = fixture();
    let actor = actor_offset();
    for (offset, value) in [
        (32, 65537),
        (32, 0),
        (32, 2),
        (36, 0),
        (36, 2),
        (36, 5),
        (36, 9),
        (36, 15),
        (40, u32::MAX),
        (40, 79),
        (44, 1),
        (56, 1),
        (SNAPSHOT_HEADER + 40, 3),
        (SNAPSHOT_HEADER + 44, 3),
        (SNAPSHOT_HEADER + 48, 0),
        (SNAPSHOT_HEADER + 52, u32::MAX),
        (SNAPSHOT_HEADER + 56, 0),
        (SNAPSHOT_HEADER + 60, u32::MAX),
        (SNAPSHOT_HEADER + 64, 0),
        (SNAPSHOT_HEADER + 68, u32::MAX),
        (SNAPSHOT_HEADER + 72, u32::MAX),
        (SNAPSHOT_HEADER + 76, 1),
        (actor, 0),
        (actor, 119),
        (actor, u32::MAX),
        (actor + 4, 2),
        (actor + 108, 0),
        (actor + 112, u32::MAX),
        (actor + 116, 1),
    ] {
        let mut malformed = valid.clone();
        u32_field(&mut malformed, offset, value);
        assert!(
            matches!(
                decode_snapshot(&malformed, 7, 2),
                Err(Error::InvalidSnapshotResponse(_))
            ),
            "accepted field {offset}={value}"
        );
    }
    for (offset, value) in [
        (24, 0),
        (24, 2),
        (48, 0),
        (48, 3),
        (56, 1),
        (SNAPSHOT_HEADER, u64::MAX),
        (SNAPSHOT_HEADER, last_tick() + 1),
        (SNAPSHOT_HEADER + 8, last_tick() + 1),
    ] {
        let mut malformed = valid.clone();
        u64_field(&mut malformed, offset, value);
        assert!(matches!(
            decode_snapshot(&malformed, 7, 2),
            Err(Error::InvalidSnapshotResponse(_))
        ));
    }
    let zone = SNAPSHOT_HEADER + RESULT_HEADER;
    for offset in [
        zone,
        zone + ZONE.len(),
        zone + ZONE.len() + TITLE.len(),
        actor + ACTOR_HEADER,
    ] {
        let mut malformed = valid.clone();
        *malformed.get_mut(offset).expect("fixture text byte") = 255;
        assert!(matches!(
            decode_snapshot(&malformed, 7, 2),
            Err(Error::InvalidSnapshotResponse(_))
        ));
    }
    let mut trailing = valid.clone();
    trailing.put_u8(0);
    u32_field(
        &mut trailing,
        40,
        (valid.len() - SNAPSHOT_HEADER + 1) as u32,
    );
    assert!(matches!(
        decode_snapshot(&trailing, 7, 2),
        Err(Error::InvalidSnapshotResponse(_))
    ));
}

#[rstest::rstest]
fn unchanged_metadata_and_capacity_growth_are_validated_without_large_allocations() {
    let unchanged = snapshot_packet(7, 5, SNAPSHOT_HEADER);
    for offset in [24, 32, 40, 48, 56] {
        let mut malformed = unchanged.clone();
        u32_field(&mut malformed, offset, 1);
        assert!(matches!(
            decode_snapshot(&malformed, 7, 5),
            Err(Error::InvalidSnapshotResponse(_))
        ));
    }
    for required in [0, 63, 64, MAX_PACKET as u32 + 1] {
        let mut packet = unchanged.clone();
        u32_field(&mut packet, 44, required);
        assert!(matches!(
            larger_snapshot_capacity(&packet),
            Err(Error::InvalidSnapshotResponse(_))
        ));
    }
    let mut packet = unchanged;
    for (required, expected) in [(65, 128), (256, 256), (MAX_PACKET, MAX_PACKET)] {
        u32_field(&mut packet, 44, required as u32);
        assert_eq!(
            larger_snapshot_capacity(&packet).expect("bounded growth"),
            expected
        );
    }
    assert!(matches!(
        larger_snapshot_capacity(&[]),
        Err(Error::InvalidSnapshotResponse(_))
    ));
}
