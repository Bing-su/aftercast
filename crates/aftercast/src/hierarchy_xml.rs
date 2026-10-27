//! ACT task-table XML depths, distinct from archive XML and single-current-table reports.
use crate::{EncounterId, Error, HostHandle, tables::decode::Reader};
use serde::Serialize;
const MAX_CONTENT: usize = 16 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(into = "u32")]
#[repr(u32)]
pub enum HierarchyXmlDepth {
    MainTable = 1,
    /// Adds category and attack-summary tables (two row levels).
    DamageTypeTables = 2,
    AllTables = 3,
}
impl From<HierarchyXmlDepth> for u32 {
    fn from(value: HierarchyXmlDepth) -> Self {
        value as Self
    }
}
/// One original topology captured by one worker session. New captures retire older selections.
/// Counts are snapshot bounds; they do not imply successful rendering within the cooperative budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyXmlSelection {
    revision: u64,
    encounter: EncounterId,
    depth: HierarchyXmlDepth,
    tables: u32,
    rows: u32,
}
impl HierarchyXmlSelection {
    pub fn encounter(&self) -> EncounterId {
        self.encounter
    }
    pub fn depth(&self) -> HierarchyXmlDepth {
        self.depth
    }
    pub fn table_count(&self) -> u32 {
        self.tables
    }
    pub fn row_count(&self) -> u32 {
        self.rows
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyXmlReport {
    pub selection: HierarchyXmlSelection,
    /// Complete owned UTF-8 source; no file/clipboard/browser operation occurs.
    pub content: String,
}
fn invalid(value: impl std::fmt::Display) -> Error {
    Error::InvalidReportRequest(value.to_string())
}
impl HostHandle {
    /// Capture the exact selected encounter and ACT depth. Caption/hash methods may run on the owner.
    /// A missing encounter returns None and retires the previous hierarchy capture.
    pub async fn capture_hierarchy_xml(
        &self,
        encounter: EncounterId,
        depth: HierarchyXmlDepth,
    ) -> Result<Option<HierarchyXmlSelection>, Error> {
        if encounter.session != self.session || encounter.value == 0 {
            return Err(invalid("hierarchy encounter belongs to another session"));
        }
        let output = self
            .table_report_call(
                3,
                &serde_json::json!({"hierarchy":{"encounterId":encounter.value,"depth":depth}}),
            )
            .await?;
        decode_capture(&output, encounter, depth)
    }
    /// Render once. There is no automatic callback retry, capacity probe or timeout fallback.
    /// Even admitted moderate-size data can exceed the explicit per-node/whole-report time budgets.
    pub async fn hierarchy_xml(
        &self,
        selection: &HierarchyXmlSelection,
    ) -> Result<HierarchyXmlReport, Error> {
        if selection.encounter.session != self.session
            || selection.encounter.value == 0
            || selection.revision == 0
        {
            return Err(invalid("hierarchy selection belongs to another session"));
        }
        let output = self
            .table_report_call(4, &serde_json::json!({"revision":selection.revision}))
            .await?;
        decode_report(&output, selection)
    }
}
fn identity(
    reader: &mut Reader<'_>,
    encounter: EncounterId,
    depth: HierarchyXmlDepth,
) -> Result<HierarchyXmlSelection, Error> {
    let revision = reader.u64()?;
    let id = reader.u64()?;
    let actual_depth = reader.u32()?;
    let tables = reader.u32()?;
    let rows = reader.u32()?;
    if revision == 0
        || id != encounter.value
        || actual_depth != u32::from(depth)
        || !(1..=1024).contains(&tables)
        || rows > 16384
        || tables > rows + 1
        || depth == HierarchyXmlDepth::MainTable && tables != 1
    {
        return Err(invalid("hierarchy response scope/depth/count mismatch"));
    }
    Ok(HierarchyXmlSelection {
        revision,
        encounter,
        depth,
        tables,
        rows,
    })
}
fn decode_capture(
    bytes: &[u8],
    encounter: EncounterId,
    depth: HierarchyXmlDepth,
) -> Result<Option<HierarchyXmlSelection>, Error> {
    let mut reader = Reader(bytes);
    if !reader.boolean()? {
        reader.finish()?;
        return Ok(None);
    }
    let selection = identity(&mut reader, encounter, depth)?;
    reader.finish()?;
    Ok(Some(selection))
}
fn decode_report(
    bytes: &[u8],
    selection: &HierarchyXmlSelection,
) -> Result<HierarchyXmlReport, Error> {
    let mut reader = Reader(bytes);
    let actual = identity(&mut reader, selection.encounter, selection.depth)?;
    if &actual != selection {
        return Err(invalid("hierarchy report differs from captured selection"));
    }
    let content = reader.text(MAX_CONTENT)?;
    reader.finish()?;
    if content.is_empty() {
        return Err(invalid("missing hierarchy XML source"));
    }
    Ok(HierarchyXmlReport {
        selection: selection.clone(),
        content,
    })
}
#[cfg(test)]
mod tests;
