use super::{AppState, Phase, import_log, refresh, select_encounter, start, stop, tr};
use crate::components::{Accordion, Badge, Button, ButtonVariant, Input};
use dioxus::prelude::*;

#[component]
pub(super) fn Header() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let phase = state.read().phase;
    rsx! {
        header { class: "topbar flex min-h-16 border-b border-border bg-ctp-mantle",
            nav {
                class: "flex min-w-0 flex-1 overflow-x-auto",
                "aria-label": tr("navigation"),
                for screen in [
                    "analysis",
                    "triggers",
                    "timers",
                    "notifications",
                    "plugins",
                    "import",
                    "history",
                    "settings",
                ]
                {
                    button {
                        class: "workflow-tab shrink-0 px-4 text-sm min-[1280px]:px-5 min-[1280px]:text-base text-muted-foreground hover:bg-accent hover:text-foreground aria-[current=page]:border-primary aria-[current=page]:bg-accent aria-[current=page]:text-primary disabled:cursor-default disabled:opacity-40",
                        "aria-current": if state.read().screen == screen { "page" } else { "false" },
                        disabled: false,
                        title: tr(screen),
                        "aria-label": tr(screen),
                        "aria-describedby": (screen == "notifications").then_some("notification-unread-status"),
                        onclick: move |_| {
                            if state.read().screen != screen {
                                super::history_sources::leave(state);
                                state.write().interchange.leave();
                                state.write().reports.leave();
                                if screen != "analysis" {
                                    state.write().mini_parse.stop();
                                }
                                state.write().screen = screen;
                            }
                        },
                        {tr(screen)}
                        if screen == "notifications" {
                            super::notifications::UnreadBadge {}
                        }
                    }
                }
            }
            div { class: "flex shrink-0 items-center gap-1 px-2",
                super::import_progress::OpenButton {}
                Button {
                    variant: ButtonVariant::Ghost,
                    aria_label: tr("import-log"),
                    class: "px-2".to_owned(),
                    disabled: !state.read().can_full_import(),
                    onclick: move |_| import_log(state),
                    icons::FileUp {}
                }
                if phase == Phase::Idle {
                    Button {
                        id: "start-host",
                        disabled: state.read().file.is_none(),
                        onclick: move |_| start(state),
                        icons::Play {}
                        {tr("start-host")}
                    }
                } else {
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: matches!(phase, Phase::Starting | Phase::Stopping),
                        onclick: move |_| stop(state),
                        icons::Square {}
                        {tr("stop-host")}
                    }
                }
                Button {
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| state.write().exit_requested = true,
                    icons::LogOut {}
                    {tr("quit")}
                }
            }
        }
    }
}

#[component]
pub(super) fn Sidebar() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut search = use_signal(String::new);
    let rows = state.read().encounters.clone();
    let selected = state.read().selected;
    let offset = state.read().offset;
    let screen = state.read().screen;
    let query = search.read().to_lowercase();
    let zones: std::collections::BTreeSet<_> = rows
        .iter()
        .filter(|row| {
            row.title.to_lowercase().contains(&query) || row.zone.to_lowercase().contains(&query)
        })
        .map(|row| row.zone.clone())
        .collect();
    rsx! {
        aside { class: "flex min-h-0 flex-col border-r border-border bg-ctp-mantle",
            div { class: "flex items-center justify-between px-4 pt-4 pb-1 [&_h2]:text-sm [&_h2]:font-semibold [&_h2]:text-muted-foreground",
                h2 { {tr(if screen == "analysis" { "data-explorer" } else { screen })} }
                if screen == "analysis" {
                    Button {
                        variant: ButtonVariant::Ghost,
                        aria_label: tr("refresh"),
                        class: "px-2".to_owned(),
                        disabled: state.read().phase != Phase::Ready || state.read().loading,
                        onclick: move |_| refresh(state, offset),
                        icons::RefreshCw {}
                    }
                }
            }
            div { class: "min-h-0 flex-1 overflow-auto",
                if screen == "analysis" {
                    div { class: "p-3",
                        label { class: "sr-only", r#for: "encounter-search",
                            {tr("search-encounters")}
                        }
                        Input {
                            id: "encounter-search".to_owned(),
                            placeholder: tr("search-encounters"),
                            value: search(),
                            oninput: move |event: FormEvent| search.set(event.value()),
                        }
                    }
                    if rows.is_empty() {
                        p { class: "px-4 py-3 text-xs leading-6 text-muted-foreground",
                            {tr("encounters-empty")}
                        }
                    }
                    for zone in zones {
                        Accordion {
                            key: "{zone}",
                            class: "explorer-group px-2".to_owned(),
                            default_open: true,
                            header: rsx! {
                                span { class: "flex min-w-0 items-center gap-2",
                                    icons::Folder { class: "size-4 shrink-0 text-muted-foreground" }
                                    span { class: "truncate", "{zone}" }
                                }
                            },
                            for (index, row) in rows.iter()
                                .enumerate()
                                .filter(|(_, row)| {
                                    row.zone == zone
                                        && (row.title.to_lowercase().contains(&query)
                                            || row.zone.to_lowercase().contains(&query))
                                })
                            {
                                {
                                    let id = row.id;
                                    rsx! {
                                        button {
                                            key: "{index}",
                                            class: "encounter-item my-0.5 block w-full rounded px-5 py-2 text-left hover:bg-accent aria-pressed:bg-accent aria-pressed:text-primary",
                                            "aria-pressed": selected == Some(id),
                                            onclick: move |_| select_encounter(state, id),
                                            span { class: "block truncate font-medium", "{row.title}" }
                                            span { class: "block truncate text-xs text-muted-foreground", "{row.duration_ticks / 10_000_000} s" }
                                        }
                                        if selected == Some(id) {
                                            if let Some(detail) = state.read().detail.clone() {
                                                div { class: "ml-5 border-l border-border pl-2",
                                                    button {
                                                        class: "tree-row",
                                                        "aria-pressed": state.read().combatant.is_none(),
                                                        onclick: move |_| {
                                                            state.write().combatant = None;
                                                            state.write().category = None;
                                                        },
                                                        icons::Users { class: "size-4 shrink-0" }
                                                        {tr("all-participants")}
                                                    }
                                                    for participant in detail.combatants {
                                                        {
                                                            let name = participant.name.clone();
                                                            let categories: std::collections::BTreeSet<_> = state
                                                                .read()
                                                                .attacks
                                                                .iter()
                                                                .filter(|row| row.combatant == name)
                                                                .map(|row| (row.category.clone(), row.outgoing))
                                                                .collect();
                                                            rsx! {
                                                                Accordion {
                                                                    key: "{id:?}-{name}",
                                                                    class: "explorer-group".to_owned(),
                                                                    default_open: state.read().combatant.as_ref() == Some(&name),
                                                                    header: rsx! {
                                                                        span { class: "flex min-w-0 items-center gap-2",
                                                                            icons::Users { class: "size-4 shrink-0 text-muted-foreground" }
                                                                            span { class: "truncate", "{name}" }
                                                                        }
                                                                    },
                                                                    button {
                                                                        class: "tree-row",
                                                                        "aria-pressed": state.read().combatant.as_ref() == Some(&name) && state.read().category.is_none(),
                                                                        onclick: {
                                                                            let name = name.clone();
                                                                            move |_| {
                                                                                state.write().combatant = Some(name.clone());
                                                                                state.write().category = None;
                                                                            }
                                                                        },
                                                                        {tr("all-categories")}
                                                                    }
                                                                    for (category, outgoing) in categories {
                                                                        button {
                                                                            class: "tree-row pl-4",
                                                                            "aria-pressed": state.read().combatant.as_ref() == Some(&name)
                                                                                && state.read().category.as_ref() == Some(&(category.clone(), outgoing)),
                                                                            onclick: {
                                                                                let name = name.clone();
                                                                                let category = category.clone();
                                                                                move |_| {
                                                                                    state.write().combatant = Some(name.clone());
                                                                                    state.write().category = Some((category.clone(), outgoing));
                                                                                }
                                                                            },
                                                                            span { class: "min-w-0 flex-1 truncate", "{category}" }
                                                                            span { class: "text-xs text-muted-foreground",
                                                                                {tr(if outgoing { "outgoing" } else { "incoming" })}
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
                                    }
                                }
                            }
                        }
                    }
                } else {
                    div { class: "space-y-2 px-4 py-4 text-sm leading-7 text-muted-foreground",
                        p {
                            {
                                tr(
                                    match screen {
                                        "settings" => "settings-description",
                                        "plugins" => "plugins-description",
                                        _ => "import-description",
                                    },
                                )
                            }
                        }
                    }
                }
            }
            if screen == "analysis" && state.read().total > 128 {
                div { class: "flex items-center justify-between border-t border-border p-2",
                    Button {
                        variant: ButtonVariant::Ghost,
                        disabled: offset == 0 || state.read().loading,
                        onclick: move |_| refresh(state, offset.saturating_sub(128)),
                        {tr("previous")}
                    }
                    span { class: "text-xs text-muted-foreground", "{offset} / {state.read().total}" }
                    Button {
                        variant: ButtonVariant::Ghost,
                        disabled: offset.saturating_add(128) >= state.read().total || state.read().loading,
                        onclick: move |_| refresh(state, offset.saturating_add(128)),
                        {tr("next")}
                    }
                }
            }
            if screen == "analysis" {
                div { class: "border-t border-border p-3",
                    Button {
                        variant: ButtonVariant::Outline,
                        class: "w-full".to_owned(),
                        disabled: state.read().phase != Phase::Ready,
                        onclick: move |_| end_combat(state),
                        icons::Square {}
                        {tr("end-combat")}
                    }
                }
            }
            div { class: "flex items-center gap-2 border-t border-border p-4 text-xs text-muted-foreground",
                icons::HardDrive { class: "size-4" }
                span { {tr("local-data")} }
            }
        }
    }
}

#[component]
pub(super) fn Footer() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    rsx! {
        footer { class: "flex min-h-9 items-center gap-5 border-t border-border bg-ctp-mantle px-5 py-2 text-xs text-muted-foreground [&>span]:max-w-[60%] [&>span]:truncate",
            div { "data-testid": "host-status", class: "shrink-0",
                Badge { class: "border-0 px-0 font-normal".to_owned(),
                    span { class: if state.read().phase == Phase::Ready { "size-1.5 rounded-full bg-ctp-green" } else { "size-1.5 rounded-full bg-muted-foreground" } }
                    {tr(state.read().phase.key())}
                }
            }
            span { role: "status", "{state.read().message}" }
            div { class: "ml-auto flex gap-4",
                button {
                    onclick: move |_| diagnostics(state),
                    disabled: state.read().phase != Phase::Ready,
                    {tr("diagnostics")}
                }
                button {
                    onclick: move |_| {
                        let root = state.read().root.clone();
                        spawn(async move {
                            match tokio::task::spawn_blocking(move || open::that(root.as_std_path()))
                                .await
                            {
                                Ok(Ok(())) => {}
                                Ok(Err(error)) => state.write().fail(error),
                                Err(error) => state.write().fail(error),
                            }
                        });
                    },
                    {tr("open-folder")}
                }
                span { "v0.1.0" }
            }
        }
    }
}

fn end_combat(mut state: Signal<AppState>) {
    let Some(handle) = state.read().handle() else {
        return;
    };
    let session = state.read().revision.session;
    spawn(async move {
        if let Err(error) = handle.end_combat().await {
            state.write().fail_session(session, error);
        }
        if state.read().revision.session == session {
            refresh(state, 0);
        }
    });
}

fn diagnostics(mut state: Signal<AppState>) {
    let Some(handle) = state.read().handle() else {
        return;
    };
    let session = state.read().revision.session;
    spawn(async move {
        match handle.diagnostics().await {
            Ok(report) if state.read().revision.session == session => {
                let summary = format!(
                    "{} · {}\n{}: {} / {}\n{}",
                    jiff::Timestamp::now(),
                    tr("diagnostics"),
                    tr("diagnostic-count"),
                    report.recent.len(),
                    report.total,
                    report.recent.join("\n")
                );
                state.write().message = summary.clone();
                match tokio::task::spawn_blocking(move || {
                    arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(summary))
                })
                .await
                {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => state.write().fail_session(session, error),
                    Err(error) => state.write().fail_session(session, error),
                }
            }
            Ok(_) => {}
            Err(error) => state.write().fail_session(session, error),
        }
    });
}
