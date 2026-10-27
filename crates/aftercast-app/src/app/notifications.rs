use super::{AppState, Phase, tr};
use crate::components::{Accordion, Badge, Button, ButtonVariant};
use aftercast::{Notification, NotificationCursor, NotificationId, NotificationPage};
use dioxus::{core::spawn_forever, prelude::*};

#[derive(Default)]
pub(super) struct NotificationState {
    loaded: bool,
    busy: bool,
    page: Option<NotificationPage>,
    cursor: Option<NotificationCursor>,
    previous: Vec<Option<NotificationCursor>>,
}
impl NotificationState {
    fn receive(&mut self, item: Notification) {
        if let Some(page) = &mut self.page
            && let Some(previous) = page
                .items
                .iter_mut()
                .find(|previous| previous.id == item.id)
        {
            *previous = item;
        }
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
        self.cursor = cursor;
        self.page = Some(page);
    }
}
impl AppState {
    fn begin_notification_request(&mut self, show: bool) -> Option<u64> {
        if self.phase != Phase::Ready || self.notifications.busy {
            return None;
        }
        self.notifications.busy = true;
        if show {
            self.phase = Phase::ShowingNotification;
        }
        self.error = None;
        Some(self.revision.session)
    }
    fn finish_notification_request(&mut self, session: u64) -> bool {
        if self.revision.session != session {
            return false;
        }
        self.notifications.busy = false;
        if self.phase == Phase::ShowingNotification {
            self.phase = Phase::Ready;
        }
        true
    }
    fn receive_notification_show(
        &mut self,
        session: u64,
        result: Result<Notification, aftercast::Error>,
    ) -> bool {
        if !self.finish_notification_request(session) {
            return false;
        }
        match result {
            Ok(item) => self.notifications.receive(item),
            Err(error) => self.fail(error),
        }
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
    let Some(session) = state.write().begin_notification_request(false) else {
        return;
    };
    state.write().notifications.loaded = true;
    spawn_forever(async move {
        let result = handle.notifications(cursor, 16).await;
        if !state.write().finish_notification_request(session) {
            return;
        }
        match result {
            Ok(page) => {
                state
                    .write()
                    .notifications
                    .accept_page(movement, cursor, page);
            }
            Err(error) => state.write().fail(error),
        }
    });
}
fn show(mut state: Signal<AppState>, id: NotificationId) {
    let Some(handle) = state.read().handle() else {
        return;
    };
    // Bind the rendered ID, not a mutable selection, before admitting an explicit action.
    let Some(session) = state.write().begin_notification_request(true) else {
        return;
    };
    spawn_forever(async move {
        let result = handle.show_notification(id).await;
        state.write().receive_notification_show(session, result);
    });
}
#[component]
pub(super) fn NotificationsPane() -> Element {
    let state = use_context::<Signal<AppState>>();
    use_effect(move || {
        if state.read().phase == Phase::Ready
            && !state.read().notifications.loaded
            && !state.read().notifications.busy
        {
            load(state, Movement::First);
        }
    });
    let current = state.read();
    let blocked = current.phase != Phase::Ready || current.notifications.busy;
    let previous = !current.notifications.previous.is_empty();
    let page = current.notifications.page.clone();
    // The opaque cursor prevents comparing IDs; a full page may have another page, including an empty last page.
    let next = page.as_ref().is_some_and(|page| page.items.len() == 16);
    let busy = current.notifications.busy;
    let stopped = current.phase == Phase::Idle;
    drop(current);
    rsx! {
        section { class: "space-y-6 p-6 min-[1000px]:p-8",
            h1 { class: "text-2xl font-semibold", {tr("notifications")} }
            p { class: "max-w-3xl text-sm leading-7 text-muted-foreground",
                {tr("notifications-help")}
            }
            if stopped {
                p { {tr("notifications-start")} }
            }
            if busy {
                p { role: "status", {tr("notifications-working")} }
            }
            div { class: "flex flex-wrap gap-2",
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: blocked,
                    onclick: move |_| load(state, Movement::First),
                    {tr("notifications-refresh")}
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: blocked || !previous,
                    onclick: move |_| load(state, Movement::Previous),
                    {tr("previous")}
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: blocked || !next,
                    onclick: move |_| load(state, Movement::Next),
                    {tr("next")}
                }
            }
            if let Some(page) = page {
                p { class: "text-sm text-muted-foreground",
                    {tr("notifications-omitted")}
                    " {page.omitted}"
                }
                if page.items.is_empty() {
                    p { {tr("notifications-empty")} }
                }
                for item in page.items {
                    {
                        let id = item.id;
                        let key = format!("{id:?}");
                        rsx! {
                            Accordion {
                                key: "{key}",
                                default_open: true,
                                header: rsx! {
                                    span { class: "flex min-w-0 items-center gap-2",
                                        span { class: "truncate", "{item.title}" }
                                        Badge { {tr(if item.read { "notification-read" } else { "notification-unread" })} }
                                    }
                                },
                                div { class: "space-y-3 px-4 pb-4",
                                    p { class: "text-xs text-muted-foreground", {notification_time(item.time_ticks)} }
                                    p { class: "whitespace-pre-wrap break-words text-sm", "{item.message}" }
                                    if let Some(error) = item.show_error {
                                        p { role: "alert", class: "text-sm text-destructive", "{error}" }
                                    }
                                    Button {
                                        variant: ButtonVariant::Outline,
                                        disabled: blocked || !item.can_show,
                                        onclick: move |_| show(state, id),
                                        {tr("notification-show")}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn notification_time(ticks: u64) -> String {
    super::encounter_context::format_time(aftercast::LogTimestamp {
        ticks,
        utc_ticks: None,
        kind: aftercast::LogTimeKind::Unspecified,
    })
}

#[cfg(test)]
pub(super) async fn test_contract(host: &aftercast::Host, ui: &mut AppState) {
    let fixture = camino::Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            aftercast::PluginConfig::builder()
                .path(fixture)
                .type_name("Aftercast.PluginFixture.NotificationPlugin")
                .build(),
        )
        .await
        .expect("synthetic notifications");
    host.initialize_plugin(plugin)
        .await
        .expect("initialize notifications");
    ui.notifications.page = Some(host.notifications(None, 16).await.expect("history"));
    let id = ui.notifications.page.as_ref().expect("page").items[0].id;
    let session = ui.begin_notification_request(true).expect("Show admitted");
    assert!(
        ui.begin_notification_request(true).is_none(),
        "duplicate click rejected"
    );
    assert!(
        ui.begin_notification_request(false).is_none(),
        "paging blocked until action completes"
    );
    ui.screen = "settings";
    assert!(ui.receive_notification_show(session, host.show_notification(id).await));
    assert_eq!(ui.screen, "settings", "completion must not navigate");
    assert!(ui.notifications.page.as_ref().expect("page").items[0].read);
    assert!(ui.notifications.page.as_ref().expect("page").items[0].can_show);
    let session = ui
        .begin_notification_request(true)
        .expect("intentional repeat");
    assert!(ui.receive_notification_show(session, host.show_notification(id).await));
    notification_stale_replies(ui, host, id).await;
}
#[cfg(test)]
async fn notification_stale_replies(ui: &mut AppState, host: &aftercast::Host, id: NotificationId) {
    let old = ui
        .begin_notification_request(true)
        .expect("old pending action");
    let response = host.show_notification(id).await;
    ui.revision.session += 1;
    ui.notifications = Default::default();
    ui.phase = Phase::Stopping;
    assert!(!ui.receive_notification_show(old, response));
    assert!(ui.phase == Phase::Stopping && ui.notifications.page.is_none());
    ui.phase = Phase::Ready;
    let current = ui
        .begin_notification_request(true)
        .expect("new worker action");
    assert!(!ui.finish_notification_request(old));
    assert!(ui.notifications.busy && ui.phase == Phase::ShowingNotification);
    assert!(ui.receive_notification_show(current, Err(aftercast::Error::Busy)));
    assert!(ui.phase == Phase::Ready && !ui.notifications.busy && ui.error.is_some());
}
