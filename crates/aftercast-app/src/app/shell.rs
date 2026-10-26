use super::{AppState, Phase, import_log, refresh, select_encounter, start, stop, tr};
use crate::components::{Badge, Button, ButtonVariant};
use dioxus::prelude::*;

#[component]
pub(super) fn Header() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let phase = state.read().phase;
    rsx! {
        header { class: "topbar flex min-h-17 items-center gap-4 border-b border-border px-4 min-[1000px]:gap-7 min-[1000px]:px-6",
            div { class: "flex items-center gap-2.5 text-[19px] font-semibold tracking-tight",
                icons::Activity { class: "size-5 text-primary" }
                span { "aftercast" }
            }
            span {
                class: "hidden min-[1000px]:inline-flex",
                "data-testid": "host-status",
                Badge { class: "font-normal text-muted-foreground".to_owned(),
                    span { class: if phase == Phase::Ready { "size-1.5 rounded-full bg-primary" } else { "size-1.5 rounded-full bg-muted-foreground" } }
                    {tr(phase.key())}
                }
            }
            div { class: "ml-auto flex items-center gap-2",
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: phase != Phase::Ready || state.read().plugins.is_empty(),
                    onclick: move |_| import_log(state),
                    icons::FileUp {}
                    {tr("import-log")}
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
    let rows = state.read().encounters.clone();
    let selected = state.read().selected;
    let offset = state.read().offset;
    rsx! {
        aside { class: "flex min-h-0 flex-col border-r border-border bg-ctp-mantle",
            nav {
                class: "border-b border-border px-3 py-4",
                "aria-label": tr("navigation"),
                for screen in ["analysis", "plugins", "settings"] {
                    button {
                        class: "my-0.5 flex w-full items-center gap-2.5 rounded-md px-3 py-2.5 text-left text-muted-foreground hover:bg-accent hover:text-foreground aria-[current=page]:bg-accent aria-[current=page]:text-foreground [&_svg]:size-4 [&[aria-current=page]_svg]:text-primary",
                        "aria-current": if state.read().screen == screen { "page" } else { "false" },
                        onclick: move |_| state.write().screen = screen,
                        match screen {
                            "analysis" => rsx! {
                                icons::ChartNoAxesCombined {}
                            },
                            "plugins" => rsx! {
                                icons::Puzzle {}
                            },
                            _ => rsx! {
                                icons::Settings2 {}
                            },
                        }
                        {tr(screen)}
                    }
                }
            }
            div { class: "flex items-center justify-between px-4 pt-4 pb-1 [&_h2]:text-xs [&_h2]:font-semibold [&_h2]:text-muted-foreground",
                h2 { {tr("encounters")} }
                Button {
                    variant: ButtonVariant::Ghost,
                    aria_label: tr("refresh"),
                    class: "px-2".to_owned(),
                    disabled: state.read().phase != Phase::Ready || state.read().loading,
                    onclick: move |_| refresh(state, offset),
                    icons::RefreshCw {}
                }
            }
            div { class: "min-h-0 flex-1 overflow-auto",
                if rows.is_empty() {
                    p { class: "px-4 py-3 text-xs leading-6 text-muted-foreground",
                        {tr("encounters-empty")}
                    }
                }
                for (index, row) in rows.iter().enumerate() {
                    {
                        let id = row.id;
                        rsx! {
                            button {
                                key: "{index}",
                                class: "encounter-item block w-full border-b border-border px-5 py-3 text-left hover:bg-accent aria-pressed:bg-accent aria-pressed:text-primary",
                                "aria-pressed": selected == Some(id),
                                onclick: move |_| select_encounter(state, id),
                                span { class: "block truncate font-medium", "{row.title}" }
                                span { class: "block truncate text-xs text-muted-foreground", "{row.zone}" }
                            }
                        }
                    }
                }
            }
            if state.read().total > 128 {
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
