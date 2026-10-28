use super::{
    AppState, Phase,
    log_window::{ContextEndpoint, WindowAppearance},
};
use crate::components::{Accordion, Button, ButtonVariant};
use aftercast::{
    EncounterId, RecastCalculation, RecastRequest, RecastSnapshot, recast_calculate,
    recast_initial_selection,
};
use dioxus::prelude::*;
use std::sync::Arc;

#[cfg(test)]
mod tests;

#[derive(Clone, PartialEq, Eq)]
struct Choice {
    category: String,
    attack: String,
    outgoing: bool,
}
#[derive(Clone)]
struct State {
    choice: Option<Choice>,
    restrict: bool,
    modifier: bool,
    active_restrict: bool,
    token: u64,
    pending: bool,
    report: Option<Arc<RecastSnapshot>>,
    checked: Vec<bool>,
    error: Option<String>,
    page: usize,
    histogram_page: usize,
    interval_page: usize,
    source: Option<(usize, i32)>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            choice: None,
            restrict: true,
            modifier: false,
            active_restrict: true,
            token: 0,
            pending: false,
            report: None,
            checked: Vec::new(),
            error: None,
            page: 0,
            histogram_page: 0,
            interval_page: 0,
            source: None,
        }
    }
}
impl State {
    fn choose(&mut self, choice: Choice) {
        self.cancel();
        self.choice = Some(choice);
        self.report = None;
        self.checked.clear();
        self.source = None;
        self.page = 0;
        self.error = None;
    }
    fn begin(&mut self, combatant: &str) -> Option<(u64, RecastRequest, bool)> {
        if self.pending {
            return None;
        }
        let choice = self.choice.as_ref()?;
        let request = RecastRequest {
            combatant: combatant.to_owned(),
            category: choice.category.clone(),
            attack: choice.attack.clone(),
            outgoing: choice.outgoing,
            modifier: self.modifier,
        };
        self.token = self.token.wrapping_add(1);
        self.pending = true;
        self.error = None;
        Some((self.token, request, self.restrict))
    }
    fn cancel(&mut self) {
        self.token = self.token.wrapping_add(1);
        self.pending = false;
    }
    fn receive(
        &mut self,
        token: u64,
        restrict: bool,
        result: Result<Option<Arc<RecastSnapshot>>, String>,
    ) {
        if !self.pending || self.token != token {
            return;
        }
        self.pending = false;
        match result {
            Ok(Some(report)) => match recast_initial_selection(&report.times(), restrict) {
                Ok(checked) => {
                    self.active_restrict = restrict;
                    self.checked = checked;
                    self.report = Some(report);
                    self.page = 0;
                    self.histogram_page = 0;
                    self.interval_page = 0;
                    self.source = None;
                    self.error = None;
                }
                Err(error) => self.error = Some(error.to_string()),
            },
            Ok(None) => self.error = Some("Selected attack is unavailable.".into()),
            Err(error) => self.error = Some(error),
        }
    }
    fn calculate(&self) -> Result<Option<RecastCalculation>, String> {
        let Some(report) = &self.report else {
            return Ok(None);
        };
        recast_calculate(
            &report.times(),
            &self.checked,
            self.active_restrict,
            report.request.modifier,
        )
        .map(Some)
        .map_err(|error| error.to_string())
    }
}
async fn load(
    endpoint: ContextEndpoint,
    request: RecastRequest,
) -> Result<Option<Arc<RecastSnapshot>>, String> {
    super::log_window::guard(&endpoint, async {
        endpoint
            .handle
            .recast_snapshot(endpoint.id, &request)
            .await
            .map_err(|error| error.to_string())
            .map(|report| report.map(Arc::new))
    })
    .await
}
fn modifier_label(modified: Option<bool>, requested: bool) -> &'static str {
    match (requested, modified) {
        (false, _) => "off",
        (true, None) => "no matching modifier attack",
        (true, Some(true)) => "applied",
        (true, Some(false)) => "not applied",
    }
}
fn source_text(report: &RecastSnapshot, source: Option<(usize, i32)>) -> Option<&str> {
    let (index, sorter) = source?;
    let log = report.logs.get(index)?;
    (log.time_sorter == sorter && usize::try_from(log.source_index).ok() == Some(index))
        .then_some(log.text.as_str())
}
fn choices(app: &AppState, combatant: &str) -> Vec<Choice> {
    app.attacks
        .iter()
        .filter(|row| {
            row.combatant == combatant
                && app.category.as_ref().is_none_or(|(name, outgoing)| {
                    row.category == *name && row.outgoing == *outgoing
                })
        })
        .map(|row| Choice {
            category: row.category.clone(),
            attack: row.attack.clone(),
            outgoing: row.outgoing,
        })
        .collect()
}

#[component]
pub(super) fn RecastPanel(id: EncounterId, combatant: String) -> Element {
    let app = use_context::<Signal<AppState>>();
    let Some(handle) = app.read().handle() else {
        return rsx! {};
    };
    let available = choices(&app.read(), &combatant);
    let endpoint = ContextEndpoint {
        handle,
        id,
        generation: app.read().log_windows.generation(),
        lifetime: app.read().log_windows.clone(),
        appearance: WindowAppearance {
            title: "Recast evidence".into(),
            theme: app.read().settings.ui.theme.clone(),
            language: app.read().settings.ui.language.clone(),
        },
    };
    let mut state = use_signal(State::default);
    let mut choice_page = use_signal(|| 0_usize);
    let current = state.read().clone();
    let start = choice_page().saturating_mul(20);
    let name = combatant.clone();
    let run_endpoint = endpoint.clone();
    rsx! {
        Accordion {
            class: "mt-5 border border-border rounded-md".to_owned(),
            default_open: false,
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! { "Recast analysis — {combatant}" },
            div { class: "p-4 space-y-3",
                p {
                    "Choose an exact category and attack, then calculate intervals between checked actions."
                }
                if available.is_empty() {
                    p { "No attack categories are available." }
                }
                for choice in available.iter().skip(start).take(20) {
                    {
                        let choice = choice.clone();
                        let direction = if choice.outgoing { "outgoing" } else { "incoming" };
                        let label = format!("{} / {} ({direction})", choice.category, choice.attack);
                        rsx! {
                            Button {
                                variant: ButtonVariant::Outline,
                                onclick: move |_| state.write().choose(choice.clone()),
                                "{label}"
                            }
                        }
                    }
                }
                div { class: "flex gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: start == 0,
                        onclick: move |_| choice_page.set(choice_page().saturating_sub(1)),
                        "Previous attacks"
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: start.saturating_add(20) >= available.len(),
                        onclick: move |_| choice_page.set(choice_page().saturating_add(1)),
                        "Next attacks"
                    }
                }
                if let Some(choice) = current.choice.as_ref() {
                    p { "Selected: {choice.category} / {choice.attack}" }
                }
                div { class: "flex flex-wrap gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        onclick: move |_| {
                            let value = state.read().restrict;
                            state.write().restrict = !value;
                        },
                        {
                            if current.restrict {
                                "Short-delay restriction on"
                            } else {
                                "Short-delay restriction off"
                            }
                        }
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        onclick: move |_| {
                            let value = state.read().modifier;
                            state.write().modifier = !value;
                        },
                        {
                            if current.modifier {
                                "Traumatic Swipe adjustment on"
                            } else {
                                "Traumatic Swipe adjustment off"
                            }
                        }
                    }
                    Button {
                        disabled: current.pending || current.choice.is_none(),
                        onclick: move |_| {
                            let next = state.write().begin(&name);
                            if let Some((token, request, restrict)) = next {
                                let endpoint = run_endpoint.clone();
                                spawn(async move {
                                    let result = load(endpoint, request).await;
                                    state.write().receive(token, restrict, result);
                                });
                            }
                        },
                        "Calculate"
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
                        rsx! {
                            RecastResults {
                                key: "{report.revision}",
                                report: SharedReport(report),
                                endpoint: SharedEndpoint(endpoint.clone()),
                                state,
                                app,
                            }
                        }
                    }
                }
            }
        }
    }
}
#[derive(Clone)]
struct SharedReport(Arc<RecastSnapshot>);
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
fn RecastResults(
    report: SharedReport,
    endpoint: SharedEndpoint,
    mut state: Signal<State>,
    mut app: Signal<AppState>,
) -> Element {
    let current = state.read().clone();
    let data = &report.0;
    let calculation = current.calculate();
    let start = current.page.saturating_mul(20);
    let source = source_text(data, current.source).map(str::to_owned);
    let histogram_start = current.histogram_page.saturating_mul(20);
    let interval_start = current.interval_page.saturating_mul(20);
    let timer_name = data.attack_name.clone().unwrap_or_default();
    let timer_attacker = data.first_attacker.clone().unwrap_or_default();
    let timer_modifier = data.request.modifier;
    let timer_endpoint = endpoint.0.clone();
    rsx! {
        if matches!(data.summary.status, 2 | 3) {
            p { "Import is incomplete or still running; some actions may be missing." }
        }
        p {
            "Active snapshot: {data.request.category} / {data.request.attack}; short-delay restriction: {current.active_restrict}; Traumatic Swipe adjustment: {data.request.modifier}. Omitted logs: {data.dropped_logs}."
        }
        if data.rows.is_empty() {
            p { "No recorded actions; no recast can be estimated." }
        }
        for (index, row) in data.rows.iter().enumerate().skip(start).take(20) {
            {
                let included = current.checked.get(index).copied().unwrap_or(false);
                let source = usize::try_from(row.action.source_index)
                    .ok()
                    .map(|i| (i, row.action.time_sorter));
                let modifier_source = row
                    .modifier_evidence
                    .as_ref()
                    .and_then(|action| {
                        usize::try_from(action.source_index)
                            .ok()
                            .map(|i| (i, action.time_sorter))
                    });
                rsx! {
                    div { class: "border-t border-border pt-2",
                        p {
                            "{row.action.time}: {row.action.attacker} → {row.action.victim}; {row.action.damage}; next interval adjustment: {modifier_label(row.modified, data.request.modifier)}"
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            onclick: move |_| {
                                if let Some(value) = state.write().checked.get_mut(index) {
                                    *value = !*value;
                                }
                            },
                            {if included { "Included" } else { "Excluded" }}
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: source.is_none(),
                            onclick: move |_| state.write().source = source,
                            "Action source log"
                        }
                        if row.action.source_index < 0 {
                            p { "Action source log unavailable or ambiguous." }
                        }
                        if row.modifier_evidence.is_some() {
                            Button {
                                variant: ButtonVariant::Outline,
                                disabled: modifier_source.is_none(),
                                onclick: move |_| state.write().source = modifier_source,
                                "Modifier source log"
                            }
                            if modifier_source.is_none() {
                                p { "Modifier source log unavailable or ambiguous." }
                            }
                        }
                    }
                }
            }
        }
        div { class: "flex gap-2",
            Button {
                variant: ButtonVariant::Outline,
                disabled: start == 0,
                onclick: move |_| state.write().page = current.page.saturating_sub(1),
                "Previous actions"
            }
            Button {
                variant: ButtonVariant::Outline,
                disabled: start.saturating_add(20) >= data.rows.len(),
                onclick: move |_| state.write().page = current.page.saturating_add(1),
                "Next actions"
            }
        }
        if let Some(text) = source {
            div {
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: move |_| state.write().source = None,
                    "Close source"
                }
                p { class: "whitespace-pre-wrap", "{text}" }
            }
        }
        match calculation {
            Err(error) => rsx! {
                p { role: "alert", "{error}" }
            },
            Ok(None) => rsx! {},
            Ok(Some(result)) => {
                let duration = result.minimum.unwrap_or(0);
                let max_count = result
                    .histogram
                    .iter()
                    .map(|(_, count)| *count)
                    .max()
                    .unwrap_or(1);
                rsx! {
                    if let Some(minimum) = result.minimum {
                        p { "Proposed timer: {minimum} seconds (minimum usable interval, not a median)." }
                    } else {
                        p { "Insufficient usable intervals; ACT's timer handoff uses 0 seconds." }
                    }
                    p {
                        "Checked adjacent intervals: {result.intervals.len()}; histogram excludes intervals ≤1s and, with restriction, ≥121s."
                    }
                    for (index, delay) in result.intervals.iter().enumerate().skip(interval_start).take(20) {
                        p { "Interval {index}: {delay}s" }
                    }
                    div { class: "flex gap-2",
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: interval_start == 0,
                            onclick: move |_| state.write().interval_page = current.interval_page.saturating_sub(1),
                            "Previous checked intervals"
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: interval_start.saturating_add(20) >= result.intervals.len(),
                            onclick: move |_| state.write().interval_page = current.interval_page.saturating_add(1),
                            "Next checked intervals"
                        }
                    }
                    if result.histogram.is_empty() {
                        p { "No intervals in the histogram." }
                    }
                    for (delay, count) in result.histogram.iter().skip(histogram_start).take(20) {
                        {
                            let width = f64::from(*count) * 300.0 / f64::from(max_count);
                            rsx! {
                                div {
                                    p { "{delay}s: {count}" }
                                    svg {
                                        view_box: "0 0 300 16",
                                        width: "100%",
                                        height: "16",
                                        role: "img",
                                        title { "Interval frequency" }
                                        rect {
                                            x: "0",
                                            y: "1",
                                            width: "{width}",
                                            height: "14",
                                            fill: "currentColor",
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "flex gap-2",
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: histogram_start == 0,
                            onclick: move |_| state.write().histogram_page = current.histogram_page.saturating_sub(1),
                            "Previous intervals"
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: histogram_start.saturating_add(20) >= result.histogram.len(),
                            onclick: move |_| state.write().histogram_page = current.histogram_page.saturating_add(1),
                            "Next intervals"
                        }
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: data.first_attacker.is_none() || app.read().phase != Phase::Ready,
                        onclick: move |_| {
                            if !timer_endpoint.valid() {
                                state.write().error = Some("Session ended or changed.".into());
                                return;
                            }
                            let mut app = app.write();
                            match app
                                .timers
                                .receive_recast_draft(&timer_name, &timer_attacker, duration, timer_modifier)
                            {
                                Ok(()) => app.screen = "timers",
                                Err(error) => state.write().error = Some(error.into()),
                            }
                        },
                        "Send to timer draft"
                    }
                }
            }
        }
    }
}
