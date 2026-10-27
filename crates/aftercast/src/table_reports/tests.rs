use super::*;
fn selection() -> TableReportSelection {
    TableReportSelection {
        session: 7,
        revision: 2,
        scope: TableScope::Combatants(EncounterId {
            session: 7,
            value: 9,
        }),
        rows: 3,
    }
}
fn options() -> TableReportOptions {
    TableReportOptions {
        format: TableReportFormat::TableXml,
        name: "한글\r\n😀".into(),
        selected_rows: vec![],
        columns: None,
        sort: None,
    }
}
fn report(content: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.put_u64_le(2);
    bytes.put_u32_le(0);
    bytes.put_u32_le(3);
    bytes.put_u32_le(content.len() as u32);
    bytes.extend_from_slice(content.as_bytes());
    bytes
}
#[test]
fn response_content_bounds_and_identity_are_checked_without_json_expansion() {
    let choice = selection();
    let options = options();
    let content = "한글日本😀\r\n";
    assert_eq!(
        decode_report(&report(content), &choice, &options)
            .expect("Unicode")
            .content,
        content
    );
    let maximum = "&".repeat(MAX_CONTENT);
    assert_eq!(
        decode_report(&report(&maximum), &choice, &options)
            .expect("exact bound")
            .content
            .len(),
        MAX_CONTENT
    );
    decode_report(&report(&(maximum + "x")), &choice, &options).expect_err("oversize");
    for offset in [0, 8, 12] {
        let mut bytes = report("x");
        bytes[offset] ^= 1;
        decode_report(&bytes, &choice, &options).expect_err("identity/count mismatch");
    }
    for bytes in [
        vec![],
        report("x\0y"),
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
        decode_report(&bytes, &choice, &options).expect_err("bad content encoding/framing");
    }
    let mut packet = vec![0; HEADER + REPORT_CAPACITY];
    packet[..4].copy_from_slice(&((HEADER + REPORT_CAPACITY) as u32).to_le_bytes());
    packet[4..8].copy_from_slice(&1u32.to_le_bytes());
    packet[8..16].copy_from_slice(&7u64.to_le_bytes());
    packet[16..20].copy_from_slice(&2u32.to_le_bytes());
    packet[24..28].copy_from_slice(&1u32.to_le_bytes());
    assert_eq!(decode_packet(&packet, 7, 2).expect("packet"), [0]);
    for offset in [0, 4, 8, 16, 28] {
        packet[offset] ^= 1;
        decode_packet(&packet, 7, 2).expect_err("envelope");
        packet[offset] ^= 1;
    }
    packet[24..28].copy_from_slice(&((REPORT_CAPACITY + 1) as u32).to_le_bytes());
    decode_packet(&packet, 7, 2).expect_err("oversize envelope");
    decode_packet(&[], 7, 2).expect_err("truncated envelope");
}
#[test]
fn rejects_invalid_selection_policy_and_catalogs() {
    let choice = selection();
    let mut option = options();
    option.selected_rows = vec![0, 0];
    option.validate(&choice).expect_err("duplicates");
    option.selected_rows = vec![3];
    option.validate(&choice).expect_err("outside owned capture");
    option.selected_rows.clear();
    option.columns = Some(vec!["fixture".into()]);
    option.validate(&choice).expect_err("XML SQL set");
    option.format = TableReportFormat::StaticHtml;
    option.validate(&choice).expect("HTML");
    option.selected_rows.push(0);
    option.validate(&choice).expect_err("HTML selection");
    scope_request(choice.scope(), 8).expect_err("other session");
    scope_request(&TableScope::Encounters, 7).expect_err("no archive mapping");
    assert!(
        decode_catalog(&0u32.to_le_bytes(), 7, choice.scope())
            .expect("absent")
            .is_none()
    );
    let mut bytes = Vec::new();
    bytes.put_u32_le(1);
    bytes.put_u64_le(2);
    bytes.put_u32_le(1);
    bytes.put_u64_le(9);
    for _ in 0..3 {
        bytes.put_u32_le(u32::MAX);
    }
    bytes.put_u32_le(0);
    bytes.put_u32_le(0);
    assert_eq!(
        decode_catalog(&bytes, 7, choice.scope())
            .expect("empty")
            .expect("catalog")
            .selection
            .row_count(),
        0
    );
    for offset in [0, 4, 12, 16] {
        let mut bad = bytes.clone();
        bad[offset] ^= 2;
        decode_catalog(&bad, 7, choice.scope()).expect_err("catalog identity");
    }
    let mut bad = bytes.clone();
    bad.extend([0]);
    decode_catalog(&bad, 7, choice.scope()).expect_err("trailing bytes");
    let last = bytes.len() - 4;
    bytes[last..].copy_from_slice(&16385u32.to_le_bytes());
    decode_catalog(&bytes, 7, choice.scope()).expect_err("row budget");
}

#[test]
fn hierarchy_commands_validate_their_own_envelope_and_capacity() {
    assert_eq!(capacity(3), 4096);
    assert_eq!(capacity(4), MAX_CONTENT + 32);
    for command in [3, 4] {
        let bound = capacity(command);
        let mut packet = vec![0; HEADER + bound];
        packet[..4].copy_from_slice(&((HEADER + bound) as u32).to_le_bytes());
        packet[4..8].copy_from_slice(&1u32.to_le_bytes());
        packet[8..16].copy_from_slice(&7u64.to_le_bytes());
        packet[16..20].copy_from_slice(&command.to_le_bytes());
        packet[24..28].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(
            decode_packet(&packet, 7, command).expect("valid envelope"),
            [0]
        );
        for offset in [0, 4, 8, 16, 28] {
            packet[offset] ^= 1;
            decode_packet(&packet, 7, command).expect_err("identity/version/reserved");
            packet[offset] ^= 1;
        }
        packet[24..28].copy_from_slice(&((bound + 1) as u32).to_le_bytes());
        decode_packet(&packet, 7, command).expect_err("reply exceeds command capacity");
        packet.pop();
        decode_packet(&packet, 7, command).expect_err("truncated fixed buffer");
    }
}
