//! Snapshot-bound checkpoint discovery; distinct from reparsing raw source logs.
use crate::{Error, HostHandle, LogTimeKind, LogTimestamp, runtime::Request};
use bytes::{BufMut, BytesMut};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
const HEADER: usize = 32;
const CAPACITY: usize = 4 * 1024 * 1024;
const MAX_TICKS: u64 = 3_155_378_975_999_999_999;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(into = "u32")]
#[repr(u32)]
pub enum HistorySearchScope {
    #[default]
    Any,
    Zone,
    Title,
    Character,
}
impl From<HistorySearchScope> for u32 {
    fn from(value: HistorySearchScope) -> Self {
        value as Self
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQuery {
    pub text: String,
    pub scope: HistorySearchScope,
    /// Start inclusive, end exclusive, comparing source wall-clock DateTime ticks.
    pub start_ticks: Option<u64>,
    pub end_ticks: Option<u64>,
    pub offset: u32,
    pub limit: u32,
}
impl Default for HistoryQuery {
    fn default() -> Self {
        Self {
            text: String::new(),
            scope: HistorySearchScope::Any,
            start_ticks: None,
            end_ticks: None,
            offset: 0,
            limit: 16,
        }
    }
}
impl HistoryQuery {
    pub fn first_page() -> Self {
        Self::default()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HistoryRevision {
    pub(crate) session: u64,
    pub(crate) digest: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HistoryId {
    pub(crate) session: u64,
    pub(crate) digest: String,
    pub(crate) file: String,
    pub(crate) start_ticks: u64,
    pub(crate) end_ticks: u64,
}
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub id: HistoryId,
    pub title: String,
    pub zone: String,
    pub character: String,
    pub folder_hint: String,
    pub start: LogTimestamp,
    pub end: LogTimestamp,
    pub partial: bool,
    /// Existence hint only; integrity/readability and raw source logs are not verified by discovery.
    pub payload_present: bool,
    pub already_restored: bool,
}
#[derive(Debug, Clone)]
pub struct HistoryPage {
    pub revision: HistoryRevision,
    pub index_present: bool,
    pub indexed_count: u32,
    pub total: u32,
    pub items: Vec<HistoryEntry>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawEntry {
    id: String,
    title: String,
    zone: String,
    character: String,
    folder_hint: String,
    start_ticks: u64,
    start_kind: u32,
    end_ticks: u64,
    end_kind: u32,
    partial: bool,
    payload_present: bool,
    already_restored: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawPage {
    index_revision: String,
    index_present: bool,
    indexed_count: u32,
    total: u32,
    items: Vec<RawEntry>,
}
impl HostHandle {
    pub async fn query_history(&self, query: &HistoryQuery) -> Result<HistoryPage, Error> {
        if !(1..=16).contains(&query.limit)
            || query.text.contains('\0')
            || query.text.encode_utf16().count() > 4096
            || query.start_ticks.is_some_and(|ticks| ticks > MAX_TICKS)
            || query.end_ticks.is_some_and(|ticks| ticks > MAX_TICKS + 1)
            || matches!((query.start_ticks,query.end_ticks),(Some(start),Some(end)) if start >= end)
        {
            return Err(invalid("invalid query bounds"));
        }
        let page: RawPage = self
            .history_call(1, &serde_json::json!({"query":query}))
            .await?;
        if !valid_digest(&page.index_revision)
            || page.total > page.indexed_count
            || page.items.len() > query.limit as usize
            || page.items.len() > page.total.saturating_sub(query.offset) as usize
            || !page.index_present && page.indexed_count != 0
        {
            return Err(invalid("invalid history page"));
        }
        let revision = HistoryRevision {
            session: self.session,
            digest: page.index_revision,
        };
        let mut seen = std::collections::HashSet::new();
        let mut items = Vec::with_capacity(page.items.len());
        for item in page.items {
            if !valid_file(&item.id)
                || !seen.insert(item.id.clone())
                || item.end_ticks < item.start_ticks
            {
                return Err(invalid("invalid/duplicate archive identity or time range"));
            }
            items.push(HistoryEntry {
                id: HistoryId {
                    session: self.session,
                    digest: revision.digest.clone(),
                    file: item.id,
                    start_ticks: item.start_ticks,
                    end_ticks: item.end_ticks,
                },
                title: item.title,
                zone: item.zone,
                character: item.character,
                folder_hint: item.folder_hint,
                start: timestamp(item.start_ticks, item.start_kind)?,
                end: timestamp(item.end_ticks, item.end_kind)?,
                partial: item.partial,
                payload_present: item.payload_present,
                already_restored: item.already_restored,
            });
        }
        Ok(HistoryPage {
            revision,
            index_present: page.index_present,
            indexed_count: page.indexed_count,
            total: page.total,
            items,
        })
    }
    /// Accepted restoration is completed during shutdown; dropping this future does not cancel it.
    pub async fn restore_selected_history(
        &self,
        revision: &HistoryRevision,
        ids: &[HistoryId],
    ) -> Result<u32, Error> {
        if revision.session != self.session
            || !(1..=64).contains(&ids.len())
            || ids
                .iter()
                .any(|id| id.session != self.session || id.digest != revision.digest)
        {
            return Err(invalid(
                "selection belongs to another snapshot/session or exceeds 1..64",
            ));
        }
        let mut files = std::collections::HashSet::new();
        if ids.iter().any(|id| !files.insert(&id.file)) {
            return Err(invalid("duplicate selected archive"));
        }
        self.history_call(2,&serde_json::json!({"selection":{"indexRevision":revision.digest,"ids":ids.iter().map(|id| &id.file).collect::<Vec<_>>()}})).await
    }
    async fn history_call<T: DeserializeOwned>(
        &self,
        command: u32,
        value: &impl Serialize,
    ) -> Result<T, Error> {
        let input = serde_json::to_vec(value).map_err(invalid)?;
        if input.len() > 128 * 1024 {
            return Err(invalid("history input exceeds 128 KiB"));
        }
        let mut packet = poll_packet(self.session);
        packet
            .get_mut(16..20)
            .ok_or_else(|| invalid("missing command"))?
            .copy_from_slice(&command.to_le_bytes());
        packet
            .get_mut(20..24)
            .ok_or_else(|| invalid("missing input length"))?
            .copy_from_slice(&(input.len() as u32).to_le_bytes());
        packet
            .get_mut(HEADER..HEADER + input.len())
            .ok_or_else(|| invalid("input exceeds packet"))?
            .copy_from_slice(&input);
        self.request(Request::History, &mut packet).await?;
        let length = u32::from_le_bytes(
            packet
                .get(24..28)
                .ok_or_else(|| invalid("missing output length"))?
                .try_into()
                .map_err(invalid)?,
        ) as usize;
        if length == 0 || length > CAPACITY {
            return Err(invalid("invalid output length"));
        }
        serde_json::from_slice(
            packet
                .get(HEADER..HEADER + length)
                .ok_or_else(|| invalid("truncated output"))?,
        )
        .map_err(invalid)
    }
}
pub(crate) fn poll_packet(session: u64) -> BytesMut {
    let mut packet = BytesMut::with_capacity(HEADER + CAPACITY);
    packet.put_u32_le((HEADER + CAPACITY) as u32);
    packet.put_u32_le(1);
    packet.put_u64_le(session);
    packet.resize(HEADER + CAPACITY, 0);
    packet
}
pub(crate) fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn valid_file(value: &str) -> bool {
    value.len() == 68
        && value.ends_with(".act")
        && value.bytes().take(64).all(|byte| byte.is_ascii_hexdigit())
}
fn timestamp(ticks: u64, kind: u32) -> Result<LogTimestamp, Error> {
    if ticks > MAX_TICKS {
        return Err(invalid("timestamp out of range"));
    }
    let kind = match kind {
        0 => LogTimeKind::Unspecified,
        1 => LogTimeKind::Utc,
        2 => LogTimeKind::Local,
        _ => return Err(invalid("invalid timestamp kind")),
    };
    Ok(LogTimestamp {
        ticks,
        utc_ticks: (kind == LogTimeKind::Utc).then_some(ticks),
        kind,
    })
}
fn invalid(error: impl std::fmt::Display) -> Error {
    Error::InvalidHistoryRequest(error.to_string())
}
