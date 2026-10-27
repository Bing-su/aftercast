use super::{AppState, tr};
use crate::components::{Accordion, Button, ButtonVariant};
use aftercast::{
    EncounterAction, EncounterId, EncounterLogs, EncounterPage, HostHandle, SourceLogLink,
};
use dioxus::prelude::*;

const PAGE: u32 = 50;

#[derive(Default)]
struct ContextState {
    generation: u64,
    busy: bool,
    logs_view: bool,
    actions_offset: u32,
    logs_offset: u32,
    actions: Option<EncounterPage<EncounterAction>>,
    logs: Option<EncounterLogs>,
    source: Option<(u32, i32)>,
    error: Option<String>,
    unavailable: bool,
}

impl ContextState {
    fn receive(&mut self, generation: u64, result: Result<PageResult, aftercast::Error>) {
        if self.generation != generation {
            return;
        }
        self.busy = false;
        match result {
            Ok(PageResult::Actions(page)) => {
                self.unavailable = page.is_none();
                self.actions = page;
            }
            Ok(PageResult::Logs(page)) => {
                self.unavailable = page.is_none();
                self.logs = page;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
}

fn source_matches(source: Option<(u32, i32)>, index: u32, line: &aftercast::EncounterLog) -> bool {
    source == Some((index, line.time_sorter)) && line.source == SourceLogLink::Retained(index)
}

fn format_time(time: aftercast::LogTimestamp) -> String {
    // Format the source wall-clock ticks without converting local/unspecified times to another zone.
    let ticks = time.ticks as i64;
    let seconds = ticks / 10_000_000 - 62_135_596_800;
    jiff::Timestamp::from_second(seconds)
        .map(|time| {
            format!(
                "{}.{:07}",
                time.strftime("%Y-%m-%d %H:%M:%S"),
                ticks % 10_000_000
            )
        })
        .unwrap_or_else(|_| tr("source-unavailable"))
}

enum PageResult {
    Actions(Option<EncounterPage<EncounterAction>>),
    Logs(Option<EncounterLogs>),
}

fn load(
    mut state: Signal<ContextState>,
    handle: HostHandle,
    id: EncounterId,
    logs: bool,
    offset: u32,
    source: Option<(u32, i32)>,
) {
    let generation = {
        let mut state = state.write();
        state.generation += 1;
        state.busy = true;
        state.logs_view = logs;
        state.source = source;
        state.error = None;
        state.unavailable = false;
        if logs {
            state.logs_offset = offset;
            state.logs = None;
        } else {
            state.actions_offset = offset;
            state.actions = None;
        }
        state.generation
    };
    spawn(async move {
        let result = if logs {
            handle
                .encounter_logs(id, offset, PAGE)
                .await
                .map(PageResult::Logs)
        } else {
            handle
                .encounter_actions(id, offset, PAGE)
                .await
                .map(PageResult::Actions)
        };
        state.write().receive(generation, result);
    });
}

#[component]
pub(super) fn EncounterContext(id: EncounterId) -> Element {
    let app = use_context::<Signal<AppState>>();
    let mut state = use_signal(ContextState::default);
    use_effect(move || {
        if let Some(handle) = app.peek().handle() {
            load(state, handle, id, false, 0, None);
        }
    });
    let logs_view = state.read().logs_view;
    let busy = state.read().busy;
    let offset = if logs_view {
        state.read().logs_offset
    } else {
        state.read().actions_offset
    };
    let total = if logs_view {
        state.read().logs.as_ref().map_or(0, |page| page.page.total)
    } else {
        state.read().actions.as_ref().map_or(0, |page| page.total)
    };
    let actions = state.read().actions.clone();
    let logs = state.read().logs.clone();
    let source = state.read().source;
    rsx! {
        Accordion {
            class: "mt-5 border border-border rounded-md".to_owned(),
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            default_open: true,
            header: rsx! {
                {tr("encounter-context")}
            },
            section { class: "space-y-4 p-4", "aria-label": tr("encounter-context"),
                div { class: "flex flex-wrap items-center gap-3",
                    Button {
                        variant: ButtonVariant::Outline,
                        onclick: move |_| {
                            if let Some(handle) = app.peek().handle() {
                                let next_offset = if logs_view { state.peek().actions_offset } else { 0 };
                                load(state, handle, id, !logs_view, next_offset, None);
                            }
                        },
                        {tr(if logs_view { "back-actions" } else { "view-logs" })}
                    }
                    if busy {
                        Button {
                            variant: ButtonVariant::Ghost,
                            onclick: move |_| {
                                let mut state = state.write();
                                state.generation += 1;
                                state.busy = false;
                                state.error = Some(tr("query-cancelled"));
                            },
                            {tr("cancel")}
                        }
                    }
                }
                if busy {
                    p { role: "status", {tr("loading-selection")} }
                }
                if let Some(error) = state.read().error.clone() {
                    p {
                        role: "alert",
                        class: "text-sm text-ctp-red break-words",
                        "{error}"
                    }
                }
                if state.read().unavailable {
                    p { role: "status", {tr("encounter-unavailable")} }
                }
                if logs_view {
                    if let Some(logs) = logs {
                        p {
                            role: "status",
                            class: "text-sm text-muted-foreground tabular-nums",
                            {tr("retained-logs")}
                            " {logs.page.total} · "
                            {tr("omitted-logs")}
                            " {logs.dropped}"
                        }
                        if logs.page.total == 0 {
                            p { {tr("empty-logs")} }
                        }
                        if let Some((index, sorter)) = source {
                            if index != offset
                                || logs
                                    .page
                                    .items
                                    .first()
                                    .is_none_or(|line| !source_matches(Some((index, sorter)), offset, line))
                            {
                                p { role: "alert",
                                    {
                                        tr(
                                            if logs
                                                .page
                                                .items
                                                .first()
                                                .is_some_and(|line| line.source == SourceLogLink::Ambiguous)
                                            {
                                                "source-ambiguous"
                                            } else {
                                                "source-unavailable"
                                            },
                                        )
                                    }
                                }
                            }
                        }
                        ol { class: "divide-y divide-border border-y border-border",
                            for (index, line) in logs.page.items.iter().enumerate() {
                                li {
                                    class: if source_matches(source, offset + index as u32, line) { "bg-accent p-3 outline-1 outline-primary" } else { "p-3" },
                                    "aria-current": if source_matches(source, offset + index as u32, line) { "true" } else { "false" },
                                    p { class: "mb-2 text-xs text-muted-foreground tabular-nums",
                                        "#{offset + index as u32 + 1} · "
                                        {format_time(line.time)}
                                        " · #{line.time_sorter} · {line.log_type}"
                                    }
                                    pre { class: "whitespace-pre-wrap break-all text-sm",
                                        "{line.text}"
                                    }
                                }
                            }
                        }
                    }
                } else if let Some(actions) = actions {
                    p { class: "text-xs text-muted-foreground", {tr("action-source-help")} }
                    if actions.total == 0 {
                        p { {tr("empty-actions")} }
                    }
                    ol { class: "divide-y divide-border border-y border-border",
                        for action in actions.items {
                            li { class: "py-2",
                                Button {
                                    variant: ButtonVariant::Ghost,
                                    class: "h-auto w-full flex-col items-start whitespace-normal py-3 text-left".to_owned(),
                                    disabled: busy || !matches!(action.source, SourceLogLink::Retained(_)),
                                    onclick: move |_| {
                                        if let (Some(handle), SourceLogLink::Retained(index)) = (
                                            app.peek().handle(),
                                            action.source,
                                        ) {
                                            load(state, handle, id, true, index, Some((index, action.time_sorter)));
                                        }
                                    },
                                    span {
                                        "{action.attacker} → {action.victim} · {action.attack} · {action.damage}"
                                    }
                                    span { class: "text-xs text-muted-foreground tabular-nums",
                                        {format_time(action.time)}
                                        " · #{action.time_sorter} · "
                                        {
                                            tr(
                                                match action.source {
                                                    SourceLogLink::Retained(_) => "view-source",
                                                    SourceLogLink::Missing => "source-unavailable",
                                                    SourceLogLink::Ambiguous => "source-ambiguous",
                                                },
                                            )
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "flex items-center gap-3 text-xs text-muted-foreground",
                    Button {
                        variant: ButtonVariant::Ghost,
                        disabled: busy || offset == 0,
                        onclick: move |_| {
                            if let Some(handle) = app.peek().handle() {
                                load(state, handle, id, logs_view, offset.saturating_sub(PAGE), None);
                            }
                        },
                        {tr("previous")}
                    }
                    span {
                        "{offset.saturating_add(1).min(total)}–{offset.saturating_add(PAGE).min(total)} / {total}"
                    }
                    Button {
                        variant: ButtonVariant::Ghost,
                        disabled: busy || offset.saturating_add(PAGE) >= total,
                        onclick: move |_| {
                            if let Some(handle) = app.peek().handle() {
                                load(state, handle, id, logs_view, offset + PAGE, None);
                            }
                        },
                        {tr("next")}
                    }
                    Button {
                        variant: ButtonVariant::Ghost,
                        disabled: busy,
                        onclick: move |_| {
                            if let Some(handle) = app.peek().handle() {
                                load(state, handle, id, logs_view, offset, source);
                            }
                        },
                        {tr("refresh")}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ContextState, PageResult, source_matches};
    #[test]
    fn source_highlight_rejects_a_duplicate_added_after_the_action_query() {
        let mut line = aftercast::EncounterLog {
            time: aftercast::LogTimestamp {
                ticks: 0,
                utc_ticks: None,
                kind: aftercast::LogTimeKind::Unspecified,
            },
            time_sorter: 42,
            log_type: 0,
            text: "source".into(),
            source: aftercast::SourceLogLink::Retained(0),
        };
        assert!(source_matches(Some((0, 42)), 0, &line));
        line.source = aftercast::SourceLogLink::Ambiguous;
        assert!(!source_matches(Some((0, 42)), 0, &line));
    }
    #[test]
    fn cancelled_or_replaced_queries_cannot_publish_late_pages_or_errors() {
        let mut state = ContextState {
            generation: 4,
            busy: true,
            ..ContextState::default()
        };
        state.receive(3, Err(aftercast::Error::Closed));
        assert!(state.error.is_none() && state.busy);
        state.receive(3, Ok(PageResult::Logs(None)));
        assert!(!state.unavailable && state.busy);
        state.receive(4, Ok(PageResult::Logs(None)));
        assert!(state.unavailable && !state.busy);
    }
}
