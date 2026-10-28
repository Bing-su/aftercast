use super::{
    AppState,
    log_window::{ContextEndpoint, WindowAppearance},
};
use crate::components::{Accordion, Button, ButtonVariant};
use aftercast::{AvoidanceNumber, EncounterId, SpecialReport};
use dioxus::prelude::*;
use std::sync::Arc;

#[cfg(test)]
mod tests;

#[derive(Clone)]
struct SharedReport(Arc<SpecialReport>);
impl PartialEq for SharedReport {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
async fn load(
    endpoint: ContextEndpoint,
    combatant: String,
) -> Result<Option<Arc<SpecialReport>>, String> {
    super::log_window::guard(&endpoint, async {
        endpoint
            .handle
            .special_report(endpoint.id, &combatant)
            .await
            .map_err(|error| error.to_string())
            .map(|report| report.map(Arc::new))
    })
    .await
}
fn source_text(report: &SpecialReport, source: Option<(usize, i32)>) -> Option<&str> {
    let (index, sorter) = source?;
    let log = report.logs.get(index)?;
    (log.time_sorter == sorter && usize::try_from(log.source_index).ok() == Some(index))
        .then_some(log.text.as_str())
}
fn number(value: &AvoidanceNumber, percent: bool) -> String {
    match value {
        AvoidanceNumber::Finite(value) => {
            format!("{:.2}", if percent { value * 100.0 } else { *value })
        }
        AvoidanceNumber::Special(value) => value.clone(),
    }
}

#[component]
pub(super) fn SpecialsPanel(id: EncounterId, combatant: String) -> Element {
    let app = use_context::<Signal<AppState>>();
    let Some(handle) = app.read().handle() else {
        return rsx! {};
    };
    let endpoint = ContextEndpoint {
        handle,
        id,
        generation: app.read().log_windows.generation(),
        lifetime: app.read().log_windows.clone(),
        appearance: WindowAppearance {
            title: "Special attacks".into(),
            theme: app.read().settings.ui.theme.clone(),
            language: app.read().settings.ui.language.clone(),
        },
    };
    let mut selected = use_signal(|| None::<(usize, i32)>);
    let name = combatant.clone();
    let mut result = use_resource(move || {
        let endpoint = endpoint.clone();
        let name = name.clone();
        async move { load(endpoint, name).await }
    });
    let state = *result.state().read();
    let pending = matches!(state, UseResourceState::Pending);
    let stopped = matches!(state, UseResourceState::Stopped);
    let current = if pending || stopped {
        None
    } else {
        result.read().clone()
    };
    rsx! {
        Accordion {
            class: "mt-5 border border-border rounded-md".to_owned(),
            default_open: false,
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! { "Special attacks — {combatant}" },
            div { class: "p-4 space-y-3",
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: pending,
                    onclick: move |_| {
                        if result.finished() {
                            selected.set(None);
                            result.restart();
                        }
                    },
                    "Refresh"
                }
                if pending {
                    Button {
                        variant: ButtonVariant::Outline,
                        onclick: move |_| {
                            selected.set(None);
                            result.cancel();
                        },
                        "Cancel"
                    }
                }
                if stopped {
                    p { "Report loading canceled." }
                } else {
                    match current {
                        None => rsx! {
                            p { "Loading special attacks…" }
                        },
                        Some(Err(error)) => rsx! {
                            p { role: "alert", "{error}" }
                        },
                        Some(Ok(None)) => rsx! {
                            p { "Selected encounter or combatant is unavailable." }
                        },
                        Some(Ok(Some(report))) => rsx! {
                            SpecialResults { report: SharedReport(report), selected }
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn SpecialResults(report: SharedReport, mut selected: Signal<Option<(usize, i32)>>) -> Element {
    let data = &report.0;
    let log = source_text(data, selected()).map(str::to_owned);
    rsx! {
        if matches!(data.summary.status, 2 | 3) {
            p { "Import is incomplete or still running; some actions may be missing." }
        }
        p {
            "Occurrences count distinct special events between None actions; ratios use ACT's None count. Retained logs: {data.logs.len()}; omitted: {data.dropped_logs}."
        }
        if data.rows.is_empty() {
            p { "No registered damage specials." }
        }
        for (index, row) in data.rows.iter().enumerate() {
            Accordion {
                key: "{data.revision}-{index}",
                class: "border border-border rounded-md".to_owned(),
                default_open: false,
                trigger_class: "px-3 py-2".to_owned(),
                header: rsx! { "{row.group}: {row.key}" },
                div { class: "p-3 space-y-2",
                    p { "Special swings: {row.swings}; encounter rate: {number(&row.rate, false)}" }
                    if let (Some(occurrences), Some(total), Some(ratio), Some(swings), Some(rate)) = (
                        row.occurrences,
                        row.total_swings,
                        row.occurrence_ratio.as_ref(),
                        row.swing_ratio.as_ref(),
                        row.total_rate.as_ref(),
                    )
                    {
                        p {
                            "Occurrences: {occurrences}; all skill swings: {total}; occurrence ratio: {number(ratio, true)}%; swing ratio: {number(swings, true)}%; skill encounter rate: {number(rate, false)}"
                        }
                    }
                    Evidence {
                        key: "{data.revision}-{index}",
                        report: report.clone(),
                        index,
                        selected,
                    }
                }
            }
        }
        if let Some(text) = log {
            div { class: "border-t border-border pt-2",
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: move |_| selected.set(None),
                    "Close source"
                }
                p { class: "whitespace-pre-wrap", "{text}" }
            }
        }
    }
}

#[component]
fn Evidence(
    report: SharedReport,
    index: usize,
    mut selected: Signal<Option<(usize, i32)>>,
) -> Element {
    let mut page = use_signal(|| 0_usize);
    let Some(row) = report.0.rows.get(index) else {
        return rsx! {};
    };
    let start = page().saturating_mul(20);
    rsx! {
        for action in row.evidence.iter().skip(start).take(20) {
            {
                let action = action.clone();
                let source = usize::try_from(action.source_index)
                    .ok()
                    .map(|i| (i, action.time_sorter));
                rsx! {
                    div { class: "border-t border-border pt-2",
                        p { "{action.time}: {action.attack} — {action.damage}" }
                        if source.is_none() {
                            p { "Source log unavailable or ambiguous." }
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: source.is_none(),
                            onclick: move |_| selected.set(source),
                            "Source log"
                        }
                    }
                }
            }
        }
        div { class: "flex gap-2",
            Button {
                variant: ButtonVariant::Outline,
                disabled: start == 0,
                onclick: move |_| page.set(page().saturating_sub(1)),
                "Previous"
            }
            Button {
                variant: ButtonVariant::Outline,
                disabled: start.saturating_add(20) >= row.evidence.len(),
                onclick: move |_| page.set(page().saturating_add(1)),
                "Next"
            }
        }
    }
}
