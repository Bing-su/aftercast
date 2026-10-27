//! Owned current-table reports. SQL XML and escaped static HTML stay separate from archive XML.
use crate::{
    EncounterId, Error, HostHandle, TableColumn, TableRow, TableScope, TableSort, runtime::Request,
    tables::decode::Reader,
};
use bytes::{BufMut, BytesMut};
use serde::Serialize;
use std::collections::HashSet;
const HEADER: usize = 32;
const MAX_CONTENT: usize = 16 * 1024 * 1024;
const CATALOG_CAPACITY: usize = 56 * 1024 * 1024;
const REPORT_CAPACITY: usize = MAX_CONTENT + 20;

/// One catalog capture in one originating session; newer captures retire earlier selections.
/// Row ordinals are snapshot-local, never action IDs or indices in a sorted report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableReportSelection {
    session: u64,
    revision: u64,
    scope: TableScope,
    rows: u32,
}
impl TableReportSelection {
    pub fn scope(&self) -> &TableScope {
        &self.scope
    }
    pub fn row_count(&self) -> u32 {
        self.rows
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct TableReportCatalog {
    pub selection: TableReportSelection,
    pub columns: Vec<TableColumn>,
    /// Descriptors have empty cells; discovery never invokes value/color callbacks.
    pub rows: Vec<TableRow>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(into = "u32")]
#[repr(u32)]
pub enum TableReportFormat {
    TableXml,
    StaticHtml,
}
impl From<TableReportFormat> for u32 {
    fn from(value: TableReportFormat) -> Self {
        value as Self
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableReportOptions {
    pub format: TableReportFormat,
    pub name: String,
    /// XML exports the full scope for zero/one selection, filters for two or more.
    /// Static HTML rejects a nonempty selection.
    pub selected_rows: Vec<u32>,
    /// XML requires None (all SQL columns); HTML requires 1..32 registration keys.
    pub columns: Option<Vec<String>>,
    pub sort: Option<TableSort>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableReport {
    pub selection: TableReportSelection,
    pub format: TableReportFormat,
    pub rows: u32,
    /// Owned UTF-8; rendering performs no file, clipboard, browser or network operation.
    pub content: String,
}
fn invalid(error: impl std::fmt::Display) -> Error {
    Error::InvalidReportRequest(error.to_string())
}
fn text(value: &str, nonempty: bool) -> bool {
    value.len() <= 1024 && !value.contains('\0') && (!nonempty || !value.is_empty())
}
fn scope_request(scope: &TableScope, session: u64) -> Result<serde_json::Value, Error> {
    let (id, actor, category, attack) = scope.parts();
    let id = id.ok_or_else(|| {
        invalid("encounter-list reports require a separate hierarchical exporter")
    })?;
    if id.session != session
        || id.value == 0
        || [actor, category, attack]
            .into_iter()
            .flatten()
            .any(|key| !text(key, false))
    {
        return Err(invalid("invalid table report scope or session"));
    }
    Ok(
        serde_json::json!({"level":scope.level(),"encounterId":id.value,"combatant":actor,"category":category,"attack":attack}),
    )
}
impl TableReportOptions {
    fn validate(&self, selection: &TableReportSelection) -> Result<(), Error> {
        if !text(&self.name, false)
            || self.selected_rows.len() > 16384
            || self
                .selected_rows
                .iter()
                .any(|index| *index >= selection.rows)
            || self.selected_rows.iter().collect::<HashSet<_>>().len() != self.selected_rows.len()
        {
            return Err(invalid("invalid report caption or selected snapshot rows"));
        }
        match self.format {
            TableReportFormat::TableXml if self.columns.is_some() => {
                return Err(invalid("table XML uses all SQL columns"));
            }
            TableReportFormat::StaticHtml => {
                let columns = self
                    .columns
                    .as_ref()
                    .ok_or_else(|| invalid("HTML needs explicit columns"))?;
                if !self.selected_rows.is_empty()
                    || !(1..=32).contains(&columns.len())
                    || columns.iter().any(|key| !text(key, true))
                    || columns.iter().collect::<HashSet<_>>().len() != columns.len()
                {
                    return Err(invalid("invalid HTML scope or columns"));
                }
            }
            TableReportFormat::TableXml => {}
        }
        if let Some(sort) = &self.sort
            && (!text(&sort.primary, true)
                || sort.secondary.as_ref().is_some_and(|key| !text(key, true)))
        {
            return Err(invalid("invalid report sort key"));
        }
        Ok(())
    }
}
impl HostHandle {
    /// Capture original row references without invoking formatters. Even an absent scope retires older captures.
    pub async fn table_report_catalog(
        &self,
        scope: &TableScope,
    ) -> Result<Option<TableReportCatalog>, Error> {
        let request = serde_json::json!({"scope":scope_request(scope, self.session)?});
        let output = self.table_report_call(1, &request).await?;
        decode_catalog(&output, self.session, scope)
    }
    /// Render exactly once, with no capacity/revision retry. Requires the latest capture of this Host.
    pub async fn table_report(
        &self,
        selection: &TableReportSelection,
        options: &TableReportOptions,
    ) -> Result<TableReport, Error> {
        if selection.session != self.session || selection.revision == 0 {
            return Err(invalid("table report selection belongs to another session"));
        }
        options.validate(selection)?;
        let request = serde_json::json!({"revision":selection.revision,"options":options});
        let output = self.table_report_call(2, &request).await?;
        decode_report(&output, selection, options)
    }
    async fn table_report_call(
        &self,
        command: u32,
        request: &serde_json::Value,
    ) -> Result<Vec<u8>, Error> {
        let input = serde_json::to_vec(request).map_err(invalid)?;
        if !(2..=256 * 1024).contains(&input.len()) {
            return Err(invalid("table report input exceeds capacity"));
        }
        let capacity = capacity(command);
        let mut packet = BytesMut::with_capacity(HEADER + capacity);
        packet.put_u32_le((HEADER + capacity) as u32);
        packet.put_u32_le(1);
        packet.put_u64_le(self.session);
        packet.put_u32_le(command);
        packet.put_u32_le(input.len() as u32);
        packet.put_u64_le(0);
        packet.extend_from_slice(&input);
        packet.resize(HEADER + capacity, 0);
        self.request(Request::TableReports, &mut packet).await?;
        decode_packet(&packet, self.session, command)
    }
}
fn capacity(command: u32) -> usize {
    if command == 1 {
        CATALOG_CAPACITY
    } else {
        REPORT_CAPACITY
    }
}
fn decode_packet(packet: &[u8], session: u64, command: u32) -> Result<Vec<u8>, Error> {
    let capacity = capacity(command);
    if packet.len() != HEADER + capacity
        || packet.get(..4) != Some(((HEADER + capacity) as u32).to_le_bytes().as_slice())
        || packet.get(4..8) != Some(1_u32.to_le_bytes().as_slice())
        || packet.get(8..16) != Some(session.to_le_bytes().as_slice())
        || packet.get(16..20) != Some(command.to_le_bytes().as_slice())
        || packet.get(28..32) != Some([0; 4].as_slice())
    {
        return Err(invalid("table report response envelope mismatch"));
    }
    let count = u32::from_le_bytes(
        packet
            .get(24..28)
            .ok_or_else(|| invalid("missing report length"))?
            .try_into()
            .map_err(invalid)?,
    ) as usize;
    if count == 0 || count > capacity {
        return Err(invalid("invalid table report response length"));
    }
    Ok(packet
        .get(HEADER..HEADER + count)
        .ok_or_else(|| invalid("truncated report body"))?
        .to_vec())
}
fn decode_catalog(
    bytes: &[u8],
    session: u64,
    scope: &TableScope,
) -> Result<Option<TableReportCatalog>, Error> {
    let mut reader = Reader(bytes);
    if !reader.boolean()? {
        reader.finish()?;
        return Ok(None);
    }
    let revision = reader.u64()?;
    let level = reader.level()?;
    let id = reader.u64()?;
    let actor = reader.optional_text(1024)?;
    let category = reader.optional_text(1024)?;
    let attack = reader.optional_text(1024)?;
    let (expected, a, c, t) = scope.parts();
    if revision == 0
        || level != scope.level()
        || expected.is_none_or(|expected| expected.value != id)
        || actor.as_deref() != a
        || category.as_deref() != c
        || attack.as_deref() != t
    {
        return Err(invalid("table report catalog scope mismatch"));
    }
    let columns = reader.columns(256)?;
    let count = reader.u32()?;
    if count > 16384 {
        return Err(invalid("too many captured report rows"));
    }
    let mut rows = Vec::with_capacity(count as usize);
    for index in 0..count {
        let ordinal = reader.u32()?;
        let row_id = reader.u64()?;
        let combatant = reader.optional_text(1024)?;
        let category = reader.optional_text(1024)?;
        let attack = reader.optional_text(1024)?;
        let time_sorter = reader.optional_int()?;
        let depth = level as u32;
        if ordinal != index
            || row_id != id
            || combatant.is_none()
            || category.is_some() != (depth >= 2)
            || attack.is_some() != (depth >= 3)
            || time_sorter.is_some() != (depth >= 4)
            || c.is_some_and(|value| category.as_deref() != Some(value))
            || t.is_some_and(|value| attack.as_deref() != Some(value))
        {
            return Err(invalid(
                "report row descriptor does not match captured scope",
            ));
        }
        rows.push(TableRow {
            index,
            encounter: EncounterId { session, value: id },
            combatant,
            category,
            attack,
            time_sorter,
            cells: Vec::new(),
        });
    }
    reader.finish()?;
    Ok(Some(TableReportCatalog {
        selection: TableReportSelection {
            session,
            revision,
            scope: scope.clone(),
            rows: count,
        },
        columns,
        rows,
    }))
}
fn decode_report(
    bytes: &[u8],
    selection: &TableReportSelection,
    options: &TableReportOptions,
) -> Result<TableReport, Error> {
    let mut reader = Reader(bytes);
    let revision = reader.u64()?;
    let format = reader.u32()?;
    let rows = reader.u32()?;
    let content = reader.text(MAX_CONTENT)?;
    reader.finish()?;
    let expected = if options.selected_rows.len() > 1 {
        options.selected_rows.len() as u32
    } else {
        selection.rows
    };
    if revision != selection.revision || format != u32::from(options.format) || rows != expected {
        return Err(invalid("report output selection/format/count mismatch"));
    }
    Ok(TableReport {
        selection: selection.clone(),
        format: options.format,
        rows,
        content,
    })
}
#[cfg(test)]
mod tests;
