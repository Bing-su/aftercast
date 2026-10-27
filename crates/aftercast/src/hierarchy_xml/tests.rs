use super::*;
use bytes::BufMut;
fn encounter() -> EncounterId {
    EncounterId {
        session: 7,
        value: 9,
    }
}
fn identity_bytes(revision: u64, depth: u32, tables: u32, rows: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.put_u64_le(revision);
    bytes.put_u64_le(9);
    bytes.put_u32_le(depth);
    bytes.put_u32_le(tables);
    bytes.put_u32_le(rows);
    bytes
}
#[test]
fn hierarchy_capture_rejects_scope_depth_counts_and_bad_presence() {
    let mut bytes = Vec::new();
    bytes.put_u32_le(1);
    bytes.extend(identity_bytes(2, 3, 4, 3));
    let capture = decode_capture(&bytes, encounter(), HierarchyXmlDepth::AllTables)
        .expect("decode")
        .expect("present");
    assert_eq!(capture.depth(), HierarchyXmlDepth::AllTables);
    assert_eq!(capture.table_count(), 4);
    assert_eq!(capture.row_count(), 3);
    for offset in [0, 12, 20, 24] {
        let mut bad = bytes.clone();
        bad[offset] ^= 128;
        decode_capture(&bad, encounter(), HierarchyXmlDepth::AllTables)
            .expect_err("identity/depth mismatch");
    }
    for (tables, rows) in [(0, 0), (1025, 1024), (1, 16385), (3, 1)] {
        let mut bad = vec![1, 0, 0, 0];
        bad.extend(identity_bytes(2, 3, tables, rows));
        decode_capture(&bad, encounter(), HierarchyXmlDepth::AllTables).expect_err("counts");
    }
    assert!(
        decode_capture(&[0, 0, 0, 0], encounter(), HierarchyXmlDepth::MainTable)
            .expect("missing")
            .is_none()
    );
    for bad in [vec![], vec![2, 0, 0, 0], vec![0, 0, 0, 0, 0]] {
        decode_capture(&bad, encounter(), HierarchyXmlDepth::MainTable)
            .expect_err("bad presence/framing");
    }
    for (revision, tables, rows) in [(130, 4, 3), (2, 4, 131), (9, 1, 0)] {
        let mut valid = vec![1, 0, 0, 0];
        valid.extend(identity_bytes(revision, 3, tables, rows));
        let capture = decode_capture(&valid, encounter(), HierarchyXmlDepth::AllTables)
            .expect("other legal catalog")
            .expect("present");
        assert_eq!(capture.revision, revision);
        assert_eq!(capture.rows, rows);
    }
    let mut bad = vec![1, 0, 0, 0];
    bad.extend(identity_bytes(0, 1, 1, 0));
    decode_capture(&bad, encounter(), HierarchyXmlDepth::MainTable).expect_err("zero revision");
}
#[test]
fn hierarchy_render_validates_opaque_selection_and_full_utf8_bound() {
    let selection = HierarchyXmlSelection {
        revision: 2,
        encounter: encounter(),
        depth: HierarchyXmlDepth::AllTables,
        tables: 4,
        rows: 3,
    };
    let report = |text: &str| {
        let mut bytes = identity_bytes(2, 3, 4, 3);
        bytes.put_u32_le(text.len() as u32);
        bytes.extend(text.as_bytes());
        bytes
    };
    let source = "<EncounterTable Name=\"한글日本😀&#xD;&#xA;\" />\r\n";
    assert_eq!(
        decode_report(&report(source), &selection)
            .expect("UTF8 source")
            .content,
        source
    );
    let maximum = "&".repeat(MAX_CONTENT);
    assert_eq!(
        decode_report(&report(&maximum), &selection)
            .expect("exact bytes")
            .content
            .len(),
        MAX_CONTENT
    );
    for bad in [
        report(""),
        report("x\0y"),
        report(&(maximum + "x")),
        {
            let mut b = report("x");
            b.push(0);
            b
        },
        {
            let mut b = report("x");
            *b.last_mut().expect("byte") = 255;
            b
        },
    ] {
        decode_report(&bad, &selection).expect_err("invalid source/framing");
    }
    for offset in [0, 8, 16, 20, 24] {
        let mut bad = report("source");
        bad[offset] ^= 1;
        decode_report(&bad, &selection).expect_err("captured identity/count mismatch");
    }
}
