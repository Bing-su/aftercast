use super::*;
#[test]
fn rejects_invalid_replies_and_preserves_unicode() {
    let encounter = EncounterId {
        session: 7,
        value: 9,
    };
    let revision = TextReportRevision {
        session: 7,
        value: 2,
    };
    for data in [
        b"{}".as_slice(),
        br#"{"encounterId":8,"revision":2,"text":"x"}"#,
        br#"{"encounterId":9,"revision":3,"text":"x"}"#,
        br#"{"encounterId":9,"revision":2,"text":"x","extra":1}"#,
        b"\xff",
    ] {
        decode_report(data, encounter, revision).expect_err("invalid selected report");
    }
    assert_eq!(
        decode_report(b"null", encounter, revision).expect("missing"),
        None
    );
    let data = serde_json::to_vec(
        &serde_json::json!({"encounterId":9,"revision":2,"text":"한글日本😀\n"}),
    )
    .expect("JSON");
    assert_eq!(
        decode_report(&data, encounter, revision)
            .expect("decode")
            .expect("report")
            .text,
        "한글日本😀\n"
    );
    let exact = serde_json::to_vec(
        &serde_json::json!({"encounterId":9,"revision":2,"text":"x".repeat(MAX_TEXT)}),
    )
    .expect("JSON");
    assert_eq!(
        decode_report(&exact, encounter, revision)
            .expect("exact maximum")
            .expect("report")
            .text
            .len(),
        MAX_TEXT
    );
    let large = serde_json::to_vec(
        &serde_json::json!({"encounterId":9,"revision":2,"text":"x".repeat(MAX_TEXT+1)}),
    )
    .expect("JSON");
    decode_report(&large, encounter, revision).expect_err("oversize report");
    for data in [
        br#"{"revision":0,"playerVariables":[],"alliedVariables":[],"sortColumns":[]}"#.as_slice(),
        br#"{"revision":1,"playerVariables":["x","x"],"alliedVariables":[],"sortColumns":[]}"#,
    ] {
        decode_catalog(data, 7).expect_err("invalid catalog");
    }
    decode_packet(&[], 7, 2).expect_err("empty envelope");
    let mut packet = vec![0; HEADER + CAPACITY];
    packet[..4].copy_from_slice(&((HEADER + CAPACITY) as u32).to_le_bytes());
    packet[4..8].copy_from_slice(&1_u32.to_le_bytes());
    packet[8..16].copy_from_slice(&7_u64.to_le_bytes());
    packet[16..20].copy_from_slice(&2_u32.to_le_bytes());
    packet[24..28].copy_from_slice(&4_u32.to_le_bytes());
    packet[HEADER..HEADER + 4].copy_from_slice(b"null");
    assert_eq!(
        decode_packet(&packet, 7, 2).expect("valid envelope"),
        b"null"
    );
    for offset in [0, 4, 8, 16, 28] {
        packet[offset] ^= 1;
        decode_packet(&packet, 7, 2).expect_err("wrong envelope field");
        packet[offset] ^= 1;
    }
    decode_packet(&packet[..packet.len() - 1], 7, 2).expect_err("short packet");
    packet[24..28].copy_from_slice(&((CAPACITY + 1) as u32).to_le_bytes());
    decode_packet(&packet, 7, 2).expect_err("oversize output");
    assert!(
        TextReportOptions {
            player_format: "x".repeat(16385),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        TextReportOptions {
            sorting: "bad\nkey".into(),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
}

#[test]
fn preset_catalog_validates_order_options_and_aggregate_budget() {
    let options = TextReportOptions {
        player_format: "한글{name}".into(),
        sorting: "Name".into(),
        ..Default::default()
    };
    let mut catalog = serde_json::json!({"revision":1,"playerVariables":[],"alliedVariables":[],"sortColumns":["Name"],"defaultOptions":options,"presets":[{"index":0,"options":options}]});
    let decode =
        |value: &serde_json::Value| decode_catalog(&serde_json::to_vec(value).expect("JSON"), 7);
    assert_eq!(
        decode(&catalog).expect("valid catalog").presets[0].options,
        options
    );
    catalog["presets"][0]["index"] = serde_json::json!(1);
    decode(&catalog).expect_err("index order mismatch");
    catalog["presets"][0]["index"] = serde_json::json!(0);
    catalog["defaultOptions"]["sorting"] = serde_json::json!("bad\nkey");
    decode(&catalog).expect_err("invalid default options");
    catalog["defaultOptions"] = serde_json::to_value(&options).expect("options");
    let large = TextReportOptions {
        player_format: "x".repeat(16384),
        allies_format: "y".repeat(16384),
        ..Default::default()
    };
    catalog["presets"] = serde_json::Value::Array(
        (0..33)
            .map(|index| serde_json::json!({"index":index,"options":large}))
            .collect(),
    );
    decode(&catalog).expect_err("aggregate budget");
    catalog["presets"] = serde_json::Value::Array(
        (0..1025)
            .map(|index| serde_json::json!({"index":index,"options":options}))
            .collect(),
    );
    decode(&catalog).expect_err("too many presets");
}
