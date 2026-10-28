use super::{
    AppState,
    log_window::{ContextEndpoint, WindowAppearance},
};
use crate::components::{Accordion, Button, ButtonVariant, Input, Label};
use aftercast::{EncounterId, TimelineOptions, TimelineReport};
use dioxus::prelude::*;
use std::sync::Arc;

#[cfg(test)]
mod tests;

#[derive(Clone)]
struct State {
    draft: TimelineOptions,
    window_text: String,
    pending: bool,
    token: u64,
    report: Option<Arc<TimelineReport>>,
    error: Option<String>,
    visible: Vec<bool>,
    start: usize,
    zoom: usize,
    share_scale: bool,
    point: Option<usize>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            draft: TimelineOptions::default(),
            window_text: "10".into(),
            pending: false,
            token: 0,
            report: None,
            error: None,
            visible: Vec::new(),
            start: 0,
            zoom: 50,
            share_scale: false,
            point: None,
        }
    }
}
impl State {
    fn begin(&mut self) -> Option<(u64, TimelineOptions)> {
        if self.pending {
            return None;
        }
        let Ok(window) = self.window_text.parse::<u16>() else {
            self.error = Some("Sample size must be 1–600 seconds.".into());
            return None;
        };
        if !(1..=600).contains(&window) {
            self.error = Some("Sample size must be 1–600 seconds.".into());
            return None;
        }
        self.token = self.token.wrapping_add(1);
        self.pending = true;
        self.error = None;
        Some((
            self.token,
            TimelineOptions {
                window,
                ..self.draft
            },
        ))
    }
    fn cancel(&mut self) {
        self.token = self.token.wrapping_add(1);
        self.pending = false;
    }
    fn receive(&mut self, token: u64, result: Result<Option<Arc<TimelineReport>>, String>) {
        if !self.pending || self.token != token {
            return;
        }
        self.pending = false;
        match result {
            Ok(Some(report)) => {
                self.visible = vec![true; report.rows.len()];
                self.start = 0;
                self.point = None;
                self.report = Some(report);
                self.error = None;
            }
            Ok(None) => self.error = Some("Selected encounter is unavailable.".into()),
            Err(error) => self.error = Some(error),
        }
    }
}
async fn load(
    endpoint: ContextEndpoint,
    options: TimelineOptions,
) -> Result<Option<Arc<TimelineReport>>, String> {
    if !endpoint.valid() {
        return Err("Session ended".into());
    }
    let result = endpoint
        .handle
        .timeline(endpoint.id, options)
        .await
        .map_err(|error| error.to_string());
    if !endpoint.valid() {
        return Err("Session changed".into());
    }
    result.map(|report| report.map(Arc::new))
}
fn point_time(report: &TimelineReport, point: usize) -> Option<(f64, f64)> {
    if point >= report.points as usize {
        return None;
    }
    let width = if report.options.rolling {
        1.0
    } else {
        f64::from(report.options.window)
    };
    Some(((point + 1) as f64 * width, width))
}
fn logs_at(report: &TimelineReport, time: f64, width: f64) -> Vec<usize> {
    report
        .logs
        .iter()
        .enumerate()
        .filter_map(|(i, log)| (log.seconds >= time && log.seconds < time + width).then_some(i))
        .collect()
}
fn graph_axis(maxima: impl Iterator<Item = i64>) -> f64 {
    let maximum = maxima.max().unwrap_or(1).max(1) as f64;
    let mut axis = 10_f64.powi((maximum as i64).to_string().len() as i32);
    while (axis / 2.0).floor() > maximum {
        axis = (axis / 2.0).floor();
    }
    if axis / 1.25 > maximum {
        axis = (axis / 1.25).round_ties_even();
    }
    axis.max(1.0)
}
fn graph_points(values: &[i64], start: usize, end: usize, axis: f64) -> String {
    let Some(values) = values.get(start..end) else {
        return String::new();
    };
    let denominator = values.len().saturating_sub(1).max(1) as f64;
    values
        .iter()
        .enumerate()
        .map(|(i, value)| {
            format!(
                "{:.2},{:.2}",
                i as f64 * 600.0 / denominator,
                (50.0 - (*value as f64 / axis) * 45.0).clamp(0.0, 100.0)
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[component]
pub(super) fn TimelinePanel(id: EncounterId) -> Element {
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
            title: "Timeline logs".into(),
            theme: app.read().settings.ui.theme.clone(),
            language: app.read().settings.ui.language.clone(),
        },
    };
    let mut state = use_signal(State::default);
    let current = state.read().clone();
    let run_endpoint = endpoint.clone();
    rsx! {
        Accordion {
            class: "mt-5 border border-border rounded-md".to_owned(),
            default_open: false,
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! { "Encounter timeline" },
            div { class: "p-4 space-y-3",
                p {
                    "Configure the timeline, then generate a frozen view. Values use ACT's full sample-window divisor."
                }
                div { class: "flex flex-wrap gap-2",
                    for (group, label) in [
                        (0, "Individual"),
                        (1, "Allied types / enemies"),
                        (2, "Us / them"),
                        (3, "Us / individual enemies"),
                    ]
                    {
                        Button {
                            variant: ButtonVariant::Outline,
                            onclick: move |_| state.write().draft.grouping = group,
                            "{label} "
                            {if current.draft.grouping == group { "✓" } else { "" }}
                        }
                    }
                }
                div { class: "flex flex-wrap gap-2",
                    for (metric, label) in [
                        (1, "Outgoing damage"),
                        (2, "Incoming damage"),
                        (4, "Outgoing healing"),
                        (8, "Incoming healing"),
                    ]
                    {
                        Button {
                            variant: ButtonVariant::Outline,
                            onclick: move |_| state.write().draft.metrics ^= metric,
                            "{label} "
                            {if current.draft.metrics & metric != 0 { "✓" } else { "" }}
                        }
                    }
                }
                div { class: "flex flex-wrap items-center gap-2",
                    Label { html_for: "timeline-window".to_string(), "Sample seconds" }
                    Input {
                        id: "timeline-window".to_string(),
                        value: current.window_text.clone(),
                        class: "max-w-28".to_string(),
                        oninput: move |event: FormEvent| state.write().window_text = event.value(),
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        onclick: move |_| {
                            let rolling = state.read().draft.rolling;
                            state.write().draft.rolling = !rolling;
                        },
                        {if current.draft.rolling { "Rolling" } else { "Segments" }}
                    }
                    Button {
                        disabled: current.pending,
                        onclick: move |_| {
                            let request = state.write().begin();
                            if let Some((token, options)) = request {
                                let endpoint = run_endpoint.clone();
                                spawn(async move {
                                    let result = load(endpoint, options).await;
                                    state.write().receive(token, result);
                                });
                            }
                        },
                        "Generate"
                    }
                    if current.pending {
                        Button {
                            variant: ButtonVariant::Outline,
                            onclick: move |_| state.write().cancel(),
                            "Cancel"
                        }
                    }
                }
                if let Some(error) = current.error.as_ref() {
                    p { role: "alert", "{error}" }
                }
                if let Some(report) = current.report.as_ref() {
                    {
                        let report = report.clone();
                        let endpoint = endpoint.clone();
                        rsx! {
                            TimelineResults {
                                key: "{report.revision}",
                                report: SharedReport(report),
                                endpoint: SharedEndpoint(endpoint),
                                state,
                            }
                        }
                    }
                }
            }
        }
    }
}
#[derive(Clone)]
struct SharedReport(Arc<TimelineReport>);
impl PartialEq for SharedReport {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
#[derive(Clone)]
struct SharedEndpoint(ContextEndpoint);
impl PartialEq for SharedEndpoint {
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id
            && self.0.generation == other.0.generation
            && Arc::ptr_eq(&self.0.lifetime, &other.0.lifetime)
    }
}

#[component]
fn TimelineResults(
    report: SharedReport,
    endpoint: SharedEndpoint,
    mut state: Signal<State>,
) -> Element {
    let current = state.read().clone();
    let data = &report.0;
    let total = data.points as usize;
    let start = current.start.min(total);
    let end = start.saturating_add(current.zoom).min(total);
    let selected = current.point.and_then(|point| point_time(data, point));
    rsx! {
        if matches!(data.summary.status, 2 | 3) {
            p { "Import is incomplete or still running; some actions may be missing." }
        }
        p {
            "Active snapshot: grouping {data.options.grouping}, {data.options.window}s, "
            {if data.options.rolling { "rolling" } else { "segments" }}
            ", metrics {data.options.metrics}. Omitted logs: {data.dropped_logs}."
        }
        if total == 0 {
            p { "No displayed timeline points." }
        }
        div { class: "flex flex-wrap gap-2",
            Button {
                variant: ButtonVariant::Outline,
                disabled: start == 0,
                onclick: move |_| {
                    let zoom = state.read().zoom;
                    state.write().start = start.saturating_sub(zoom);
                },
                "Previous"
            }
            Button {
                variant: ButtonVariant::Outline,
                disabled: end >= total,
                onclick: move |_| state.write().start = end,
                "Next"
            }
            Button {
                variant: ButtonVariant::Outline,
                onclick: move |_| {
                    let shared = state.read().share_scale;
                    state.write().share_scale = !shared;
                },
                {if current.share_scale { "Shared metric scale" } else { "Individual scale" }}
            }
            for zoom in [25_usize, 50, 100] {
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: move |_| {
                        let mut state = state.write();
                        state.zoom = zoom;
                        state.start = 0;
                    },
                    "{zoom} points"
                }
            }
            p { "Points {start}–{end} of {total}" }
        }
        for (index, row) in data.rows.iter().enumerate() {
            {
                let visible = current.visible.get(index).copied().unwrap_or(false);
                rsx! {
                    div { class: "border-t border-border pt-2",
                        Button {
                            variant: ButtonVariant::Outline,
                            onclick: move |_| {
                                let mut state = state.write();
                                if let Some(value) = state.visible.get_mut(index) {
                                    *value = !*value;
                                }
                            },
                            "{row.name} "
                            {if visible { "✓" } else { "" }}
                        }
                        if visible {
                            for series in &row.series {
                                {
                                    let axis = if current.share_scale {
                                        graph_axis(
                                            data
                                                .rows
                                                .iter()
                                                .enumerate()
                                                .filter(|(i, _)| current.visible.get(*i).copied().unwrap_or(false))
                                                .flat_map(|(_, row)| {
                                                    row
                                                        .series
                                                        .iter()
                                                        .filter(|candidate| candidate.metric == series.metric)
                                                        .map(|candidate| candidate.axis_maximum)
                                                }),
                                        )
                                    } else {
                                        graph_axis(std::iter::once(series.axis_maximum))
                                    };
                                    rsx! {
                                        p {
                                            {
                                                match series.metric {
                                                    0 => "Outgoing damage",
                                                    1 => "Incoming damage",
                                                    2 => "Outgoing healing",
                                                    _ => "Incoming healing",
                                                }
                                            }
                                        }
                                        if let Some(point) = current.point {
                                            if let Some(value) = series.rates.get(point) {
                                                p { "Selected point rate: {value}" }
                                            }
                                        }
                                        svg {
                                            view_box: "0 0 600 100",
                                            width: "100%",
                                            height: "100",
                                            role: "img",
                                            title { "Timeline values" }
                                            polyline {
                                                points: "{graph_points(&series.raw, start, end, axis)}",
                                                fill: "none",
                                                stroke: "currentColor",
                                                stroke_width: "2",
                                            }
                                        }
                                        p { "Raw sum scale maximum: {axis}" }
                                    }
                                }
                            }
                            p { "Markers: K = kill, D = death, S = timer spell." }
                            if let Some(point) = current.point {
                                if let Some(marker) = row.markers.get(point) {
                                    p { "Selected point marker bits: {marker} (K=1, D=2, S=4)." }
                                }
                            }
                        }
                    }
                }
            }
        }
        if total > 0 {
            div { class: "flex flex-wrap gap-2",
                for point in start..end {
                    {
                        let seconds = if data.options.rolling {
                            point + 1
                        } else {
                            (point + 1) * usize::from(data.options.window)
                        };
                        rsx! {
                            Button {
                                variant: ButtonVariant::Outline,
                                onclick: move |_| state.write().point = Some(point),
                                "+{seconds}s"
                            }
                        }
                    }
                }
            }
        }
        if let Some((time, width)) = selected {
            {
                let endpoint = endpoint.0.clone();
                let report = report.0.clone();
                let end_time = time + width;
                rsx! {
                    p { "Selected range: +{time}s to +{end_time}s (end excluded)." }
                    Button {
                        variant: ButtonVariant::Outline,
                        onclick: move |_| {
                            if let Some(pending) = log_window::open(
                                endpoint.clone(),
                                report.clone(),
                                time,
                                width,
                            ) {
                                spawn(async move {
                                    if pending.try_resolve().await.is_err() {
                                        tracing::error!("failed to open timeline logs");
                                    }
                                });
                            }
                        },
                        "Open logs for range"
                    }
                }
            }
        }
    }
}
mod log_window;
