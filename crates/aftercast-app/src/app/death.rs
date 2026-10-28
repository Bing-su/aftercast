use super::{AppState, log_window::ContextEndpoint};
use crate::components::{Accordion, Button, ButtonVariant};
use aftercast::{DeathAnalysis, EncounterId, SourceLogLink, death_report};
use dioxus::prelude::*;

#[derive(Clone)]
struct EndpointProp(ContextEndpoint);
impl PartialEq for EndpointProp {
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id
            && self.0.generation == other.0.generation
            && std::sync::Arc::ptr_eq(&self.0.lifetime, &other.0.lifetime)
    }
}

#[cfg(test)]
mod tests;

pub(super) fn incomplete(status: aftercast::EncounterStatus) -> bool {
    matches!(
        status,
        aftercast::EncounterStatus::PartialImport | aftercast::EncounterStatus::Importing
    )
}

#[derive(Default, Clone, PartialEq)]
struct DeathSelection {
    selected: Option<usize>,
    source: Option<(u32, i32)>,
}
impl DeathSelection {
    fn select(&mut self, index: usize, total: usize) -> bool {
        if index >= total {
            return false;
        }
        self.selected = Some(index);
        self.source = None;
        true
    }
    fn select_source(&mut self, source: Option<(u32, i32)>) {
        self.source = source;
    }
    fn cancel_source(&mut self) {
        self.source = None;
    }
}

async fn load_analysis(
    endpoint: ContextEndpoint,
    name: String,
) -> Result<Option<DeathAnalysis>, String> {
    if !endpoint.valid() {
        return Err("Session ended".to_owned());
    }
    let result = endpoint
        .handle
        .death_analysis(endpoint.id, &name)
        .await
        .map_err(|e| e.to_string());
    if !endpoint.valid() {
        return Err("Session changed".to_owned());
    }
    result
}
async fn load_source(
    endpoint: ContextEndpoint,
    snapshot: aftercast::ActionSnapshot,
    offset: u32,
    sorter: i32,
) -> Result<String, String> {
    if !endpoint.valid() {
        return Err("Session ended".to_owned());
    }
    let result = endpoint
        .handle
        .action_snapshot_logs(snapshot, offset, 1)
        .await
        .map_err(|e| e.to_string());
    if !endpoint.valid() {
        return Err("Session changed".to_owned());
    }
    let row = result?
        .page
        .items
        .into_iter()
        .next()
        .ok_or("Log unavailable")?;
    if row.time_sorter != sorter || row.source != SourceLogLink::Retained(offset) {
        return Err("Log identity changed".to_owned());
    }
    Ok(row.text)
}

#[component]
pub(super) fn DeathPanel(id: EncounterId, combatant: String, partial: bool) -> Element {
    let app = use_context::<Signal<AppState>>();
    let Some(handle) = app.read().handle() else {
        return rsx! {};
    };
    let endpoint = ContextEndpoint {
        handle,
        id,
        generation: app.read().log_windows.generation(),
        lifetime: app.read().log_windows.clone(),
        appearance: super::log_window::WindowAppearance {
            title: combatant.clone(),
            theme: app.read().settings.ui.theme.clone(),
            language: app.read().settings.ui.language.clone(),
        },
    };
    let selection = use_signal(DeathSelection::default);
    let endpoint_load = endpoint.clone();
    let name = combatant.clone();
    let data = use_resource(move || {
        let endpoint = endpoint_load.clone();
        let name = name.clone();
        async move { load_analysis(endpoint, name).await }
    });
    let current = data.read().clone();
    rsx! {
        Accordion {
            class: "mt-5 border border-border rounded-md".to_owned(),
            default_open: false,
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! { "Death analysis — {combatant}" },
            div { class: "p-4 space-y-3",
                if partial {
                    p { "Import is incomplete or still running; some actions may be missing." }
                }
                match current {
                    None => rsx! {
                        p { "Loading death analysis…" }
                    },
                    Some(Err(error)) => rsx! {
                        p { role: "alert", "{error}" }
                    },
                    Some(Ok(None)) => rsx! {
                        p { "Combatant is no longer available." }
                    },
                    Some(Ok(Some(analysis))) => rsx! {
                        DeathResults { analysis, selection, endpoint: EndpointProp(endpoint) }
                    },
                }
            }
        }
    }
}

#[component]
fn DeathResults(
    analysis: DeathAnalysis,
    mut selection: Signal<DeathSelection>,
    endpoint: EndpointProp,
) -> Element {
    let index = selection()
        .selected
        .unwrap_or_else(|| analysis.deaths.len().saturating_sub(1));
    let death_count = analysis.deaths.len();
    let report = analysis
        .deaths
        .get(index)
        .and_then(|i| death_report(&analysis.actions, *i));
    rsx! {
        p { "Retained logs: {analysis.retained_logs}; omitted: {analysis.dropped_logs}." }
        if analysis.deaths.is_empty() {
            p { "No death markers found." }
        }
        for (i, action_index) in analysis.deaths.iter().enumerate() {
            {
                let time = super::encounter_context::format_time(
                    analysis
                        .actions
                        .get(*action_index)
                        .expect("death index generated from actions")
                        .action
                        .time,
                );
                rsx! {
                    Button {
                        variant: ButtonVariant::Outline,
                        onclick: move |_| {
                            selection.write().select(i, death_count);
                        },
                        "{time}"
                    }
                }
            }
        }
        if let Some(report) = report {
            {
                let damage_totals = report.cumulative_damage();
                let net_totals = report.chronological_net();
                let magnitude = report
                    .buckets
                    .iter()
                    .map(|b| b.damage.unsigned_abs().max(b.healing.unsigned_abs()))
                    .max()
                    .unwrap_or(1)
                    .max(1) as f64;
                rsx! {
                    div {
                        class: "space-y-1",
                        role: "img",
                        aria_label: "Chronological damage and healing magnitudes",
                        for i in (0..10).rev() {
                            {
                                let bucket = report.buckets.get(i).expect("bucket index 0..10");
                                let damage_width = (bucket.damage.unsigned_abs() as f64 / magnitude * 100.0)
                                    .round();
                                let healing_width = (bucket.healing.unsigned_abs() as f64 / magnitude * 100.0)
                                    .round();
                                let net = *net_totals.get(9 - i).expect("bucket index 0..10");
                                rsx! {
                                    div { class: "text-xs",
                                        "T-{i}: cumulative net {net}"
                                        div { style: "height: 8px; background-color: #c44; width: {damage_width}%;" }
                                        div { style: "height: 8px; background-color: #4a6; width: {healing_width}%;" }
                                    }
                                }
                            }
                        }
                    }
                    for i in 0..10 {
                        {
                            let bucket = report.buckets.get(i).expect("bucket index 0..10");
                            let d = bucket.damage;
                            let net = d.wrapping_sub(bucket.healing);
                            let total = *damage_totals.get(i).expect("bucket index 0..10");
                            rsx! {
                                p { "T-{i}: damage {d}, net {net}, cumulative damage {total}" }
                            }
                        }
                    }
                }
            }
            for i in (0..10).rev() {
                {
                    let bucket = report.buckets.get(i).expect("bucket index 0..10");
                    let damage = bucket.damage;
                    let healing = bucket.healing;
                    let net = damage.wrapping_sub(healing);
                    rsx! {
                        div { class: "border-t border-border pt-2",
                            p { "T-{i}: damage {damage}, healing {healing}, net {net}" }
                            for action_index in bucket.evidence.iter().copied() {
                                {
                                    let row = &analysis
                                        .actions
                                        .get(action_index)
                                        .expect("evidence index generated from actions")
                                        .action;
                                    let label = format!(
                                        "{} — {} — {}{}",
                                        row.attacker,
                                        row.attack,
                                        analysis
                                            .actions
                                            .get(action_index)
                                            .expect("evidence index generated from actions")
                                            .numeric_tooltip,
                                        match row.source {
                                            SourceLogLink::Missing => " (log unavailable)",
                                            SourceLogLink::Ambiguous => " (ambiguous log)",
                                            _ => "",
                                        },
                                    );
                                    let link = match row.source {
                                        SourceLogLink::Retained(n) => Some((n, row.time_sorter)),
                                        _ => None,
                                    };
                                    rsx! {
                                        Button {
                                            variant: ButtonVariant::Outline,
                                            disabled: link.is_none(),
                                            onclick: move |_| selection.write().select_source(link),
                                            "{label}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else if !analysis.deaths.is_empty() {
            p { role: "alert", "Death time is outside the supported range." }
        }
        if let Some((offset, sorter)) = selection().source {
            div { key: "{offset}-{sorter}",
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: move |_| selection.write().cancel_source(),
                    "Close source"
                }
                FrozenLog {
                    endpoint,
                    snapshot: analysis.snapshot,
                    offset,
                    sorter,
                }
            }
        }
    }
}

#[component]
fn FrozenLog(
    endpoint: EndpointProp,
    snapshot: aftercast::ActionSnapshot,
    offset: u32,
    sorter: i32,
) -> Element {
    let response = use_resource(move || {
        let endpoint = endpoint.0.clone();
        async move { load_source(endpoint, snapshot, offset, sorter).await }
    });
    match response.read().clone() {
        None => rsx! {
            p { "Loading source log…" }
        },
        Some(Ok(text)) => rsx! {
            p { class: "whitespace-pre-wrap", "{text}" }
        },
        Some(Err(error)) => rsx! {
            p { role: "alert", "{error}" }
        },
    }
}
