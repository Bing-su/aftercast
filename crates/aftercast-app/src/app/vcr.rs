use super::{
    AppState,
    log_window::{ContextEndpoint, WindowAppearance, guard},
};
use crate::components::{Accordion, Button, ButtonVariant, Input, Label};
use aftercast::{EncounterId, VcrClock, VcrFrame, VcrMode, VcrSnapshot, vcr_frame};
use dioxus::prelude::*;
use std::sync::Arc;

#[cfg(test)]
mod tests;

#[derive(Clone)]
struct State {
    snapshot: Option<Arc<VcrSnapshot>>,
    clock: VcrClock,
    selected: Vec<String>,
    lookback: String,
    seek: String,
    persistent: bool,
    pending: bool,
    token: u64,
    error: Option<String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            snapshot: None,
            clock: VcrClock::new(0),
            selected: Vec::new(),
            lookback: "1".into(),
            seek: "0".into(),
            persistent: false,
            pending: false,
            token: 0,
            error: None,
        }
    }
}
impl State {
    fn begin(&mut self) -> Option<u64> {
        if self.pending {
            return None;
        }
        self.token = self.token.wrapping_add(1);
        self.pending = true;
        self.error = None;
        Some(self.token)
    }
    fn cancel(&mut self) {
        self.token = self.token.wrapping_add(1);
        self.pending = false;
        self.clock.mode = VcrMode::Paused;
        self.clock.linked = false;
    }
    fn receive(
        &mut self,
        token: u64,
        result: Result<Option<Arc<VcrSnapshot>>, String>,
        follow: bool,
    ) {
        if !self.pending || self.token != token {
            return;
        }
        self.pending = false;
        match result {
            Ok(Some(snapshot)) => {
                if follow {
                    if self.lookback.parse::<u32>().is_ok_and(|value| value < 1) {
                        self.lookback = "1".into();
                    }
                    self.selected
                        .retain(|name| snapshot.categories.contains(name));
                    self.clock.duration = snapshot.duration;
                    self.clock.position = snapshot.duration;
                } else {
                    self.selected = snapshot.categories.clone();
                    self.clock = VcrClock::new(snapshot.duration);
                }
                self.seek = self.clock.position.to_string();
                self.snapshot = Some(snapshot);
                self.error = None;
            }
            Ok(None) => {
                self.clock.linked = false;
                self.clock.mode = VcrMode::Paused;
                self.error = Some("Encounter is unavailable.".into());
            }
            Err(error) => {
                self.clock.linked = false;
                self.clock.mode = VcrMode::Paused;
                self.error = Some(error);
            }
        }
    }
}
async fn load(
    endpoint: &ContextEndpoint,
    follow: bool,
) -> Result<Option<Arc<VcrSnapshot>>, String> {
    guard(endpoint, async {
        let id = if follow {
            let Some(detail) = endpoint
                .handle
                .live_combat()
                .await
                .map_err(|e| e.to_string())?
            else {
                return Ok(None);
            };
            detail.summary.id
        } else {
            endpoint.id
        };
        endpoint
            .handle
            .vcr_snapshot(id)
            .await
            .map(|s| s.map(Arc::new))
            .map_err(|e| e.to_string())
    })
    .await
}
fn frame(snapshot: &VcrSnapshot, state: &State) -> Result<VcrFrame, String> {
    let lookback = state
        .lookback
        .parse::<u32>()
        .map_err(|_error| "Lookback must be 0–30 seconds.".to_string())?;
    vcr_frame(
        snapshot,
        state.clock.position,
        lookback,
        &state.selected,
        state.persistent,
        true,
    )
    .map_err(|error| error.to_string())
}

fn position(index: usize, count: usize) -> (f64, f64) {
    let angle = std::f64::consts::TAU * index as f64 / count.max(1) as f64;
    (300.0 + 250.0 * angle.cos(), 300.0 + 250.0 * angle.sin())
}

#[component]
pub(super) fn VcrPanel(id: EncounterId) -> Element {
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
            title: "VCR".into(),
            theme: app.read().settings.ui.theme.clone(),
            language: app.read().settings.ui.language.clone(),
        },
    };
    let mut state = use_signal(State::default);
    use_future({
        let endpoint = endpoint.clone();
        move || {
            let endpoint = endpoint.clone();
            async move {
                loop {
                    dioxus_sdk::time::sleep(std::time::Duration::from_millis(250)).await;
                    if !endpoint.valid() {
                        break;
                    }
                    let refresh = {
                        let mut s = state.write();
                        if s.snapshot.is_none() || s.pending {
                            false
                        } else {
                            let previous = s.clock.position;
                            let refresh = s.clock.tick();
                            if previous != s.clock.position {
                                s.seek = s.clock.position.to_string();
                            }
                            refresh
                        }
                    };
                    if refresh {
                        let token = state.write().begin();
                        if let Some(token) = token {
                            let result = load(&endpoint, true).await;
                            state.write().receive(token, result, true);
                        }
                    }
                }
            }
        }
    });
    let current = state.read().clone();
    let open_endpoint = endpoint.clone();
    rsx! {
        Accordion {
            class: "mt-5 border border-border rounded-md".to_owned(),
            default_open: false,
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! { "Encounter VCR" },
            div { class: "p-4 space-y-3",
                p {
                    "Replay a frozen encounter graph. This does not reparse logs or run triggers. Health is inferred; relationships are not world positions."
                }
                div { class: "flex flex-wrap gap-2",
                    Button {
                        disabled: current.pending,
                        onclick: move |_| {
                            let request = state.write().begin();
                            if let Some(token) = request {
                                let endpoint = open_endpoint.clone();
                                spawn(async move {
                                    let result = load(&endpoint, false).await;
                                    state.write().receive(token, result, false);
                                });
                            }
                        },
                        "Capture selected encounter"
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
                if let Some(snapshot) = current.snapshot.as_ref() {
                    p {
                        "{snapshot.summary.zone} / {snapshot.summary.title} · {current.clock.position} / {snapshot.duration} s · {current.clock.mode:?} · omitted logs: {snapshot.dropped_logs} · status: {snapshot.summary.status}"
                    }
                    div { class: "flex flex-wrap gap-2",
                        for (mode, label) in [
                            (VcrMode::Rewind, "Rewind 4×"),
                            (VcrMode::Play, "Play 1×"),
                            (VcrMode::Paused, "Pause"),
                            (VcrMode::FastForward, "Fast forward 4×"),
                        ]
                        {
                            Button {
                                variant: ButtonVariant::Outline,
                                onclick: move |_| {
                                    let mut s = state.write();
                                    if s.pending {
                                        s.cancel();
                                    }
                                    s.clock.mode = mode;
                                    if mode == VcrMode::Paused {
                                        s.clock.linked = false;
                                    }
                                },
                                "{label}"
                            }
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            onclick: move |_| {
                                let mut s = state.write();
                                s.cancel();
                                s.clock.seek(0);
                                s.seek = "0".into();
                            },
                            "Beginning"
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            onclick: move |_| {
                                let mut s = state.write();
                                s.cancel();
                                let end = s.clock.duration;
                                s.clock.seek(end);
                                s.seek = end.to_string();
                            },
                            "End"
                        }
                        Button {
                            disabled: current.pending,
                            variant: ButtonVariant::Outline,
                            onclick: move |_| {
                                let mut s = state.write();
                                s.clock.linked = true;
                                s.clock.mode = VcrMode::Play;
                            },
                            {if current.clock.linked { "Following live ✓" } else { "Follow live" }}
                        }
                    }
                    div { class: "flex flex-wrap items-center gap-2",
                        Label { html_for: "vcr-seek".to_string(), "Seek seconds" }
                        Input {
                            id: "vcr-seek".to_string(),
                            value: current.seek.clone(),
                            class: "max-w-28".to_string(),
                            oninput: move |event: FormEvent| state.write().seek = event.value(),
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            onclick: move |_| {
                                let mut s = state.write();
                                match s.seek.parse::<u32>() {
                                    Ok(value) if value <= s.clock.duration => {
                                        if s.pending {
                                            s.token = s.token.wrapping_add(1);
                                            s.pending = false;
                                        }
                                        s.clock.seek(value);
                                        s.error = None;
                                    }
                                    _ => s.error = Some("Seek must be within the encounter.".into()),
                                }
                            },
                            "Seek"
                        }
                        Label { html_for: "vcr-lookback".to_string(), "Lookback seconds (0–30)" }
                        Input {
                            id: "vcr-lookback".to_string(),
                            value: current.lookback.clone(),
                            class: "max-w-28".to_string(),
                            oninput: move |event: FormEvent| state.write().lookback = event.value(),
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            onclick: move |_| {
                                let mut s = state.write();
                                s.persistent = !s.persistent;
                            },
                            {
                                if current.persistent {
                                    "Keep inactive actors ✓"
                                } else {
                                    "Keep inactive actors"
                                }
                            }
                        }
                    }
                    div { class: "flex flex-wrap gap-2",
                        for category in snapshot.categories.iter().cloned() {
                            {
                                let selected = current.selected.contains(&category);
                                let key = category.clone();
                                rsx! {
                                    Button {
                                        variant: ButtonVariant::Outline,
                                        onclick: move |_| {
                                            let mut s = state.write();
                                            if selected {
                                                s.selected.retain(|name| name != &key);
                                            } else {
                                                s.selected.push(key.clone());
                                            }
                                        },
                                        "{category} "
                                        {if selected { "✓" } else { "" }}
                                    }
                                }
                            }
                        }
                    }
                    {
                        match frame(snapshot, &current) {
                            Ok(frame) => rsx! {
                                if matches!(snapshot.summary.status, 2 | 3) {
                                    p { "Import is incomplete or still running; some actions may be missing." }
                                }
                                svg { view_box: "0 0 600 600", role: "img",
                                    for edge in frame.edges.iter().take(512) {
                                        {
                                            let from = frame.nodes.iter().take(128).position(|node| node.name == edge.from);
                                            let to = frame.nodes.iter().take(128).position(|node| node.name == edge.to);
                                            if let (Some(from), Some(to)) = (from, to) {
                                                let (x1, y1) = position(from, frame.nodes.len().min(128));
                                                let (x2, y2) = position(to, frame.nodes.len().min(128));
                                                rsx! {
                                                    line {
                                                        x1: "{x1}",
                                                        y1: "{y1}",
                                                        x2: "{x2}",
                                                        y2: "{y2}",
                                                        stroke: "currentColor",
                                                        stroke_width: "1",
                                                    }
                                                }
                                            } else {
                                                rsx! {}
                                            }
                                        }
                                    }
                                    for (index, node) in frame.nodes.iter().take(128).enumerate() {
                                        {
                                            let (x, y) = position(index, frame.nodes.len().min(128));
                                            rsx! {
                                                circle {
                                                    cx: "{x}",
                                                    cy: "{y}",
                                                    r: "5",
                                                    fill: "currentColor",
                                                }
                                                text { x: "{x}", y: "{y}", fill: "currentColor", "{node.name}" }
                                            }
                                        }
                                    }
                                }
                                p {
                                    "Diagram and lists display up to 128 actors and 512 interactions; totals include all calculated rows."
                                }
                                p { "Actors ({frame.nodes.len()})" }
                                for node in frame.nodes.iter().take(128) {
                                    p {
                                        "{node.name}: inferred health {node.health:.2}, alive {node.alive}/{node.instances}, active {node.active}"
                                    }
                                }
                                p { "Recent interactions ({frame.edges.len()})" }
                                for edge in frame.edges.iter().take(512) {
                                    p { "{edge.from} → {edge.to} · {edge.category}: {edge.amount}" }
                                }
                            },
                            Err(error) => rsx! {
                                p { role: "alert", "{error}" }
                            },
                        }
                    }
                }
            }
        }
    }
}
