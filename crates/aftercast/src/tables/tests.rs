use super::*;
use bytes::BufMut;
fn text(bytes: &mut Vec<u8>, value: Option<&str>) {
    bytes.put_u32_le(value.map_or(u32::MAX, |value| value.len() as u32));
    if let Some(value) = value {
        bytes.extend_from_slice(value.as_bytes());
    }
}
fn column(bytes: &mut Vec<u8>, key: &str) {
    for value in [key, "같은 라벨", "TEXT", "sql"] {
        text(bytes, Some(value));
    }
    bytes.put_u32_le(7);
}
fn query() -> TableQuery {
    TableQuery {
        scope: TableScope::Combatants(EncounterId {
            session: 9,
            value: 3,
        }),
        revision: TableRevision {
            session: 9,
            level: TableLevel::Combatant,
            value: 7,
        },
        columns: vec!["fixture".into()],
        offset: 0,
        limit: 16,
        sort: None,
    }
}
fn catalog_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.put_u32_le(1);
    bytes.put_u64_le(7);
    bytes.put_u32_le(1);
    column(&mut bytes, "fixture");
    bytes.put_u32_le(2);
    for outgoing in [true, false] {
        text(&mut bytes, Some("same-key"));
        text(&mut bytes, Some("same label"));
        bytes.put_u32_le(u32::from(outgoing));
        bytes.put_i32_le(-1);
        bytes.put_i32_le(i32::MIN);
    }
    bytes
}
fn page_bytes(cell_error: Option<&str>) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.put_u32_le(1);
    bytes.put_u32_le(1);
    bytes.put_u64_le(7);
    bytes.put_u32_le(1);
    bytes.put_u32_le(1);
    column(&mut bytes, "fixture");
    text(&mut bytes, None);
    bytes.put_u32_le(1);
    bytes.put_u32_le(0);
    bytes.put_u64_le(3);
    text(&mut bytes, Some("ACTOR"));
    text(&mut bytes, None);
    text(&mut bytes, None);
    bytes.put_u32_le(0);
    bytes.put_u32_le(0); // ally
    bytes.put_u32_le(0); // self
    bytes.put_u32_le(1);
    text(
        &mut bytes,
        if cell_error.is_some() {
            None
        } else {
            Some("valid text")
        },
    );
    bytes.put_u32_le(0);
    bytes.put_u32_le(0);
    text(&mut bytes, cell_error);
    bytes
}
#[test]
fn binary_catalog_and_cells_validate_ownership_lengths_and_error_status() {
    let catalog = catalog_bytes();
    let decoded = decode::catalog(&catalog, 9, TableLevel::Combatant).expect("catalog");
    assert_eq!(decoded.columns.first().expect("column").label, "같은 라벨");
    assert_ne!(
        decoded.categories.first().expect("outgoing").outgoing,
        decoded.categories.get(1).expect("incoming").outgoing
    );
    assert_eq!(
        decoded.categories.first().expect("category").color_argb,
        i32::MIN
    );
    let page = page_bytes(None);
    assert_eq!(
        decode::page(&page, 9, &query())
            .expect("page")
            .expect("scope")
            .rows
            .len(),
        1
    );
    let error = decode::page(&page_bytes(Some("column-error")), 9, &query())
        .expect("error page")
        .expect("scope");
    assert_eq!(
        error
            .rows
            .first()
            .expect("row")
            .cells
            .first()
            .expect("cell")
            .error,
        Some(TableCellError::Column)
    );
    decode::page(&page_bytes(Some("unknown")), 9, &query()).expect_err("unknown cell error");
    for length in 0..catalog.len() {
        decode::catalog(&catalog[..length], 9, TableLevel::Combatant)
            .expect_err("truncated catalog");
    }
    for length in 0..page.len() {
        decode::page(&page[..length], 9, &query()).expect_err("truncated page");
    }
    decode::catalog(&catalog, 9, TableLevel::Action).expect_err("wrong level");
    let mut trailing = page.clone();
    trailing.push(0);
    decode::page(&trailing, 9, &query()).expect_err("trailing bytes");
    let mut wrong = query();
    wrong.revision.value = 8;
    decode::page(&page, 9, &wrong).expect_err("wrong revision");
    wrong = query();
    wrong.columns = vec!["other".into()];
    decode::page(&page, 9, &wrong).expect_err("wrong columns");
    wrong = query();
    wrong.scope = TableScope::Combatants(EncounterId {
        session: 9,
        value: 4,
    });
    decode::page(&page, 9, &wrong).expect_err("wrong encounter");
    decode::page(&[0, 0, 0, 0], 9, &query()).expect("missing scope");
}
#[test]
fn table_requests_reject_foreign_sessions_levels_and_invalid_bounds_before_ipc() {
    let valid = query();
    valid.request(9).expect("request");
    valid.request(10).expect_err("foreign revision/session");
    let mut wrong = query();
    wrong.revision.level = TableLevel::Action;
    wrong.request(9).expect_err("wrong level");
    wrong = query();
    wrong.scope = TableScope::Combatants(EncounterId {
        session: 10,
        value: 3,
    });
    wrong.request(9).expect_err("foreign scope");
    for columns in [
        vec![],
        vec!["a".into(); 33],
        vec!["a".into(), "a".into()],
        vec!["NUL\0".into()],
    ] {
        let mut wrong = query();
        wrong.columns = columns;
        wrong.request(9).expect_err("invalid keys");
    }
    for limit in [0, 257] {
        let mut wrong = query();
        wrong.limit = limit;
        wrong.request(9).expect_err("invalid limit");
    }
    wrong = query();
    wrong.offset = u32::MAX;
    wrong.request(9).expect_err("invalid offset");
    wrong = query();
    wrong.sort = Some(TableSort {
        primary: String::new(),
        secondary: None,
        descending: false,
    });
    wrong.request(9).expect_err("empty sort key");
}

fn replace_number(bytes: &mut [u8], offset: usize, value: u32) {
    bytes
        .get_mut(offset..offset + 4)
        .expect("fixture field")
        .copy_from_slice(&value.to_le_bytes());
}
#[test]
fn malformed_binary_counts_flags_text_and_cell_status_are_rejected() {
    let mut encoded_column = Vec::new();
    column(&mut encoded_column, "fixture");
    let catalog = catalog_bytes();
    for (offset, value) in [
        (4, 0),
        (12, 257),
        (16, u32::MAX),
        (16 + encoded_column.len() - 4, 8),
        (16 + encoded_column.len(), 513),
    ] {
        let mut malformed = catalog.clone();
        replace_number(&mut malformed, offset, value);
        decode::catalog(&malformed, 9, TableLevel::Combatant).expect_err("invalid catalog field");
    }
    let mut encoding = catalog.clone();
    *encoding.get_mut(20).expect("key text") = 255;
    decode::catalog(&encoding, 9, TableLevel::Combatant).expect_err("invalid UTF8");
    let mut duplicate = Vec::new();
    duplicate.put_u32_le(1);
    duplicate.put_u64_le(7);
    duplicate.put_u32_le(2);
    column(&mut duplicate, "fixture");
    column(&mut duplicate, "fixture");
    duplicate.put_u32_le(0);
    decode::catalog(&duplicate, 9, TableLevel::Combatant).expect_err("duplicate key");
    let page = page_bytes(None);
    for (offset, value) in [
        (0, 2),
        (16, 16385),
        (24 + encoded_column.len() + 4, 257),
        (24 + encoded_column.len() - 4, 1),
    ] {
        let mut malformed = page.clone();
        replace_number(&mut malformed, offset, value);
        decode::page(&malformed, 9, &query()).expect_err("invalid page field/status");
    }
    let mut wrong_status = page.clone();
    wrong_status.truncate(wrong_status.len() - 4);
    text(&mut wrong_status, Some("column-error"));
    decode::page(&wrong_status, 9, &query()).expect_err("successful text mixed with error status");
    let mut sort_error = Vec::new();
    text(&mut sort_error, Some("comparison-budget"));
    let mut unsolicited = page;
    let offset = 24 + encoded_column.len();
    drop(unsolicited.splice(offset..offset + 4, sort_error));
    decode::page(&unsolicited, 9, &query()).expect_err("unsolicited sort status");
}

#[test]
fn main_sort_decoder_keeps_general_unsorted_contract_strict() {
    let mut page = Vec::new();
    page.put_u32_le(1);
    page.put_u32_le(1);
    page.put_u64_le(7);
    page.put_u32_le(2);
    page.put_u32_le(1);
    column(&mut page, "fixture");
    text(&mut page, None);
    page.put_u32_le(1);
    page.put_u32_le(1);
    page.put_u64_le(3);
    text(&mut page, Some("ACTOR"));
    text(&mut page, None);
    text(&mut page, None);
    page.put_u32_le(0);
    page.put_u32_le(0); // ally
    page.put_u32_le(0); // self
    page.put_u32_le(1);
    text(&mut page, Some("value"));
    page.put_u32_le(0);
    page.put_u32_le(0);
    text(&mut page, None);
    let mut request = query();
    request.limit = 1;
    decode::page(&page, 9, &request).expect_err("ordinary unsorted order");
    assert_eq!(
        decode::main_page(&page, 9, &request)
            .expect("main")
            .expect("scope")
            .rows[0]
            .index,
        1
    );
    let last = page.len() - 1;
    page.truncate(last);
    decode::main_page(&page, 9, &request).expect_err("truncated");
}

#[test]
fn header_click_response_rejects_bad_key_flags_and_trailing_bytes() {
    let mut bytes = Vec::new();
    text(&mut bytes, Some("new"));
    text(&mut bytes, Some("old"));
    bytes.put_u32_le(1);
    let result = decode_main_sort_click(&bytes, "new").expect("click");
    assert_eq!(result.secondary.as_deref(), Some("old"));
    assert!(result.descending);
    decode_main_sort_click(&bytes, "other").expect_err("wrong key");
    let mut malformed = bytes.clone();
    let n = malformed.len();
    malformed[n - 4..].copy_from_slice(&2u32.to_le_bytes());
    decode_main_sort_click(&malformed, "new").expect_err("invalid flag");
    bytes.push(0);
    decode_main_sort_click(&bytes, "new").expect_err("trailing bytes");
}
