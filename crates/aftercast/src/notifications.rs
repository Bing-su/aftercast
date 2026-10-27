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
    pub status: NotificationStatus,
}
/// Whole-history counts at one model snapshot, independent of the visible page.
#[derive(Debug, Clone)]
pub struct NotificationStatus {
    pub snapshot_cursor: NotificationCursor,
    pub omitted: u64,
    pub retained: u32,
    pub unread: u32,
}
#[derive(Debug, Clone)]
pub struct NotificationChange {
    pub changed: u32,
    /// First page and counts captured atomically with the mutation.
    pub page: NotificationPage,
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
    retained: u32,
    unread: u32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawStatus {
    latest_id: u64,
    omitted: u64,
    retained: u32,
    unread: u32,
}
impl RawStatus {
    fn owned(self, session: u64) -> Result<NotificationStatus, Error> {
        if self.retained > 256 || self.unread > self.retained {
            return Err(invalid("invalid notification history counts"));
        }
        Ok(NotificationStatus {
            snapshot_cursor: NotificationCursor {
                session,
                value: self.latest_id,
            },
            omitted: self.omitted,
            retained: self.retained,
            unread: self.unread,
        })
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawChange {
    changed: u32,
    page: RawPage,
}
impl RawPage {
    fn owned(self, session: u64, after: u64, limit: u32) -> Result<NotificationPage, Error> {
        let status = RawStatus {
            latest_id: self.latest_id,
            omitted: self.omitted,
            retained: self.retained,
            unread: self.unread,
        }
        .owned(session)?;
        if after > self.latest_id
            || self.items.len() > limit as usize
            || self.items.len() > self.retained as usize
            || self.items.iter().filter(|item| !item.read).count() > self.unread as usize
        {
            return Err(invalid("notification page/count boundary mismatch"));
        }
        let mut sequence = after;
        let mut items = Vec::with_capacity(self.items.len());
        for item in self.items {
            if item.id <= sequence || item.id > self.latest_id {
                return Err(invalid("unordered notification IDs"));
            }
            sequence = item.id;
            items.push(item.owned(session)?);
        }
        if items.is_empty() {
            sequence = self.latest_id;
        }
        Ok(NotificationPage {
            latest_id: self.latest_id,
            omitted: self.omitted,
            items,
            next_cursor: NotificationCursor {
                session,
                value: sequence,
            },
            status,
        })
    }
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
        page.owned(self.session, after, limit)
    }
    /// Whole retained-history counts, without invoking a plugin callback.
    pub async fn notification_status(&self) -> Result<NotificationStatus, Error> {
        let status: RawStatus = self.notification_call(5, 0, 0).await?;
        status.owned(self.session)
    }
    /// Marks all retained entries through the accepted snapshot read, without invoking Show.
    /// Newer entries remain untouched. Repeated requests are idempotent.
    pub async fn mark_notifications_read(
        &self,
        through: NotificationCursor,
    ) -> Result<NotificationChange, Error> {
        self.change_notifications(through, 3).await
    }
    /// Releases read entries/delegates through the accepted snapshot. Cleared IDs expire permanently.
    pub async fn clear_read_notifications(
        &self,
        through: NotificationCursor,
    ) -> Result<NotificationChange, Error> {
        self.change_notifications(through, 4).await
    }
    async fn change_notifications(
        &self,
        through: NotificationCursor,
        command: u32,
    ) -> Result<NotificationChange, Error> {
        if through.session != self.session {
            return Err(invalid("bulk boundary belongs to another host session"));
        }
        let change: RawChange = self.notification_call(command, through.value, 0).await?;
        if change.changed > 256 || change.page.latest_id < through.value {
            return Err(invalid("invalid notification mutation result"));
        }
        Ok(NotificationChange {
            changed: change.changed,
            page: change.page.owned(self.session, 0, 16)?,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_counts_and_cursors_reject_inconsistent_worker_results() {
        let page = || RawPage {
            latest_id: 3,
            omitted: 0,
            retained: 0,
            unread: 0,
            items: vec![],
        };
        assert!(page().owned(1, 4, 16).is_err(), "cursor beyond snapshot");
        let mut invalid = page();
        invalid.unread = 1;
        invalid.owned(1, 0, 16).expect_err("invalid history counts");
        let mut invalid = page();
        invalid.retained = 257;
        invalid.owned(1, 0, 16).expect_err("invalid history counts");
        let mut invalid = page();
        invalid.items.push(RawItem {
            id: 1,
            time_ticks: 1,
            title: String::new(),
            message: String::new(),
            read: true,
            can_show: false,
            show_error: None,
        });
        assert!(
            invalid.owned(1, 0, 16).is_err(),
            "items exceed retained count"
        );
        let valid = page()
            .owned(7, 0, 16)
            .expect("empty after intentional clear");
        assert_eq!(
            valid.status.snapshot_cursor,
            NotificationCursor {
                session: 7,
                value: 3
            }
        );
        assert_eq!(valid.next_cursor, valid.status.snapshot_cursor);
    }
}
