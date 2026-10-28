use super::{
    AppState,
    log_window::{ContextEndpoint, WindowAppearance},
};
use crate::components::{Accordion, Button, ButtonVariant};
use aftercast::{AvoidanceAction, AvoidanceNumber, AvoidanceReport, EncounterId};
use dioxus::prelude::*;
use std::sync::Arc;

#[cfg(test)]
mod tests;
#[derive(Clone, PartialEq)]
enum Scope {
    Combatant(String),
    Encounter,
}
#[derive(Clone)]
struct SharedReport(Arc<AvoidanceReport>);
impl PartialEq for SharedReport {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
#[derive(Default, Clone, PartialEq)]
struct Selection {
    source: Option<(usize, i32)>,
}
impl Selection {
    fn select(&mut self, row: &AvoidanceAction) {
        self.source = usize::try_from(row.source_index)
            .ok()
            .map(|index| (index, row.time_sorter));
    }
    fn clear(&mut self) {
        self.source = None;
    }
}
fn source_text(report: &AvoidanceReport, selected: Option<(usize, i32)>) -> Option<&str> {
    let (index, sorter) = selected?;
    let log = report.logs.get(index)?;
    (log.time_sorter == sorter && usize::try_from(log.source_index).ok() == Some(index))
        .then_some(log.text.as_str())
}
async fn load(
    endpoint: ContextEndpoint,
    scope: Scope,
) -> Result<Option<Arc<AvoidanceReport>>, String> {
    if !endpoint.valid() {
        return Err("Session ended".to_owned());
    }
    let result = match scope {
        Scope::Combatant(name) => {
            endpoint
                .handle
                .combatant_avoidance(endpoint.id, &name)
                .await
        }
        Scope::Encounter => endpoint.handle.encounter_avoidance(endpoint.id).await,
    }
    .map_err(|error| error.to_string());
    if !endpoint.valid() {
        return Err("Session changed".to_owned());
    }
    result.map(|report| report.map(Arc::new))
}
fn number(value: &AvoidanceNumber) -> String {
    match value {
        AvoidanceNumber::Finite(n) => format!("{n:.2}"),
        AvoidanceNumber::Special(s) => s.clone(),
    }
}

#[component]
pub(super) fn AvoidancePanel(id: EncounterId, combatant: Option<String>) -> Element {
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
            title: "Avoidance".into(),
            theme: app.read().settings.ui.theme.clone(),
            language: app.read().settings.ui.language.clone(),
        },
    };
    let scope = combatant.map_or(Scope::Encounter, Scope::Combatant);
    let title = match &scope {
        Scope::Combatant(name) => format!("Avoidance — {name}"),
        Scope::Encounter => "Encounter avoidance".to_owned(),
    };
    let mut selection = use_signal(Selection::default);
    let mut result = use_resource(move || {
        let endpoint = endpoint.clone();
        let scope = scope.clone();
        async move { load(endpoint, scope).await }
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
            header: rsx! { "{title}" },
            div { class: "p-4 space-y-3",
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: pending,
                    onclick: move |_| {
                        if result.finished() {
                            selection.write().clear();
                            result.restart();
                        }
                    },
                    "Refresh"
                }
                if pending {
                    Button {
                        variant: ButtonVariant::Outline,
                        onclick: move |_| {
                            selection.write().clear();
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
                            p { "Loading avoidance report…" }
                        },
                        Some(Err(error)) => rsx! {
                            p { role: "alert", "{error}" }
                        },
                        Some(Ok(None)) => rsx! {
                            p { "Selected encounter or combatant is unavailable." }
                        },
                        Some(Ok(Some(report))) => rsx! {
                            AvoidanceResults { report: SharedReport(report), selection }
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn AvoidanceResults(report: SharedReport, mut selection: Signal<Selection>) -> Element {
    let data = &report.0;
    let partial = matches!(data.summary.status, 2 | 3);
    let log = source_text(data, selection().source).map(str::to_owned);
    rsx! {
        if partial {
            p { "Import is incomplete or still running; some actions may be missing." }
        }
        if data.scope == "encounter" {
            p {
                "ACT filters outcome labels by ally names and YOU; overlapping names can share outcomes."
            }
        }
        p {
            "Retained logs: {data.logs.len()}; omitted: {data.dropped_logs}. Estimated damage is an estimate."
        }
        if data.categories.is_empty() {
            p { "No matching avoidance categories." }
        }
        for (category_index, category) in data.categories.iter().enumerate() {
            div { class: "border-t border-border pt-2",
                h3 { class: "font-semibold", "{category.name}" }
                p {
                    "Occurrences: {category.total_occurrences}; estimated damage: {number(&category.estimated_damage)}; estimated rate: {number(&category.estimated_rate)}"
                }
                if let (Some(swings), Some(hits), Some(percent)) = (
                    category.swings,
                    category.hits,
                    category.percent_swings.as_ref(),
                )
                {
                    p { "Swings: {swings}; hits: {hits}; occurrence percentage: {number(percent)}%" }
                }
                if category.outcomes.is_empty() {
                    p { "No avoidance outcomes." }
                }
                for (outcome_index, _) in category.outcomes.iter().enumerate() {
                    OutcomePanel {
                        key: "{report.0.revision}-{category_index}-{outcome_index}",
                        report: report.clone(),
                        category_index,
                        outcome_index,
                        selection,
                    }
                }
            }
        }
        if let Some(text) = log {
            div { class: "border-t border-border pt-2",
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: move |_| selection.write().clear(),
                    "Close source"
                }
                p { class: "whitespace-pre-wrap", "{text}" }
            }
        }
    }
}

#[component]
fn OutcomePanel(
    report: SharedReport,
    category_index: usize,
    outcome_index: usize,
    mut selection: Signal<Selection>,
) -> Element {
    let mut page = use_signal(|| 0_usize);
    let mut expanded = use_signal(|| None::<usize>);
    let Some(outcome) = report
        .0
        .categories
        .get(category_index)
        .and_then(|c| c.outcomes.get(outcome_index))
    else {
        return rsx! {};
    };
    let start = page().saturating_mul(20);
    let total = outcome.evidence.len();
    rsx! {
        Accordion {
            class: "mt-2 border border-border rounded-md".to_owned(),
            default_open: false,
            trigger_class: "px-3 py-2".to_owned(),
            header: rsx! { "{outcome.label} — {outcome.count}" },
            div { class: "p-3 space-y-2",
                p {
                    "Estimated damage: {number(&outcome.estimated_damage)}; estimated rate: {number(&outcome.estimated_rate)}"
                }
                if let (Some(nonhits), Some(swings)) = (
                    outcome.percent_non_hits.as_ref(),
                    outcome.percent_swings.as_ref(),
                )
                {
                    p { "Of non-hits: {number(nonhits)}%; of swings: {number(swings)}%" }
                }
                for (offset, entry) in outcome.evidence.iter().enumerate().skip(start).take(20) {
                    {
                        let row = entry.action.clone();
                        let missing = row.source_index < 0;
                        let fallback = entry.used_fallback;
                        let estimate = number(&entry.estimate);
                        rsx! {
                            div { class: "border-t border-border pt-2",
                                p {
                                    "{row.time}: {row.attacker} → {row.victim}, {row.attack}; estimate {estimate}; fallback: {fallback}"
                                }
                                if missing {
                                    p { "Source log unavailable or ambiguous." }
                                }
                                Button {
                                    variant: ButtonVariant::Outline,
                                    disabled: missing,
                                    onclick: move |_| selection.write().select(&row),
                                    "Source log"
                                }
                                Button {
                                    variant: ButtonVariant::Outline,
                                    onclick: move |_| expanded.set(Some(offset)),
                                    "View samples"
                                }
                            }
                        }
                    }
                }
                div { class: "flex gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: start == 0,
                        onclick: move |_| {
                            page.set(page().saturating_sub(1));
                            expanded.set(None);
                        },
                        "Previous"
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: start.saturating_add(20) >= total,
                        onclick: move |_| {
                            page.set(page().saturating_add(1));
                            expanded.set(None);
                        },
                        "Next"
                    }
                }
                if let Some(evidence_index) = expanded() {
                    SamplePanel {
                        key: "{evidence_index}",
                        report,
                        category_index,
                        outcome_index,
                        evidence_index,
                        selection,
                    }
                }
            }
        }
    }
}

#[component]
fn SamplePanel(
    report: SharedReport,
    category_index: usize,
    outcome_index: usize,
    evidence_index: usize,
    mut selection: Signal<Selection>,
) -> Element {
    let mut page = use_signal(|| 0_usize);
    let Some(entry) = report
        .0
        .categories
        .get(category_index)
        .and_then(|c| c.outcomes.get(outcome_index))
        .and_then(|o| o.evidence.get(evidence_index))
    else {
        return rsx! {};
    };
    let start = page().saturating_mul(20);
    let total = entry.samples.len();
    rsx! {
        div { class: "border border-border p-2",
            p {
                "Sample pool: {entry.sample_pool}; rows: {total}; fallback: {entry.used_fallback}; average available: {entry.has_sample_average}"
            }
            if total == 0 {
                p { "No sample rows." }
            }
            for row in entry.samples.iter().skip(start).take(20) {
                {
                    let row = row.clone();
                    let missing = row.source_index < 0;
                    rsx! {
                        div {
                            p { "{row.time}: {row.attacker} → {row.victim}, {row.attack}: {row.damage}" }
                            Button {
                                variant: ButtonVariant::Outline,
                                disabled: missing,
                                onclick: move |_| selection.write().select(&row),
                                "Sample source log"
                            }
                            if missing {
                                p { "Source log unavailable or ambiguous." }
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
                    "Previous samples"
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: start.saturating_add(20) >= total,
                    onclick: move |_| page.set(page().saturating_add(1)),
                    "Next samples"
                }
            }
        }
    }
}
