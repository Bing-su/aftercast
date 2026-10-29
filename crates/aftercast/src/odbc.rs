//! Offline synthetic SQL plans. This API has no driver, credentials or connection operation.
use crate::{
    Error, HierarchyHtmlSelection, HierarchyXmlDepth, HierarchyXmlSelection, HostHandle,
    TableLevel, TableReportSelection, TableScope, tables::decode::Reader,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OdbcColumn {
    pub name: String,
    pub sql_type: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OdbcPreview {
    pub table: String,
    pub create_sql: String,
    pub insert_sql: String,
    pub reset_sql: Option<String>,
    pub columns: Vec<OdbcColumn>,
    pub parameters: Vec<Vec<String>>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OdbcHierarchyPreview {
    pub tables: Vec<OdbcPreview>,
}
/// Regex rules operate on offline parameterized templates and the fixed synthetic connection label.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OdbcRewriteRule {
    pub connection_pattern: String,
    pub find: String,
    pub replacement: String,
}
fn invalid(value: impl std::fmt::Display) -> Error {
    Error::InvalidReportRequest(value.to_string())
}
fn valid_names(names: &[String]) -> bool {
    (1..=256).contains(&names.len())
        && names
            .iter()
            .all(|s| !s.is_empty() && s.len() <= 1024 && !s.contains('\0'))
}
impl HostHandle {
    /// Build all five offline plans once using a full-depth, session-owned hierarchy capture.
    pub async fn odbc_hierarchy_preview(
        &self,
        selection: &HierarchyXmlSelection,
        columns: &[Vec<String>; 5],
        all_term: &str,
    ) -> Result<OdbcHierarchyPreview, Error> {
        if selection.encounter().session != self.session
            || selection.depth() != HierarchyXmlDepth::AllTables
            || selection.revision == 0
            || all_term.len() > 1024
            || all_term.contains('\0')
            || columns.iter().any(|v| !valid_names(v))
        {
            return Err(invalid(
                "invalid ODBC hierarchy session, selection or columns",
            ));
        }
        let response=self.table_report_call(7,&serde_json::json!({"revision":selection.revision,"odbcHierarchy":{"destinationColumns":columns,"allTerm":all_term}})).await?;
        let tables = decode(
            &response,
            selection.revision,
            columns,
            selection.row_count(),
            false,
            1,
        )?;
        Ok(OdbcHierarchyPreview { tables })
    }
    /// Build one bounded, atomic five-table preview for a captured encounter set.
    pub async fn odbc_batch_preview(
        &self,
        selection: &HierarchyHtmlSelection,
        columns: &[Vec<String>; 5],
        all_term: &str,
        rules: &[OdbcRewriteRule],
    ) -> Result<OdbcHierarchyPreview, Error> {
        if selection.session != self.session
            || selection.revision == 0
            || selection.last_level() != TableLevel::Action
            || selection.encounters().is_empty()
            || selection.encounters().len() > 64
            || all_term.len() > 1024
            || all_term.contains('\0')
            || columns.iter().any(|v| !valid_names(v))
            || rules.len() > 16
            || rules.iter().any(|r| {
                r.connection_pattern.len() > 512
                    || r.find.is_empty()
                    || r.find.len() > 512
                    || r.replacement.len() > 1024
                    || r.connection_pattern.contains('\0')
                    || r.find.contains('\0')
                    || r.replacement.contains('\0')
            })
        {
            return Err(invalid("invalid ODBC batch session, selection or columns"));
        }
        let response = self.table_report_call(9, &serde_json::json!({"revision":selection.revision,"odbcHierarchy":{"destinationColumns":columns,"allTerm":all_term,"rules":rules}})).await?;
        let tables = decode(
            &response,
            selection.revision,
            columns,
            selection.row_count(),
            false,
            selection.encounters().len() as u32,
        )?;
        Ok(OdbcHierarchyPreview { tables })
    }
    /// Build the inert current-table reset and insert plan. No SQL is executed.
    pub async fn odbc_current_preview(
        &self,
        selection: &TableReportSelection,
        columns: &[String],
    ) -> Result<OdbcPreview, Error> {
        if selection.session != self.session
            || selection.revision == 0
            || !matches!(selection.scope(), TableScope::Combatants(_))
            || !valid_names(columns)
        {
            return Err(invalid("invalid ODBC current selection or columns"));
        }
        let response = self
            .table_report_call(
                8,
                &serde_json::json!({"revision":selection.revision,"odbcCurrent":columns}),
            )
            .await?;
        let column_sets = [columns.to_vec()];
        decode(
            &response,
            selection.revision,
            &column_sets,
            selection.row_count(),
            true,
            0,
        )?
        .into_iter()
        .next()
        .ok_or_else(|| invalid("missing current table"))
    }
}
fn decode(
    bytes: &[u8],
    revision: u64,
    columns: &[Vec<String>],
    max_rows: u32,
    current: bool,
    encounter_rows: u32,
) -> Result<Vec<OdbcPreview>, Error> {
    let expected: &[&str] = if current {
        &["current_table"]
    } else {
        &[
            "encounter_table",
            "combatant_table",
            "damagetype_table",
            "attacktype_table",
            "swing_table",
        ]
    };
    let mut reader = Reader(bytes);
    if reader.u64()? != revision
        || reader.u32()? as usize != expected.len()
        || columns.len() != expected.len()
    {
        return Err(invalid("ODBC revision or table count mismatch"));
    }
    let mut output = Vec::with_capacity(expected.len());
    let mut total_rows = 0_u32;
    let mut text_bytes = 0_usize;
    for (index, expected_table) in expected.iter().enumerate() {
        let table = reader.text(1024)?;
        let create_sql = reader.text(512 * 1024)?;
        let insert_sql = reader.text(512 * 1024)?;
        let reset_sql = reader.optional_text(1024)?;
        if table != *expected_table
            || reset_sql.as_deref()
                != if current {
                    Some("DELETE FROM \"current_table\";")
                } else {
                    None
                }
        {
            return Err(invalid("ODBC table identity mismatch"));
        }
        let count = reader.u32()? as usize;
        let expected_columns = columns
            .get(index)
            .ok_or_else(|| invalid("missing column set"))?;
        if count != expected_columns.len() || !(1..=256).contains(&count) {
            return Err(invalid("ODBC column count mismatch"));
        }
        let mut descriptors = Vec::with_capacity(count);
        for name in expected_columns {
            let actual = reader.text(1024)?;
            let sql_type = reader.text(1024)?;
            if actual != *name || sql_type.is_empty() {
                return Err(invalid("ODBC column identity mismatch"));
            }
            descriptors.push(OdbcColumn {
                name: actual,
                sql_type,
            });
        }
        let rows = reader.u32()?;
        total_rows = total_rows
            .checked_add(rows)
            .ok_or_else(|| invalid("ODBC row overflow"))?;
        if rows > 16384
            || total_rows > max_rows.saturating_add(encounter_rows)
            || (!current && index == 0 && rows != encounter_rows)
        {
            return Err(invalid("ODBC row count mismatch"));
        }
        let parameters = read_parameters(&mut reader, rows, count, &mut text_bytes)?;
        output.push(OdbcPreview {
            table,
            create_sql,
            insert_sql,
            reset_sql,
            columns: descriptors,
            parameters,
        });
    }
    reader.finish()?;
    Ok(output)
}

fn read_parameters(
    reader: &mut Reader<'_>,
    rows: u32,
    columns: usize,
    text_bytes: &mut usize,
) -> Result<Vec<Vec<String>>, Error> {
    let mut parameters = Vec::with_capacity(rows as usize);
    for _ in 0..rows {
        let mut row = Vec::with_capacity(columns);
        for _ in 0..columns {
            let value = reader.text(4096)?;
            *text_bytes = text_bytes
                .checked_add(value.len())
                .ok_or_else(|| invalid("ODBC text overflow"))?;
            if *text_bytes > 8 * 1024 * 1024 {
                return Err(invalid("ODBC text limit"));
            }
            row.push(value);
        }
        parameters.push(row);
    }
    Ok(parameters)
}

#[cfg(test)]
mod tests {
    use super::decode;
    use bytes::BufMut;
    fn text(out: &mut Vec<u8>, value: Option<&str>) {
        match value {
            Some(value) => {
                out.put_u32_le(value.len() as u32);
                out.extend_from_slice(value.as_bytes());
            }
            None => out.put_u32_le(u32::MAX),
        }
    }
    fn response() -> Vec<u8> {
        let mut out = Vec::new();
        out.put_u64_le(12);
        out.put_u32_le(1);
        text(&mut out, Some("current_table"));
        text(&mut out, Some("CREATE"));
        text(&mut out, Some("INSERT"));
        text(&mut out, Some("DELETE FROM \"current_table\";"));
        out.put_u32_le(1);
        text(&mut out, Some("value"));
        text(&mut out, Some("TEXT"));
        out.put_u32_le(1);
        text(&mut out, Some("safe"));
        out
    }
    #[test]
    fn malformed_odbc_responses_are_rejected() {
        let bytes = response();
        let columns = [vec!["value".into()]];
        assert_eq!(
            decode(&bytes, 12, &columns, 1, true, 0)
                .expect("valid")
                .len(),
            1
        );
        decode(&bytes, 13, &columns, 1, true, 0).expect_err("wrong revision");
        decode(&bytes, 12, &columns, 0, true, 0).expect_err("too many rows");
        decode(&bytes, 12, &[vec!["wrong".into()]], 1, true, 0).expect_err("wrong column");
        for n in 0..bytes.len() {
            decode(&bytes[..n], 12, &columns, 1, true, 0).expect_err("truncated");
        }
        let mut trailing = bytes;
        trailing.push(0);
        decode(&trailing, 12, &columns, 1, true, 0).expect_err("trailing");
    }
}
