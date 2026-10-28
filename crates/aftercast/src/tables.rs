//! Plugin-defined tables. Formatting/comparison delegates stay inside their original worker.
use crate::{EncounterId, Error, HostHandle, ZoneId, runtime::Request};
use bytes::{BufMut, BytesMut};
use serde::Serialize;
pub(crate) mod decode;
const HEADER: usize = 32;
const CATALOG_CAPACITY: usize = 4 * 1024 * 1024;
const ROW_CAPACITY: usize = 16 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(into = "u32")]
#[repr(u32)]
pub enum TableLevel {
    Encounter,
    Combatant,
    Category,
    Attack,
    Action,
}
impl From<TableLevel> for u32 {
    fn from(value: TableLevel) -> Self {
        value as Self
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableRevision {
    session: u64,
    level: TableLevel,
    value: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumn {
    pub key: String,
    pub label: String,
    pub default_visible: bool,
    pub sql_data_type: String,
    pub sql_data_name: String,
    pub can_read: bool,
    pub can_sort: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCategory {
    pub key: String,
    pub label: String,
    pub outgoing: bool,
    pub ally_value: i32,
    pub color_argb: i32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct TableCatalog {
    pub level: TableLevel,
    pub revision: TableRevision,
    pub columns: Vec<TableColumn>,
    pub categories: Vec<TableCategory>,
}
/// Scope keys come from the originating graph/table rows. Encounter IDs cannot cross sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableScope {
    Encounters,
    /// Zone identified by a retained encounter in the current session.
    Zone(EncounterId),
    /// ZoneList object, including an empty zone, owned by this session.
    ZoneId(ZoneId),
    Combatants(EncounterId),
    Categories {
        encounter: EncounterId,
        combatant: String,
    },
    Attacks {
        encounter: EncounterId,
        combatant: String,
        category: String,
    },
    Actions {
        encounter: EncounterId,
        combatant: String,
        category: String,
        attack: String,
    },
}
impl TableScope {
    pub fn level(&self) -> TableLevel {
        match self {
            Self::Encounters | Self::Zone(_) | Self::ZoneId(_) => TableLevel::Encounter,
            Self::Combatants(_) => TableLevel::Combatant,
            Self::Categories { .. } => TableLevel::Category,
            Self::Attacks { .. } => TableLevel::Attack,
            Self::Actions { .. } => TableLevel::Action,
        }
    }
    pub(crate) fn parts(
        &self,
    ) -> (
        Option<EncounterId>,
        Option<&str>,
        Option<&str>,
        Option<&str>,
    ) {
        match self {
            Self::Encounters | Self::Zone(_) | Self::ZoneId(_) => (None, None, None, None),
            Self::Combatants(id) => (Some(*id), None, None, None),
            Self::Categories {
                encounter,
                combatant,
            } => (Some(*encounter), Some(combatant), None, None),
            Self::Attacks {
                encounter,
                combatant,
                category,
            } => (Some(*encounter), Some(combatant), Some(category), None),
            Self::Actions {
                encounter,
                combatant,
                category,
                attack,
            } => (
                Some(*encounter),
                Some(combatant),
                Some(category),
                Some(attack),
            ),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSort {
    pub primary: String,
    pub secondary: Option<String>,
    pub descending: bool,
}
#[derive(Debug, Clone)]
pub struct TableQuery {
    pub scope: TableScope,
    pub revision: TableRevision,
    pub columns: Vec<String>,
    pub offset: u32,
    pub limit: u32,
    pub sort: Option<TableSort>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableCellError {
    Column,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableSortError {
    Callback,
    InconsistentComparison,
    ComparisonBudget,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCell {
    pub text: Option<String>,
    pub foreground_argb: Option<i32>,
    pub background_argb: Option<i32>,
    pub error: Option<TableCellError>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    /// Original ordinal in this scope snapshot; never a durable action ID or source-log index.
    pub index: u32,
    pub encounter: EncounterId,
    pub combatant: Option<String>,
    pub category: Option<String>,
    pub attack: Option<String>,
    pub time_sorter: Option<i32>,
    pub cells: Vec<TableCell>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct TablePage {
    pub revision: TableRevision,
    pub total: u32,
    pub columns: Vec<TableColumn>,
    pub rows: Vec<TableRow>,
    pub sort_error: Option<TableSortError>,
}
fn valid_text(text: &str, maximum: usize, nonempty: bool) -> bool {
    text.len() <= maximum && !text.contains('\0') && (!nonempty || !text.is_empty())
}
impl TableQuery {
    fn request(&self, session: u64) -> Result<serde_json::Value, Error> {
        let level = self.scope.level();
        let (id, actor, category, attack) = self.scope.parts();
        let zone_anchor = if let TableScope::Zone(anchor) = self.scope {
            Some(anchor)
        } else {
            None
        };
        let zone_id = if let TableScope::ZoneId(zone) = self.scope {
            Some(zone)
        } else {
            None
        };
        if self.revision.session != session
            || self.revision.level != level
            || self.revision.value == 0
            || id.is_some_and(|id| id.session != session)
            || zone_anchor.is_some_and(|id| id.session != session)
            || zone_id.is_some_and(|id| id.session != session)
        {
            return Err(invalid(
                "scope/revision belongs to another host session or table level",
            ));
        }
        if !(1..=256).contains(&self.limit)
            || self.offset > i32::MAX as u32
            || !(1..=32).contains(&self.columns.len())
            || self.columns.iter().any(|key| !valid_text(key, 1024, true))
            || self
                .columns
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != self.columns.len()
            || [actor, category, attack]
                .into_iter()
                .flatten()
                .any(|text| !valid_text(text, 1024, false))
        {
            return Err(invalid("invalid table page, selected keys or scope text"));
        }
        if let Some(sort) = &self.sort
            && (!valid_text(&sort.primary, 1024, true)
                || sort
                    .secondary
                    .as_ref()
                    .is_some_and(|key| !valid_text(key, 1024, true)))
        {
            return Err(invalid("invalid table sort key"));
        }
        Ok(serde_json::json!({"query":{
            "scope":{"level":level,"encounterId":id.or(zone_anchor).map_or(0, |id| id.value), "zoneAnchor":zone_anchor.is_some(), "zoneId":zone_id.map_or(0, |id| id.value), "combatant":actor,"category":category,"attack":attack},
            "revision":self.revision.value,"columns":self.columns,"offset":self.offset,"limit":self.limit,"sort":self.sort
        }}))
    }
}
impl HostHandle {
    /// Catalog discovery does not invoke plugin formatters. Busy while another owned operation is active.
    pub async fn table_columns(&self, level: TableLevel) -> Result<TableCatalog, Error> {
        let output = self
            .table_call(1, &serde_json::json!({"level":level}))
            .await?;
        decode::catalog(&output, self.session, level)
    }
    /// Reads one owned page after bounded whole-scope sorting. No callback capacity retry is performed.
    /// Cells/sort errors remain explicit. Plugin code is not preempted here; shutdown uses the existing process boundary.
    pub async fn table_rows(&self, query: &TableQuery) -> Result<Option<TablePage>, Error> {
        let request = query.request(self.session)?;
        let output = self.table_call(2, &request).await?;
        decode::page(&output, self.session, query)
    }
    /// Main-table ACT comparison, including default callbacks and reverse tie order.
    pub async fn main_table_rows(
        &self,
        query: &TableQuery,
        descending: bool,
        publish_sort: bool,
    ) -> Result<Option<TablePage>, Error> {
        if !matches!(
            query.scope.level(),
            TableLevel::Combatant | TableLevel::Attack | TableLevel::Action
        ) {
            return Err(invalid(
                "main sorting requires combatant, attack or action scope",
            ));
        }
        if publish_sort && query.sort.is_none() {
            return Err(invalid("publishing requires an explicit sort"));
        }
        let mut request = query.request(self.session)?;
        request
            .get_mut("query")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| invalid("missing table query"))?
            .insert("mainDescending".into(), descending.into());
        if publish_sort {
            request
                .get_mut("query")
                .and_then(serde_json::Value::as_object_mut)
                .ok_or_else(|| invalid("missing table query"))?
                .insert("publishMainSort".into(), true.into());
        }
        let output = self.table_call(2, &request).await?;
        decode::main_page(&output, self.session, query)
    }
    /// Read an ACT lookup graph owned by the captured action snapshot.
    pub async fn lookup_table_rows(
        &self,
        snapshot: crate::ActionSnapshot,
        query: &TableQuery,
        peer: Option<&str>,
        skill: Option<&str>,
    ) -> Result<TablePage, Error> {
        if snapshot.session != self.session
            || peer.is_none() && skill.is_some()
            || peer
                .into_iter()
                .chain(skill)
                .any(|text| !valid_text(text, 1024, false))
        {
            return Err(invalid("invalid lookup ownership or selection"));
        }
        let level = if peer.is_none() {
            TableLevel::Category
        } else if skill.is_none() {
            TableLevel::Attack
        } else {
            TableLevel::Action
        };
        if query.scope.level() != level {
            return Err(invalid("lookup scope level mismatch"));
        }
        let mut request = query.request(self.session)?;
        let fields = request
            .as_object_mut()
            .ok_or_else(|| invalid("invalid lookup request"))?;
        fields.insert("lookupRevision".into(), snapshot.revision.into());
        fields.insert("peer".into(), serde_json::json!(peer));
        fields.insert("skill".into(), serde_json::json!(skill));
        let output = self.table_call(3, &request).await?;
        decode::lookup_page(&output, self.session, query, peer, skill)
    }
    async fn table_call(
        &self,
        command: u32,
        request: &serde_json::Value,
    ) -> Result<Vec<u8>, Error> {
        let input = serde_json::to_vec(request).map_err(invalid)?;
        if !(2..=256 * 1024).contains(&input.len()) {
            return Err(invalid("table request exceeds bounded input"));
        }
        let capacity = if command == 1 {
            CATALOG_CAPACITY
        } else {
            ROW_CAPACITY
        };
        let mut packet = BytesMut::with_capacity(HEADER + capacity);
        packet.put_u32_le((HEADER + capacity) as u32);
        packet.put_u32_le(1);
        packet.put_u64_le(self.session);
        packet.put_u32_le(command);
        packet.put_u32_le(input.len() as u32);
        packet.put_u64_le(0);
        packet.extend_from_slice(&input);
        packet.resize(HEADER + capacity, 0);
        self.request(Request::Tables, &mut packet).await?;
        let length = u32::from_le_bytes(
            packet
                .get(24..28)
                .ok_or_else(|| invalid("missing table length"))?
                .try_into()
                .map_err(invalid)?,
        ) as usize;
        if length == 0 || length > capacity {
            return Err(invalid("invalid table response length"));
        }
        Ok(packet
            .get(HEADER..HEADER + length)
            .ok_or_else(|| invalid("truncated table reply"))?
            .to_vec())
    }
}
fn invalid(error: impl std::fmt::Display) -> Error {
    Error::InvalidTableRequest(error.to_string())
}
#[cfg(test)]
mod tests;
