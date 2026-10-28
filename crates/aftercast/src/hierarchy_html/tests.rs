use super::*;
use bytes::BufMut;
fn ids() -> Vec<EncounterId> {
    vec![
        EncounterId {
            session: 7,
            value: 9,
        },
        EncounterId {
            session: 7,
            value: 10,
        },
    ]
}
fn metadata(ids: &[u64], revision: u64, level: u32, tables: u32, rows: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.put_u64_le(revision);
    bytes.put_u32_le(level);
    bytes.put_u32_le(tables);
    bytes.put_u32_le(rows);
    bytes.put_u32_le(ids.len() as u32);
    for id in ids {
        bytes.put_u64_le(*id);
    }
    bytes
}
fn capture(ids: &[u64], revision: u64, level: u32, tables: u32, rows: u32) -> Vec<u8> {
    let mut bytes = vec![1, 0, 0, 0];
    bytes.extend(metadata(ids, revision, level, tables, rows));
    bytes
}
#[test]
fn html_capture_checks_set_counts_depth_and_allows_original_tree_order() {
    let bytes = capture(&[10, 9], 2, 4, 5, 3);
    let selected = decode_capture(&bytes, &ids(), TableLevel::Action, 7)
        .expect("decode")
        .expect("scope");
    assert_eq!(
        selected
            .encounters()
            .iter()
            .map(|id| id.value)
            .collect::<Vec<_>>(),
        [10, 9]
    );
    for (values, revision, level, tables, rows) in [
        (vec![9, 9], 2, 4, 5, 3),
        (vec![9, 11], 2, 4, 5, 3),
        (vec![9], 2, 4, 5, 3),
        (vec![9, 10], 0, 4, 5, 3),
        (vec![9, 10], 2, 0, 5, 3),
        (vec![9, 10], 2, 3, 5, 3),
        (vec![9, 10], 2, 4, 1, 3),
        (vec![9, 10], 2, 4, 1025, 1024),
        (vec![9, 10], 2, 4, 3, 0),
        (vec![9, 10], 2, 4, 2, 16385),
    ] {
        decode_capture(
            &capture(&values, revision, level, tables, rows),
            &ids(),
            TableLevel::Action,
            7,
        )
        .expect_err("invalid identity/count");
    }
    for bad in [vec![], vec![2, 0, 0, 0], vec![0, 0, 0, 0, 0]] {
        decode_capture(&bad, &ids(), TableLevel::Action, 7).expect_err("bad presence/frame");
    }
    assert!(
        decode_capture(&[0, 0, 0, 0], &ids(), TableLevel::Action, 7)
            .expect("missing")
            .is_none()
    );
    let maximum: Vec<_> = (1..=64)
        .map(|value| EncounterId { session: 7, value })
        .collect();
    let values: Vec<_> = maximum.iter().map(|id| id.value).collect();
    decode_capture(
        &capture(&values, 130, 1, 64, 0),
        &maximum,
        TableLevel::Combatant,
        7,
    )
    .expect("64 empty encounters");
    decode_capture(
        &capture(&[9, 10], 2, 1, 3, 1),
        &ids(),
        TableLevel::Combatant,
        7,
    )
    .expect_err("one root per main encounter");
}
#[test]
fn html_render_rejects_order_revision_content_and_option_policy_mismatch() {
    let selected = decode_capture(
        &capture(&[10, 9], 2, 4, 5, 3),
        &ids(),
        TableLevel::Action,
        7,
    )
    .expect("decode")
    .expect("scope");
    let report = |ids: &[u64], content: &str| {
        let mut bytes = metadata(ids, 2, 4, 5, 3);
        bytes.put_u32_le(content.len() as u32);
        bytes.extend(content.as_bytes());
        bytes
    };
    let text = "<!doctype html><p>&lt;script&gt; 한글日本😀</p>";
    assert_eq!(
        decode_report(&report(&[10, 9], text), &selected)
            .expect("source")
            .content,
        text
    );
    let maximum = "&".repeat(MAX_CONTENT);
    decode_report(&report(&[10, 9], &maximum), &selected).expect("exact byte bound");
    for bad in [
        report(&[9, 10], text),
        report(&[10, 9], ""),
        report(&[10, 9], "x\0y"),
        report(&[10, 9], &(maximum + "x")),
        {
            let mut b = report(&[10, 9], "x");
            b.push(0);
            b
        },
        {
            let mut b = report(&[10, 9], "x");
            *b.last_mut().expect("byte") = 255;
            b
        },
    ] {
        decode_report(&bad, &selected).expect_err("invalid content/order");
    }
    for offset in [0, 8, 12, 16] {
        let mut bad = report(&[10, 9], text);
        bad[offset] ^= 1;
        decode_report(&bad, &selected).expect_err("identity");
    }
    let mut levels = vec![HierarchyHtmlLevel {
        level: TableLevel::Combatant,
        columns: vec!["key".into()],
        sort: None,
    }];
    assert!(options_valid(&levels, TableLevel::Combatant));
    levels[0].columns.push("key".into());
    assert!(!options_valid(&levels, TableLevel::Combatant));
    levels[0].columns.pop();
    levels[0].level = TableLevel::Category;
    assert!(!options_valid(&levels, TableLevel::Combatant));
    levels.push(HierarchyHtmlLevel {
        level: TableLevel::Combatant,
        columns: vec!["key".into()],
        sort: None,
    });
    assert!(options_valid(&levels, TableLevel::Category));
    levels[0].sort = Some(TableSort {
        primary: "key".into(),
        secondary: None,
        descending: false,
    });
    assert!(!options_valid(&levels, TableLevel::Category));
    assert!(!ids_valid(&ids(), 8));
    assert!(!ids_valid(&[], 7));
    assert!(!ids_valid(&[ids()[0], ids()[0]], 7));
}
