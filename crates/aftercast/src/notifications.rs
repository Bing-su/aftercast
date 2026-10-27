//! Bounded, transient ACT notification history. Delegates remain inside the owning worker.
use crate::{Error, HostHandle, runtime::Request};
use bytes::{BufMut, BytesMut};
use serde::{Deserialize, de::DeserializeOwned};
const HEADER: usize = 32;
const CAPACITY: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationId {
    session: u64,
    value: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationCursor {
    session: u64,
    value: u64,
}
#[derive(Debug, Clone)]
pub struct Notification {
    pub id: NotificationId,
    /// Receipt time as wall-clock DateTime ticks, without implied timezone conversion.
    pub time_ticks: u64,
    pub title: String,
    pub message: String,
    pub read: bool,
    pub can_show: bool,
    pub show_error: Option<String>,
}
#[derive(Debug, Clone)]
pub struct NotificationPage {
    pub latest_id: u64,
    pub omitted: u64,
    pub items: Vec<Notification>,
    pub next_cursor: NotificationCursor,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawItem {
    id: u64,
    time_ticks: u64,
    title: String,
    message: String,
    read: bool,
    can_show: bool,
    show_error: Option<String>,
}
impl RawItem {
    fn owned(self, session: u64) -> Result<Notification, Error> {
        if self.id == 0 || self.time_ticks > 3_155_378_975_999_999_999 {
            return Err(invalid("invalid notification identity/time"));
        }
        Ok(Notification {
            id: NotificationId {
                session,
                value: self.id,
            },
            time_ticks: self.time_ticks,
            title: self.title,
            message: self.message,
            read: self.read,
            can_show: self.can_show,
            show_error: self.show_error,
        })
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawPage {
    latest_id: u64,
    omitted: u64,
    items: Vec<RawItem>,
}
impl HostHandle {
    /// Reads at most 16 notifications after an opaque session-owned cursor.
    pub async fn notifications(
        &self,
        after: Option<NotificationCursor>,
        limit: u32,
    ) -> Result<NotificationPage, Error> {
        if after.is_some_and(|cursor| cursor.session != self.session) {
            return Err(invalid("cursor belongs to another host session"));
        }
        if !(1..=16).contains(&limit) {
            return Err(invalid("limit must be 1..16"));
        }
        let after = after.map_or(0, |cursor| cursor.value);
        let page: RawPage = self.notification_call(1, after, limit).await?;
        if page.items.len() > limit as usize {
            return Err(invalid("page exceeds requested limit"));
        }
        let mut sequence = after;
        let mut items = Vec::with_capacity(page.items.len());
        for item in page.items {
            if item.id <= sequence || item.id > page.latest_id {
                return Err(invalid("unordered notification IDs"));
            }
            sequence = item.id;
            items.push(item.owned(self.session)?);
        }
        if items.is_empty() {
            sequence = page.latest_id;
        }
        Ok(NotificationPage {
            latest_id: page.latest_id,
            omitted: page.omitted,
            items,
            next_cursor: NotificationCursor {
                session: self.session,
                value: sequence,
            },
        })
    }
    /// Explicitly invokes the retained callback on its worker owner. Success permits subsequent intentional Show calls.
    pub async fn show_notification(&self, id: NotificationId) -> Result<Notification, Error> {
        if id.session != self.session {
            return Err(invalid("notification belongs to another host session"));
        }
        let item: RawItem = self.notification_call(2, id.value, 0).await?;
        if item.id != id.value {
            return Err(invalid("Show returned a different notification"));
        }
        item.owned(self.session)
    }
    async fn notification_call<T: DeserializeOwned>(
        &self,
        command: u32,
        id: u64,
        limit: u32,
    ) -> Result<T, Error> {
        let mut packet = BytesMut::with_capacity(HEADER + CAPACITY);
        packet.put_u32_le((HEADER + CAPACITY) as u32);
        packet.put_u32_le(1);
        packet.put_u64_le(self.session);
        packet.put_u32_le(command);
        packet.put_u32_le(limit);
        packet.put_u64_le(id);
        packet.resize(HEADER + CAPACITY, 0);
        self.request(Request::Notifications, &mut packet).await?;
        let length = u32::from_le_bytes(
            packet
                .get(20..24)
                .ok_or_else(|| invalid("missing reply length"))?
                .try_into()
                .map_err(invalid)?,
        ) as usize;
        if length == 0 || length > CAPACITY {
            return Err(invalid("invalid notification reply length"));
        }
        serde_json::from_slice(
            packet
                .get(HEADER..HEADER + length)
                .ok_or_else(|| invalid("truncated reply"))?,
        )
        .map_err(invalid)
    }
}
fn invalid(error: impl std::fmt::Display) -> Error {
    Error::InvalidNotificationRequest(error.to_string())
}
