use super::{AppState, Phase, tr};
use aftercast::{
    Notification, NotificationChange, NotificationCursor, NotificationId, NotificationPage,
    NotificationStatus,
};
use dioxus::{core::spawn_forever, prelude::*};
#[cfg(test)]
mod tests;
mod view;
#[cfg(test)]
pub(super) use tests::test_contract;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Read,
    Show,
    Bulk,
    Status,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Request {
    session: u64,
    serial: u64,
    kind: Kind,
}
#[derive(Default)]
pub(super) struct NotificationState {
    loaded: bool,
    serial: u64,
    pending: Option<Request>,
    page: Option<NotificationPage>,
    status: Option<NotificationStatus>,
    status_error: Option<String>,
    cursor: Option<NotificationCursor>,
    previous: Vec<Option<NotificationCursor>>,
    message: String,
}
impl NotificationState {
    fn busy(&self) -> bool {
        self.pending
            .is_some_and(|request| request.kind != Kind::Status)
    }
    fn receive(&mut self, item: Notification) {
        if let Some(page) = &mut self.page
            && let Some(previous) = page
                .items
                .iter_mut()
                .find(|previous| previous.id == item.id)
        {
            *previous = item;
        }
        // Show can add more notifications, so never infer whole-history counts from the visible item.
        self.status = None;
    }
    fn accept_page(
        &mut self,
        movement: Movement,
        cursor: Option<NotificationCursor>,
        page: NotificationPage,
    ) {
        match movement {
            Movement::First => self.previous.clear(),
            Movement::Next => {
                if self.previous.len() == 256 {
                    self.previous.remove(0);
                }
                self.previous.push(self.cursor);
            }
            Movement::Previous => {
                self.previous.pop();
            }
        }
        self.status = Some(page.status.clone());
        self.status_error = None;
        self.cursor = cursor;
        self.page = Some(page);
    }
}
impl AppState {
    fn begin_notification_request(&mut self, show: bool) -> Option<Request> {
        self.begin_notification(if show { Kind::Show } else { Kind::Read })
    }
    fn begin_notification(&mut self, kind: Kind) -> Option<Request> {
        if self.phase != Phase::Ready
            || self.notifications.busy()
            || self.saving
            || self.exit_requested
            || kind == Kind::Status
                && (self.notifications.pending.is_some()
                    || self.loading
                    || self.detail_loading
                    || self.plugin_ui_pending)
        {
            return None;
        }
        // A user action supersedes a model-only count refresh; its late reply cannot replace the newer result.
        self.notifications.serial += 1;
        let token = Request {
            session: self.revision.session,
            serial: self.notifications.serial,
            kind,
        };
        self.notifications.pending = Some(token);
        match kind {
            Kind::Show => self.phase = Phase::ShowingNotification,
            Kind::Bulk => self.phase = Phase::UpdatingNotifications,
            _ => {}
        }
        if kind != Kind::Status {
            self.error = None;
            self.notifications.message.clear();
        }
        Some(token)
    }
    fn finish_notification_request(&mut self, token: Request) -> bool {
        if self.revision.session != token.session || self.notifications.pending != Some(token) {
            return false;
        }
        self.notifications.pending = None;
        if matches!(
            self.phase,
            Phase::ShowingNotification | Phase::UpdatingNotifications
        ) {
            self.phase = Phase::Ready;
        }
        true
    }
    fn receive_notification_show(
        &mut self,
        token: Request,
        result: Result<Notification, aftercast::Error>,
    ) -> bool {
        if !self.finish_notification_request(token) {
            return false;
        }
        match result {
            Ok(item) => self.notifications.receive(item),
            Err(error) => self.fail(error),
        }
        true
    }
    fn receive_notification_page(
        &mut self,
        token: Request,
        movement: Movement,
        cursor: Option<NotificationCursor>,
        result: Result<NotificationPage, aftercast::Error>,
    ) -> bool {
        if !self.finish_notification_request(token) {
            return false;
        }
        match result {
            Ok(page) => self.notifications.accept_page(movement, cursor, page),
            Err(error) => self.fail(error),
        }
        true
    }
    fn receive_notification_status(
        &mut self,
        token: Request,
        result: Result<NotificationStatus, aftercast::Error>,
    ) -> bool {
        if !self.finish_notification_request(token) {
            return false;
        }
        match result {
            Ok(status) => {
                self.notifications.status = Some(status);
                self.notifications.status_error = None;
            }
            Err(aftercast::Error::Busy) => {} // An admitted host operation may start after the idle check; retry on the next tick.
            Err(error) => {
                self.notifications.status = None;
                self.notifications.status_error = Some(error.to_string());
            }
        }
        true
    }
    fn begin_notification_bulk(&mut self, through: NotificationCursor) -> Option<Request> {
        if self.notifications.page.as_ref()?.status.snapshot_cursor != through {
            return None;
        }
        self.begin_notification(Kind::Bulk)
    }
    fn receive_notification_bulk(
        &mut self,
        token: Request,
        result: Result<NotificationChange, aftercast::Error>,
        message: &str,
    ) -> bool {
        if !self.finish_notification_request(token) {
            return false;
        }
        match result {
            Ok(change) => {
                self.notifications.message = format!("{message}: {}", change.changed);
                self.notifications
                    .accept_page(Movement::First, None, change.page);
            }
            Err(error) => self.fail(error),
        }
        true
    }
    fn toggle_notification_blink(&mut self) -> bool {
        if self.saving || self.file.is_none() {
            return false;
        }
        self.settings.ui.notification_blink = !self.settings.ui.notification_blink;
        true
    }
}
#[derive(Clone, Copy)]
enum Movement {
    First,
    Next,
    Previous,
}
fn load(mut state: Signal<AppState>, movement: Movement) {
    let Some(handle) = state.read().handle() else {
        return;
    };
    let cursor = {
        let current = state.read();
        match movement {
            Movement::First => None,
            Movement::Next => current
                .notifications
                .page
                .as_ref()
                .map(|page| page.next_cursor),
            Movement::Previous => current.notifications.previous.last().copied().flatten(),
        }
    };
    let Some(token) = state.write().begin_notification_request(false) else {
        return;
    };
    state.write().notifications.loaded = true;
    spawn_forever(async move {
        let result = handle.notifications(cursor, 16).await;
        state
            .write()
            .receive_notification_page(token, movement, cursor, result);
    });
}
fn show(mut state: Signal<AppState>, id: NotificationId) {
    let Some(handle) = state.read().handle() else {
        return;
    };
    let Some(token) = state.write().begin_notification_request(true) else {
        return;
    };
    spawn_forever(async move {
        let result = handle.show_notification(id).await;
        state.write().receive_notification_show(token, result);
    });
}
fn bulk(mut state: Signal<AppState>, through: NotificationCursor, clear: bool) {
    let Some(handle) = state.read().handle() else {
        return;
    };
    let Some(token) = state.write().begin_notification_bulk(through) else {
        return;
    };
    let message = tr(if clear {
        "notifications-cleared"
    } else {
        "notifications-marked"
    });
    spawn_forever(async move {
        let result = if clear {
            handle.clear_read_notifications(through).await
        } else {
            handle.mark_notifications_read(through).await
        };
        state
            .write()
            .receive_notification_bulk(token, result, &message);
    });
}
pub(super) async fn poll(mut state: Signal<AppState>) {
    loop {
        dioxus_sdk::time::sleep(std::time::Duration::from_secs(1)).await;
        // A failed shutdown can cancel Quit and keep this App alive; only unmount ends the polling future.
        if state.read().exit_requested {
            continue;
        }
        let Some(handle) = state.read().handle() else {
            continue;
        };
        let Some(token) = state.write().begin_notification(Kind::Status) else {
            continue;
        };
        let result = handle.notification_status().await;
        state.write().receive_notification_status(token, result);
    }
}
#[component]
pub(super) fn NotificationsPane() -> Element {
    rsx! {
        view::Panel {}
    }
}
#[component]
pub(super) fn UnreadBadge() -> Element {
    rsx! {
        view::BadgeIndicator {}
    }
}
