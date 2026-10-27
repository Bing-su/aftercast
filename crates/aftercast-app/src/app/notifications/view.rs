use super::*;
use crate::components::{Accordion, Badge, Button, ButtonVariant};
pub(super) fn badge_text(unread: u32) -> Option<String> {
    match unread {
        0 => None,
        1..=9 => Some(unread.to_string()),
        _ => Some("10+".into()),
    }
}
#[component]
pub(super) fn BadgeIndicator() -> Element {
    let state = use_context::<Signal<AppState>>();
    let current = state.read();
    if matches!(current.phase, Phase::Idle | Phase::Stopping) {
        return rsx! {};
    }
    let Some(status) = &current.notifications.status else {
        return rsx! {
            span { id: "notification-unread-status", class: "sr-only",
                {tr("notifications-count-unknown")}
            }
            span {
                "data-testid": "notification-badge",
                title: tr("notifications-count-unknown"),
                "…"
            }
        };
    };
    let Some(text) = badge_text(status.unread) else {
        return rsx! {};
    };
    let title = format!("{}: {}", tr("notification-unread"), status.unread);
    let class = if current.settings.ui.notification_blink {
        "notification-count-blink"
    } else {
        ""
    };
    rsx! {
        span { id: "notification-unread-status", class: "sr-only", "{title}" }
        span { "data-testid": "notification-badge", title,
            Badge { class: class.to_owned(), "{text}" }
        }
    }
}
#[component]
pub(super) fn Panel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    use_effect(move || {
        if state.read().phase == Phase::Ready
            && !state.read().notifications.loaded
            && !state.read().notifications.busy()
        {
            load(state, Movement::First);
        }
    });
    let current = state.read();
    let blocked = current.phase != Phase::Ready
        || current.notifications.busy()
        || current.saving
        || current.exit_requested;
    let previous = !current.notifications.previous.is_empty();
    let page = current.notifications.page.clone();
    // The opaque cursor prevents comparing IDs; a full page may have another page, including an empty last page.
    let next = page.as_ref().is_some_and(|page| page.items.len() == 16);
    let busy = current.notifications.busy();
    let stopped = current.phase == Phase::Idle;
    let file_available = current.file.is_some();
    let saving = current.saving;
    let blink = current.settings.ui.notification_blink;
    let dirty = current
        .file
        .as_ref()
        .is_some_and(|file| file.settings.ui.notification_blink != blink);
    let message = current.notifications.message.clone();
    let status_error = current.notifications.status_error.clone();
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
            div { class: "flex flex-wrap items-center gap-3",
                Button {
                    variant: ButtonVariant::Outline,
                    aria_pressed: blink,
                    disabled: saving || !file_available,
                    onclick: move |_| {
                        state.write().toggle_notification_blink();
                    },
                    {tr("notifications-blink")}
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: !state.read().can_save_settings(),
                    onclick: move |_| super::super::save_settings(state),
                    {tr("save-settings")}
                }
                if dirty {
                    p { class: "text-sm text-muted-foreground", {tr("notifications-blink-draft")} }
                }
            }
            if let Some(error) = status_error {
                p { role: "alert", "{error}" }
            }
            if !message.is_empty() {
                p { role: "status", "{message}" }
            }
            if let Some(page) = page {
                div { class: "flex flex-wrap gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked,
                        onclick: move |_| bulk(state, page.status.snapshot_cursor, false),
                        {tr("notifications-mark-all")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked,
                        onclick: move |_| bulk(state, page.status.snapshot_cursor, true),
                        {tr("notifications-clear-read")}
                    }
                }
                p { class: "text-xs text-muted-foreground", {tr("notifications-snapshot-help")} }
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
    super::super::encounter_context::format_time(aftercast::LogTimestamp {
        ticks,
        utc_ticks: None,
        kind: aftercast::LogTimeKind::Unspecified,
    })
}
