//! Bounded static HTML table hierarchy; native graph/resource export is separate.
use crate::{EncounterId, Error, HostHandle, TableLevel, TableSort, tables::decode::Reader};
use serde::Serialize;
use std::collections::HashSet;
const MAX_CONTENT: usize = 16 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyHtmlSelection {
    session: u64,
    revision: u64,
    encounters: Vec<EncounterId>,
    last_level: TableLevel,
    tables: u32,
    rows: u32,
}
impl HierarchyHtmlSelection {
    /// Original tree order, regardless of the order of requested IDs.
    pub fn encounters(&self) -> &[EncounterId] {
        &self.encounters
    }
    pub fn last_level(&self) -> TableLevel {
        self.last_level
    }
    pub fn table_count(&self) -> u32 {
        self.tables
    }
    pub fn row_count(&self) -> u32 {
        self.rows
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HierarchyHtmlLevel {
    pub level: TableLevel,
    /// 1..32 exact registration keys, not labels or SQL names.
    pub columns: Vec<String>,
    /// Category tables preserve ACT source order and reject a sorter.
    pub sort: Option<TableSort>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyHtmlReport {
    pub selection: HierarchyHtmlSelection,
    /// Complete owned UTF-8 source. No browser, file, clipboard or external resource is accessed.
    pub content: String,
}
fn invalid(value: impl std::fmt::Display) -> Error {
    Error::InvalidReportRequest(value.to_string())
}
fn ids_valid(ids: &[EncounterId], session: u64) -> bool {
    (1..=64).contains(&ids.len())
        && ids.iter().all(|id| id.session == session && id.value != 0)
        && ids.iter().collect::<HashSet<_>>().len() == ids.len()
}
fn key(value: &str) -> bool {
    !value.is_empty() && value.len() <= 1024 && !value.contains('\0')
}
fn options_valid(levels: &[HierarchyHtmlLevel], last: TableLevel) -> bool {
    levels.len() == last as usize
        && levels
            .iter()
            .map(|level| level.level)
            .collect::<HashSet<_>>()
            .len()
            == levels.len()
        && levels.iter().all(|level| {
            level.level != TableLevel::Encounter
                && (level.level as u32) <= last as u32
                && (1..=32).contains(&level.columns.len())
                && level.columns.iter().all(|column| key(column))
                && level.columns.iter().collect::<HashSet<_>>().len() == level.columns.len()
                && level.sort.as_ref().is_none_or(|sort| {
                    level.level != TableLevel::Category
                        && key(&sort.primary)
                        && sort
                            .secondary
                            .as_ref()
                            .is_none_or(|key_value| key(key_value))
                })
        })
}
impl HostHandle {
    /// Capture 1..64 explicit session-owned encounters in original tree order, with a mandatory main table.
    /// A missing encounter returns None and retires the previous HTML capture, independently of XML.
    pub async fn capture_hierarchy_html(
        &self,
        encounters: &[EncounterId],
        last_level: TableLevel,
    ) -> Result<Option<HierarchyHtmlSelection>, Error> {
        if !ids_valid(encounters, self.session) || last_level == TableLevel::Encounter {
            return Err(invalid("invalid HTML encounter selection or depth/session"));
        }
        let ids: Vec<_> = encounters.iter().map(|id| id.value).collect();
        let output = self
            .table_report_call(
                5,
                &serde_json::json!({"html":{"encounters":ids,"lastLevel":last_level}}),
            )
            .await?;
        decode_capture(&output, encounters, last_level, self.session)
    }
    /// Invoke original display/comparison callbacks once per used field/comparison, without retry or fallback.
    /// Cooperative timeout/output rejection is an error, not a partial report or a large-workload guarantee.
    pub async fn hierarchy_html(
        &self,
        selection: &HierarchyHtmlSelection,
        levels: &[HierarchyHtmlLevel],
    ) -> Result<HierarchyHtmlReport, Error> {
        if selection.session != self.session
            || selection.revision == 0
            || !ids_valid(&selection.encounters, self.session)
        {
            return Err(invalid("HTML selection belongs to another session"));
        }
        if !options_valid(levels, selection.last_level) {
            return Err(invalid("invalid HTML levels/columns/sorting"));
        }
        let output = self
            .table_report_call(
                6,
                &serde_json::json!({"revision":selection.revision,"htmlLevels":levels}),
            )
            .await?;
        decode_report(&output, selection)
    }
}
fn identity(
    reader: &mut Reader<'_>,
    expected: &[EncounterId],
    last_level: TableLevel,
    session: u64,
) -> Result<HierarchyHtmlSelection, Error> {
    let revision = reader.u64()?;
    let actual_level = reader.level()?;
    let tables = reader.u32()?;
    let rows = reader.u32()?;
    let count = reader.u32()?;
    if revision == 0
        || actual_level != last_level
        || count as usize != expected.len()
        || !(1..=64).contains(&count)
        || !(count..=1024).contains(&tables)
        || rows > 16384
        || tables > rows + count
        || last_level == TableLevel::Combatant && tables != count
    {
        return Err(invalid("HTML hierarchy identity/depth/count mismatch"));
    }
    let mut encounters = Vec::with_capacity(count as usize);
    for _ in 0..count {
        encounters.push(EncounterId {
            session,
            value: reader.u64()?,
        });
    }
    if !ids_valid(&encounters, session)
        || encounters.iter().collect::<HashSet<_>>() != expected.iter().collect::<HashSet<_>>()
    {
        return Err(invalid("HTML hierarchy encounter set mismatch"));
    }
    Ok(HierarchyHtmlSelection {
        session,
        revision,
        encounters,
        last_level,
        tables,
        rows,
    })
}
fn decode_capture(
    bytes: &[u8],
    expected: &[EncounterId],
    last: TableLevel,
    session: u64,
) -> Result<Option<HierarchyHtmlSelection>, Error> {
    let mut reader = Reader(bytes);
    if !reader.boolean()? {
        reader.finish()?;
        return Ok(None);
    }
    let selection = identity(&mut reader, expected, last, session)?;
    reader.finish()?;
    Ok(Some(selection))
}
fn decode_report(
    bytes: &[u8],
    selection: &HierarchyHtmlSelection,
) -> Result<HierarchyHtmlReport, Error> {
    let mut reader = Reader(bytes);
    let actual = identity(
        &mut reader,
        &selection.encounters,
        selection.last_level,
        selection.session,
    )?;
    if &actual != selection {
        return Err(invalid("HTML output differs from captured selection/order"));
    }
    let content = reader.text(MAX_CONTENT)?;
    reader.finish()?;
    if content.is_empty() {
        return Err(invalid("missing hierarchy HTML source"));
    }
    Ok(HierarchyHtmlReport {
        selection: selection.clone(),
        content,
    })
}
#[cfg(test)]
mod tests;
