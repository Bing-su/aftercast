use super::{AppState, Phase, tr};
use crate::components::{Accordion, Button, ButtonVariant, Input, Label};
use aftercast::{EncounterId, EncounterStatus, HostHandle, TextReportOptions, TextReportRevision};
use dioxus::desktop::WeakDesktopContext;
use dioxus::{core::spawn_forever, prelude::*};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
mod window;
type EndedKey = (EncounterId, TextReportRevision);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ticket {
    session: u64,
    generation: u64,
}
#[derive(Clone, Default)]
struct Frame {
    output: Option<String>,
    graph: Option<aftercast::MiniGraph>,
    graph_mode: bool,
    graph_totals: bool,
    bar_colors: [String; 3],
    error: Option<String>,
    truncated: bool,
    pending: bool,
    style: String,
}
#[derive(Default)]
struct WindowState {
    serial: u64,
    pending: Option<u64>,
    alive: Arc<AtomicBool>,
    desktop: WeakDesktopContext,
    frame: Arc<Mutex<Frame>>,
    observed: bool,
}
impl WindowState {
    fn reserve(&mut self) -> Option<u64> {
        if self.alive.load(Ordering::SeqCst) || self.pending.is_some() {
            return None;
        }
        let serial = self.serial.checked_add(1)?;
        self.serial = serial;
        self.pending = Some(serial);
        self.desktop = WeakDesktopContext::default();
        self.alive = Arc::new(AtomicBool::new(true));
        self.observed = true;
        Some(serial)
    }
    fn resolve(&mut self, serial: u64, success: bool) -> bool {
        if self.pending != Some(serial) {
            return false;
        }
        self.pending = None;
        if !success {
            self.alive.store(false, Ordering::SeqCst);
        }
        true
    }
}
#[derive(Default)]
pub(super) struct State {
    enabled: bool,
    generation: u64,
    pending: Option<Ticket>,
    pending_settings: Option<(String, u8, Option<TextReportOptions>)>,
    pending_graph_settings: Option<(bool, bool, bool, bool, bool)>,
    last_ended_graph_settings: Option<(bool, bool, bool, bool, bool)>,
    last: Option<Duration>,
    clock: Option<Instant>,
    force_refresh: bool,
    last_ended: Option<EndedKey>,
    last_ended_profile: Option<Option<TextReportOptions>>,
    output: Option<String>,
    graph: Option<aftercast::MiniGraph>,
    graph_mode: bool,
    graph_totals: bool,
    truncated: bool,
    error: Option<String>,
    window: WindowState,
    mini_style: String,
    bar_colors: [String; 3],
}
impl State {
    fn now(&mut self) -> Duration {
        self.clock.get_or_insert_with(Instant::now).elapsed()
    }
    fn combat_changed(&mut self) {
        if !self.force_refresh {
            self.force_refresh = true;
            if self.pending.is_some() {
                self.generation = self.generation.wrapping_add(1);
            }
        }
    }
    fn reusable_end(&self, profile: &Option<TextReportOptions>) -> Option<EndedKey> {
        (self.last_ended_profile.as_ref() == Some(profile))
            .then_some(self.last_ended)
            .flatten()
    }
    fn missing_profile(&mut self, now: Duration, error: String) {
        self.error = Some(error);
        self.last = Some(now);
        self.output = None;
        self.graph = None;
        self.truncated = false;
        self.last_ended = None;
        self.last_ended_profile = None;
        self.last_ended_graph_settings = None;
        self.publish();
    }
    fn window_open(&self) -> bool {
        self.window.alive.load(Ordering::SeqCst)
    }
    fn active(&self, screen: &str) -> bool {
        self.window_open() || (self.enabled && screen == "analysis")
    }
    fn publish(&self) {
        let mut frame = self
            .window
            .frame
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *frame = Frame {
            output: self.output.clone(),
            graph: self.graph.clone(),
            graph_mode: self.graph_mode,
            graph_totals: self.graph_totals,
            bar_colors: self.bar_colors.clone(),
            error: self.error.clone(),
            truncated: self.truncated,
            pending: self.pending.is_some(),
            style: self.mini_style.clone(),
        };
    }
    pub(super) fn stop(&mut self) {
        self.enabled = false;
        if !self.window_open() {
            self.invalidate();
        }
    }
    pub(super) fn reset_session(&mut self) {
        self.window.alive.store(false, Ordering::SeqCst);
        self.window.pending = None;
        self.window.serial = self.window.serial.wrapping_add(1);
        self.window.observed = false;
        self.enabled = false;
        self.pending = None;
        self.pending_settings = None;
        self.pending_graph_settings = None;
        self.invalidate();
    }
    fn observe_window(&mut self) {
        if self.window.observed && !self.window_open() {
            self.window.observed = false;
            if !self.enabled {
                self.invalidate();
            }
        }
    }
    fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.last = None;
        self.force_refresh = false;
        self.last_ended = None;
        self.last_ended_profile = None;
        self.last_ended_graph_settings = None;
        self.output = None;
        self.graph = None;
        self.truncated = false;
        self.error = None;
        self.publish();
    }
}
fn bounded_preview(text: String) -> (Option<String>, bool) {
    let mut end = text.len().min(64 * 1024);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let truncated = end < text.len();
    (
        Some(text.get(..end).unwrap_or_default().to_owned()),
        truncated,
    )
}
fn due(last: Option<Duration>, now: Duration, seconds: u8) -> bool {
    last.is_none_or(|last| now.saturating_sub(last) >= Duration::from_secs(u64::from(seconds)))
}
impl AppState {
    fn mini_toggle(&mut self) {
        if self.mini_parse.enabled {
            self.mini_parse.stop();
        } else {
            self.mini_parse.enabled = true;
            if !self.mini_parse.window_open() {
                self.mini_parse.invalidate();
            }
        }
    }
    fn mini_begin(
        &mut self,
        now: Duration,
    ) -> Option<(
        Ticket,
        HostHandle,
        Option<TextReportOptions>,
        Option<EndedKey>,
    )> {
        self.mini_begin_on(now, self.handle()?)
    }
    fn mini_begin_on(
        &mut self,
        now: Duration,
        handle: HostHandle,
    ) -> Option<(
        Ticket,
        HostHandle,
        Option<TextReportOptions>,
        Option<EndedKey>,
    )> {
        if !self.mini_parse.active(self.screen)
            || self.phase != Phase::Ready
            || self.exit_requested
            || self.loading
            || self.history_sources.holds_resources()
            || self.mini_parse.pending.is_some()
        {
            return None;
        }
        if !self.mini_parse.force_refresh
            && !due(
                self.mini_parse.last,
                now,
                self.settings.mini_parse.interval_seconds,
            )
        {
            return None;
        }
        let profile = if self.settings.mini_parse.profile.is_empty() {
            None
        } else {
            match self
                .file
                .as_ref()
                .and_then(|file| {
                    file.settings
                        .text_profiles
                        .get(&self.settings.mini_parse.profile)
                })
                .cloned()
            {
                Some(profile) => Some(profile),
                None => {
                    self.mini_parse
                        .missing_profile(now, tr("mini-missing-profile"));
                    return None;
                }
            }
        };
        let ticket = Ticket {
            session: self.revision.session,
            generation: self.mini_parse.generation,
        };
        let graph_settings = (
            self.settings.mini_parse.graph,
            self.settings.graphs.only_allies,
            self.settings.graphs.totals,
            self.settings.graphs.use_suffix,
            self.settings.graphs.suffix_decimals,
        );
        self.mini_parse.graph_mode = graph_settings.0;
        self.mini_parse.graph_totals = graph_settings.2;
        self.mini_parse.pending_graph_settings = Some(graph_settings);
        self.mini_parse.last = Some(now);
        self.mini_parse.pending = Some(ticket);
        self.mini_parse.pending_settings = Some((
            self.settings.mini_parse.profile.clone(),
            self.settings.mini_parse.interval_seconds,
            profile.clone(),
        ));
        self.mini_parse.publish();
        let forced = std::mem::take(&mut self.mini_parse.force_refresh);
        let last_ended =
            if forced || self.mini_parse.last_ended_graph_settings != Some(graph_settings) {
                None
            } else {
                self.mini_parse.reusable_end(&profile)
            };
        Some((ticket, handle, profile, last_ended))
    }
    fn mini_finish_graph(
        &mut self,
        ticket: Ticket,
        result: Result<Option<(String, Option<EndedKey>)>, String>,
        graph: Option<aftercast::MiniGraph>,
    ) {
        if self.mini_finish(ticket, result)
            && let Some(graph) = graph
        {
            self.mini_parse.graph = Some(graph);
            self.mini_parse.publish();
        }
    }
    fn mini_finish(
        &mut self,
        ticket: Ticket,
        result: Result<Option<(String, Option<EndedKey>)>, String>,
    ) -> bool {
        if self.mini_parse.pending != Some(ticket) {
            return false;
        }
        self.mini_parse.pending = None;
        let Some((name, interval, profile)) = self.mini_parse.pending_settings.take() else {
            return false;
        };
        let current = if name.is_empty() {
            None
        } else {
            self.file
                .as_ref()
                .and_then(|file| file.settings.text_profiles.get(&name))
                .cloned()
        };
        let graph_settings = (
            self.settings.mini_parse.graph,
            self.settings.graphs.only_allies,
            self.settings.graphs.totals,
            self.settings.graphs.use_suffix,
            self.settings.graphs.suffix_decimals,
        );
        if self
            .mini_parse
            .pending_graph_settings
            .take()
            .is_some_and(|captured| captured != graph_settings)
            || self.settings.mini_parse.profile != name
            || self.settings.mini_parse.interval_seconds != interval
            || current != profile
        {
            self.mini_parse.invalidate();
            return false;
        }
        if self.mini_parse.generation != ticket.generation
            && self.mini_parse.force_refresh
            && self.revision.session == ticket.session
            && !self.exit_requested
        {
            self.mini_parse.publish();
            return false;
        }
        if !self.mini_parse.active(self.screen)
            || self.phase != Phase::Ready
            || self.exit_requested
            || self.revision.session != ticket.session
            || self.mini_parse.generation != ticket.generation
        {
            self.mini_parse.output = None;
            self.mini_parse.graph = None;
            self.mini_parse.truncated = false;
            self.mini_parse.publish();
            return false;
        }
        match result {
            Ok(Some((output, ended))) => {
                self.mini_parse.last_ended = ended;
                self.mini_parse.last_ended_graph_settings = ended.map(|_| graph_settings);
                self.mini_parse.last_ended_profile = ended.map(|_| profile);
                let (output, truncated) = bounded_preview(output);
                self.mini_parse.output = output;
                self.mini_parse.truncated = truncated;
                self.mini_parse.error = None;
            }
            // During a transition or with no current encounter, retain the last frame.
            Ok(None) => {
                self.mini_parse.error = None;
            }
            Err(error) => {
                self.mini_parse.output = None;
                self.mini_parse.graph = None;
                self.mini_parse.truncated = false;
                self.mini_parse.error = Some(error);
            }
        }
        self.mini_parse.publish();
        true
    }
}
async fn fetch(
    handle: &HostHandle,
    profile: Option<TextReportOptions>,
    last_ended: Option<EndedKey>,
) -> Result<Option<(String, Option<EndedKey>)>, String> {
    let Some(active) = handle
        .current_zone_encounter()
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    let catalog = handle
        .text_report_catalog()
        .await
        .map_err(|e| e.to_string())?;
    let is_profile = profile.is_some();
    let options = profile.unwrap_or_else(|| catalog.default_options.clone());
    if is_profile && !options.sorting.is_empty() && !catalog.sort_columns.contains(&options.sorting)
    {
        return Err("report-sort-missing".to_owned());
    }
    if active.summary.status == EncounterStatus::Ended
        && last_ended == Some((active.summary.id, catalog.revision))
    {
        return Ok(None);
    }
    let report = handle
        .text_report(active.summary.id, catalog.revision, &options)
        .await
        .map_err(|e| e.to_string())?;
    let current = handle
        .current_zone_encounter()
        .await
        .map_err(|e| e.to_string())?;
    if current.as_ref().is_none_or(|current| {
        current.summary.id != active.summary.id || current.summary.status != active.summary.status
    }) {
        return Ok(None);
    }
    let ended = (active.summary.status == EncounterStatus::Ended)
        .then_some((active.summary.id, catalog.revision));
    Ok(report.map(|report| (report.text, ended)))
}
#[cfg(test)]
async fn fetch_graph(
    handle: &HostHandle,
    profile: Option<TextReportOptions>,
    last_ended: Option<EndedKey>,
    only_allies: bool,
) -> Result<Option<(aftercast::MiniGraph, Option<EndedKey>)>, String> {
    fetch_graph_options(handle, profile, last_ended, only_allies, true, true).await
}
async fn fetch_graph_options(
    handle: &HostHandle,
    profile: Option<TextReportOptions>,
    last_ended: Option<EndedKey>,
    only_allies: bool,
    use_suffix: bool,
    suffix_decimals: bool,
) -> Result<Option<(aftercast::MiniGraph, Option<EndedKey>)>, String> {
    let Some(active) = handle
        .current_zone_encounter()
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    let catalog = handle
        .text_report_catalog()
        .await
        .map_err(|e| e.to_string())?;
    let options = profile.unwrap_or_else(|| catalog.default_options.clone());
    if options.sorting.is_empty() || !catalog.sort_columns.contains(&options.sorting) {
        return Err("report-sort-missing".into());
    }
    if active.summary.status == EncounterStatus::Ended
        && last_ended == Some((active.summary.id, catalog.revision))
    {
        return Ok(None);
    }
    let graph = handle
        .mini_graph_with_format(
            active.summary.id,
            &options.sorting,
            only_allies,
            use_suffix,
            suffix_decimals,
        )
        .await
        .map_err(|e| e.to_string())?;
    let current = handle
        .current_zone_encounter()
        .await
        .map_err(|e| e.to_string())?;
    if current.as_ref().is_none_or(|current| {
        current.summary.id != active.summary.id || current.summary.status != active.summary.status
    }) {
        return Ok(None);
    }
    let ended = (active.summary.status == EncounterStatus::Ended)
        .then_some((active.summary.id, catalog.revision));
    Ok(Some((graph, ended)))
}

fn mini_bar_style(
    value: f64,
    axis: f64,
    index: usize,
    is_self: bool,
    colors: &[String; 3],
) -> String {
    let slot = if is_self { 2 } else { index % 2 };
    let defaults = ["#8b0000", "#00008b", "#9400d3"];
    let candidate = colors.get(slot).map(String::as_str).unwrap_or("");
    let color = if !candidate.is_empty() && crate::config::valid_color(candidate) {
        candidate
    } else {
        defaults.get(slot).copied().unwrap_or("#8b0000")
    };
    format!("background:{color};width:{:.3}%;", value / axis * 100.0)
}

#[component]
fn GraphView(
    graph: aftercast::MiniGraph,
    totals: bool,
    style: String,
    colors: [String; 3],
) -> Element {
    let content = match graph.data {
        None => {
            rsx! {
                p {
                    if graph.no_actors {
                        {tr("mini-graph-no-actors")}
                    } else {
                        {tr("mini-graph-empty")}
                    }
                }
            }
        }
        Some(data) => {
            let omitted = data.values.len().saturating_sub(512);
            let Some(display) = graph.display else {
                return rsx! {
                    p { role: "alert", "Invalid graph display" }
                };
            };
            rsx! {
                div { class: "space-y-2 rounded-md border border-border p-3 text-sm",
                    p {
                        {tr("mini-graph-axis")}
                        ": {display.axis_label}"
                    }
                    for (index, (item, formatted)) in data.values.iter().zip(display.rows.iter()).take(512).enumerate() {
                        div { class: "space-y-1",
                            p { title: "{item.name}: {formatted.tooltip_value}",
                                "{item.name}: {formatted.label}"
                            }
                            div { class: "h-3 w-full rounded bg-muted",
                                div {
                                    class: "h-3 rounded",
                                    style: mini_bar_style(item.value, data.axis, index, formatted.is_self, &colors),
                                }
                            }
                        }
                    }
                    if omitted > 0 {
                        p {
                            "{omitted} "
                            {tr("mini-graph-omitted")}
                        }
                    }
                    if totals {
                        p { class: "whitespace-pre-line", "{display.totals_text}" }
                    }
                }
            }
        }
    };
    rsx! {
        div { style, {content} }
    }
}

fn launch(mut state: Signal<AppState>) {
    let work = {
        let mut current = state.write();
        let now = current.mini_parse.now();
        current.mini_begin(now)
    };
    if let Some((ticket, handle, profile, last_ended)) = work {
        let graph_settings = state
            .peek()
            .mini_parse
            .pending_graph_settings
            .unwrap_or((false, true, true, true, true));
        spawn_forever(async move {
            let (result, graph) = if graph_settings.0 {
                match fetch_graph_options(
                    &handle,
                    profile,
                    last_ended,
                    graph_settings.1,
                    graph_settings.3,
                    graph_settings.4,
                )
                .await
                {
                    Ok(Some((graph, ended))) => (Ok(Some((String::new(), ended))), Some(graph)),
                    Ok(None) => (Ok(None), None),
                    Err(error) => (Err(error), None),
                }
            } else {
                (fetch(&handle, profile, last_ended).await, None)
            };
            state.write().mini_finish_graph(ticket, result, graph);
            let refresh = state.read().mini_parse.force_refresh;
            if refresh {
                launch(state);
            }
        });
    }
}

fn notify_combat(mut state: Signal<AppState>, session: u64) {
    {
        let mut current = state.write();
        if current.revision.session != session
            || current.exit_requested
            || !current.mini_parse.active(current.screen)
        {
            return;
        }
        current.mini_parse.combat_changed();
    }
    launch(state);
}

pub(super) async fn monitor_combat(state: Signal<AppState>) {
    loop {
        let endpoint = {
            let current = state.read();
            if current.exit_requested {
                None
            } else {
                current
                    .handle()
                    .map(|handle| (current.revision.session, handle))
            }
        };
        let Some((session, handle)) = endpoint else {
            dioxus_sdk::time::sleep(Duration::from_millis(200)).await;
            continue;
        };
        let mut changes = handle.combat_changes();
        if *changes.borrow_and_update() != 0 {
            notify_combat(state, session);
        }
        loop {
            tokio::select! {
                changed = changes.changed() => {
                    if changed.is_err() { break; }
                    notify_combat(state, session);
                }
                () = dioxus_sdk::time::sleep(Duration::from_millis(200)) => {
                    let current = state.read();
                    if current.revision.session != session || current.handle().is_none() || current.exit_requested { break; }
                }
            }
        }
        dioxus_sdk::time::sleep(Duration::from_millis(200)).await;
    }
}

pub(super) async fn poll(mut state: Signal<AppState>) {
    loop {
        dioxus_sdk::time::sleep(Duration::from_secs(1)).await;
        {
            let mut current = state.write();
            let style = current.settings.appearance.mini_style();
            let colors = current.settings.graphs.bar_colors.clone();
            if current.mini_parse.mini_style != style || current.mini_parse.bar_colors != colors {
                current.mini_parse.mini_style = style;
                current.mini_parse.bar_colors = colors;
                current.mini_parse.publish();
            }
            if current.exit_requested {
                current.mini_parse.reset_session();
                continue;
            }
            if current.screen != "analysis" && current.mini_parse.enabled {
                current.mini_parse.stop();
            }
            current.mini_parse.observe_window();
            if current
                .mini_parse
                .pending
                .is_some_and(|ticket| ticket.session != current.revision.session)
            {
                current.mini_parse.reset_session();
            }
        }
        launch(state);
    }
}
#[component]
pub(super) fn Panel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let enabled = current.mini_parse.enabled;
    let pending = current.mini_parse.pending.is_some();
    let interval = current.settings.mini_parse.interval_seconds;
    let profile = current.settings.mini_parse.profile.clone();
    let profiles = current
        .file
        .as_ref()
        .map(|f| f.settings.text_profiles.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let missing_profile = !profile.is_empty() && !profiles.contains(&profile);
    let mini_style = current.settings.appearance.mini_style();
    let output = current.mini_parse.output.clone();
    let graph = current.mini_parse.graph.clone();
    let graph_mode = current.settings.mini_parse.graph;
    let graph_totals = current.settings.graphs.totals;
    let graph_allies = current.settings.graphs.only_allies;
    let bar_colors = current.settings.graphs.bar_colors.clone();
    let use_suffix = current.settings.graphs.use_suffix;
    let suffix_decimals = current.settings.graphs.suffix_decimals;
    let truncated = current.mini_parse.truncated;
    let error = current.mini_parse.error.clone();
    let ready = current.phase == Phase::Ready && !current.exit_requested;
    let can_save = current.can_save_settings();
    drop(current);
    rsx! {
        Accordion {
            class: "mt-6 rounded-md border border-border".to_owned(),
            default_open: false,
            on_toggle: move |open: bool| {
                if !open {
                    state.write().mini_parse.stop();
                }
            },
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! {
                {tr("mini-title")}
            },
            section { class: "space-y-3 p-4", "aria-label": tr("mini-title"),
                p { class: "text-sm text-muted-foreground", {tr("mini-help")} }
                Label { html_for: "mini-profile", {tr("mini-profile")} }
                select {
                    id: "mini-profile",
                    value: profile,
                    class: "h-9 rounded-md border border-border bg-background px-2",
                    onchange: move |event| {
                        let mut s = state.write();
                        s.settings.mini_parse.profile = event.value();
                        s.mini_parse.invalidate();
                    },
                    option { value: "", {tr("report-default")} }
                    if missing_profile {
                        option { value: profile.clone(),
                            "{profile} — "
                            {tr("mini-missing-profile")}
                        }
                    }
                    for name in profiles {
                        option { value: name.clone(), {name.clone()} }
                    }
                }
                Label { html_for: "mini-interval", {tr("mini-interval")} }
                select {
                    id: "mini-interval",
                    value: interval.to_string(),
                    class: "h-9 rounded-md border border-border bg-background px-2",
                    onchange: move |event| {
                        if let Ok(value) = event.value().parse::<u8>() && (1..=60).contains(&value) {
                            let mut s = state.write();
                            s.settings.mini_parse.interval_seconds = value;
                            s.mini_parse.invalidate();
                        }
                    },
                    for value in 1..=60_u8 {
                        option { value: value.to_string(), "{value}" }
                    }
                }
                div { class: "flex flex-wrap gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: !can_save,
                        aria_pressed: graph_mode,
                        onclick: move |_| {
                            let mut s = state.write();
                            if !s.can_save_settings() || s.settings.mini_parse.graph != graph_mode {
                                return;
                            }
                            s.settings.mini_parse.graph = !graph_mode;
                            s.mini_parse.graph_mode = !graph_mode;
                            s.mini_parse.invalidate();
                        },
                        {tr("mini-graph-toggle")}
                    }
                    if graph_mode {
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: !can_save,
                            aria_pressed: graph_allies,
                            onclick: move |_| {
                                let mut s = state.write();
                                if !s.can_save_settings() || !s.settings.mini_parse.graph
                                    || s.settings.graphs.only_allies != graph_allies
                                {
                                    return;
                                }
                                s.settings.graphs.only_allies = !graph_allies;
                                s.mini_parse.invalidate();
                            },
                            {tr("mini-graph-allies")}
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: !can_save,
                            aria_pressed: use_suffix,
                            onclick: move |_| {
                                let mut s = state.write();
                                if !s.can_save_settings() || !s.settings.mini_parse.graph
                                    || s.settings.graphs.use_suffix != use_suffix
                                {
                                    return;
                                }
                                s.settings.graphs.use_suffix = !use_suffix;
                                s.mini_parse.invalidate();
                            },
                            {tr("mini-graph-suffix")}
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: !can_save || !use_suffix,
                            aria_pressed: suffix_decimals,
                            onclick: move |_| {
                                let mut s = state.write();
                                if !s.can_save_settings() || !s.settings.mini_parse.graph
                                    || s.settings.graphs.suffix_decimals != suffix_decimals
                                {
                                    return;
                                }
                                s.settings.graphs.suffix_decimals = !suffix_decimals;
                                s.mini_parse.invalidate();
                            },
                            {tr("mini-graph-suffix-decimals")}
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: !can_save,
                            aria_pressed: graph_totals,
                            onclick: move |_| {
                                let mut s = state.write();
                                if !s.can_save_settings() || !s.settings.mini_parse.graph
                                    || s.settings.graphs.totals != graph_totals
                                {
                                    return;
                                }
                                s.settings.graphs.totals = !graph_totals;
                                s.mini_parse.graph_totals = !graph_totals;
                                s.mini_parse.invalidate();
                            },
                            {tr("mini-graph-totals")}
                        }
                    }
                }
                if graph_mode {
                    div { class: "grid gap-2 sm:grid-cols-3",
                        for (index, id, label) in [
                            (0, "mini-bar-one", "mini-graph-color-one"),
                            (1, "mini-bar-two", "mini-graph-color-two"),
                            (2, "mini-bar-self", "mini-graph-color-self"),
                        ]
                        {
                            div {
                                Label { html_for: id, {tr(label)} }
                                Input {
                                    id: id.to_owned(),
                                    value: bar_colors.get(index).cloned().unwrap_or_default(),
                                    disabled: !can_save,
                                    oninput: move |event: FormEvent| {
                                        let mut s = state.write();
                                        if s.can_save_settings()
                                            && let Some(color) = s.settings.graphs.bar_colors.get_mut(index)
                                        {
                                            *color = event.value();
                                        }
                                    },
                                }
                            }
                        }
                    }
                }
                div { class: "flex flex-wrap gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: !ready,
                        onclick: move |_| window::open(state),
                        {tr("mini-open-window")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: !can_save,
                        onclick: move |_| super::save_settings(state),
                        {tr("mini-save")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: !ready && !enabled,
                        aria_pressed: Some(enabled),
                        onclick: move |_| state.write().mini_toggle(),
                        {tr(if enabled { "mini-stop" } else { "mini-start" })}
                    }
                }
                if let Some(error) = error {
                    p { class: "text-sm text-destructive",
                        {if error == "report-sort-missing" { tr("report-sort-missing") } else { error }}
                    }
                }
                if enabled && graph_mode {
                    if let Some(graph) = graph {
                        GraphView {
                            graph,
                            totals: graph_totals,
                            style: mini_style.clone(),
                            colors: bar_colors.clone(),
                        }
                    } else if pending {
                        p { {tr("mini-loading")} }
                    } else {
                        p { {tr("mini-empty")} }
                    }
                }
                if enabled && !graph_mode {
                    pre {
                        class: "max-h-80 overflow-auto whitespace-pre-wrap rounded-md border border-border p-3 text-sm",
                        style: mini_style,
                        {
                            match output {
                                Some(text) if text.is_empty() => tr("mini-empty-output"),
                                Some(text) => text,
                                None if pending => tr("mini-loading"),
                                None => tr("mini-empty"),
                            }
                        }
                    }
                    if truncated {
                        p { class: "text-xs text-muted-foreground", {tr("mini-truncated")} }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn graph_bar_color_and_axis_scale_are_deterministic() {
        let custom = ["#123456".into(), "bad;color:red".into(), "#654321".into()];
        assert_eq!(
            mini_bar_style(1.0, 2.0, 0, false, &custom),
            "background:#123456;width:50.000%;"
        );
        assert_eq!(
            mini_bar_style(1.0, 2.0, 1, false, &custom),
            "background:#00008b;width:50.000%;"
        );
        assert_eq!(
            mini_bar_style(1.0, 2.0, 1, true, &custom),
            "background:#654321;width:50.000%;"
        );
        assert_eq!(
            mini_bar_style(
                50.0,
                100.0,
                0,
                false,
                &crate::config::GraphSettings::default().bar_colors
            ),
            "background:#8b0000;width:50.000%;"
        );
        assert_eq!(
            mini_bar_style(
                25.0,
                100.0,
                1,
                true,
                &crate::config::GraphSettings::default().bar_colors
            ),
            "background:#9400d3;width:25.000%;"
        );
        assert_eq!(
            mini_bar_style(
                100.0,
                110.0,
                1,
                false,
                &crate::config::GraphSettings::default().bar_colors
            ),
            "background:#00008b;width:90.909%;"
        );
    }
    #[test]
    fn graph_options_retire_pending_work_and_window_state() {
        let mut ui = AppState::new("/tmp".into());
        ui.phase = Phase::Ready;
        ui.screen = "analysis";
        ui.revision.session = 7;
        ui.mini_parse.enabled = true;
        let ticket = Ticket {
            session: 7,
            generation: ui.mini_parse.generation,
        };
        ui.mini_parse.pending = Some(ticket);
        ui.mini_parse.pending_settings = Some((String::new(), 5, None));
        ui.mini_parse.pending_graph_settings = Some((true, true, true, true, true));
        ui.settings.mini_parse.graph = true;
        ui.settings.graphs.only_allies = false;
        assert!(!ui.mini_finish(ticket, Ok(Some((String::new(), None)))));
        assert!(ui.mini_parse.graph.is_none());
        let ticket = Ticket {
            session: 7,
            generation: ui.mini_parse.generation,
        };
        ui.mini_parse.pending = Some(ticket);
        ui.mini_parse.pending_settings = Some((String::new(), 5, None));
        ui.mini_parse.pending_graph_settings = Some((true, true, true, true, true));
        ui.settings.graphs.only_allies = true;
        ui.settings.graphs.use_suffix = false;
        assert!(!ui.mini_finish(ticket, Ok(Some((String::new(), None)))));
        assert!(ui.mini_parse.graph.is_none());
        let first = ui.mini_parse.window.reserve().unwrap();
        assert!(ui.mini_parse.window.resolve(first, true));
        ui.mini_parse.reset_session();
        assert!(!ui.mini_parse.window.resolve(first, true));
        assert!(ui.mini_parse.graph.is_none());
    }
    #[tokio::test(flavor = "current_thread")]
    #[ignore = "requires managed fixture; run mise run test:gui-state"]
    async fn mini_graph_ui_contract_worker_reads_numeric_values_without_report_capture_mutation() {
        let (_temp, host) = start_synthetic_host().await;
        assert!(
            fetch_graph(&host.handle(), None, None, false)
                .await
                .unwrap()
                .is_none()
        );
        add_plugin(&host, "MiniGraphFixturePlugin").await;
        let active = tokio::time::timeout(Duration::from_secs(2), wait_for_graph_actors(&host))
            .await
            .expect("synthetic actions drained");
        let capture = host
            .capture_hierarchy_html(&[active.summary.id], aftercast::TableLevel::Combatant)
            .await
            .unwrap()
            .unwrap();
        let profile = TextReportOptions {
            sorting: "mini-graph-value".into(),
            ..Default::default()
        };
        let (graph, _) = fetch_graph(&host.handle(), Some(profile.clone()), None, false)
            .await
            .unwrap()
            .unwrap();
        let mut cached = State {
            graph: Some(graph.clone()),
            ..Default::default()
        };
        cached.missing_profile(Duration::ZERO, "missing profile".into());
        assert!(cached.graph.is_none());
        assert_eq!(cached.error.as_deref(), Some("missing profile"));
        let display = graph.display.clone().expect("formatted graph");
        assert_eq!(display.rows.len(), 2);
        assert_eq!(display.rows[0].label, "100");
        assert_eq!(display.rows[1].label, "50");
        assert_eq!(display.axis_label, "110");
        let data = graph.data.unwrap();
        assert_eq!(data.values.len(), 2);
        assert_eq!(data.values[0].name, "Synthetic Source");
        assert_eq!(data.values[0].value, 100.0);
        assert_eq!(data.values[1].name, "Second Source");
        assert_eq!(data.values[1].value, 50.0);
        assert!((data.axis - 110.0).abs() < 0.00001);
        assert_eq!(data.total, 150.0);
        assert_eq!(data.average, 75.0);
        let abbreviated = host
            .mini_graph_with_format(active.summary.id, "mini-graph-scaled", false, true, true)
            .await
            .expect("suffix");
        let plain = host
            .mini_graph_with_format(active.summary.id, "mini-graph-scaled", false, false, true)
            .await
            .expect("plain");
        assert!(
            abbreviated.display.expect("display").rows[0]
                .label
                .ends_with('K')
        );
        assert_eq!(plain.display.expect("display").rows[0].label, "2000");
        let partial = host
            .mini_graph(active.summary.id, "mini-graph-partial", false)
            .await
            .expect("row exception is skipped");
        assert_eq!(partial.data.expect("remaining rows").total, 150.0);
        for key in ["mini-graph-zero", "mini-graph-negative", "mini-graph-fail"] {
            let empty = host
                .mini_graph(active.summary.id, key, false)
                .await
                .expect("empty numeric graph");
            assert!(!empty.no_actors && empty.data.is_none());
        }
        host.mini_graph(active.summary.id, "mini-graph-infinite", false)
            .await
            .expect_err("nonfinite graph must fail explicitly");
        host.mini_graph(active.summary.id, "mini-graph-badsort", false)
            .await
            .expect_err("sort callback must fail explicitly");
        verify_graph_capture_and_transition(&host, active.summary.id, &capture, profile).await;
        host.shutdown().await.unwrap();
    }

    async fn verify_graph_capture_and_transition(
        host: &aftercast::Host,
        id: EncounterId,
        capture: &aftercast::HierarchyHtmlSelection,
        profile: TextReportOptions,
    ) {
        let levels = [aftercast::HierarchyHtmlLevel {
            level: aftercast::TableLevel::Combatant,
            columns: vec!["mini-graph-value".into()],
            sort: None,
        }];
        host.hierarchy_html(capture, &levels)
            .await
            .expect("mini graph must not retire independent HTML capture");
        host.mini_graph(id, "missing-column", false)
            .await
            .expect_err("missing graph key");
        let mut invalid = profile;
        invalid.sorting = "missing-column".into();
        fetch_graph(&host.handle(), Some(invalid), None, false)
            .await
            .expect_err("missing profile key");
        add_plugin(host, "MiniParseEndPlugin").await;
        let (ended, key) = fetch_graph(
            &host.handle(),
            Some(TextReportOptions {
                sorting: "mini-graph-value".into(),
                ..Default::default()
            }),
            None,
            false,
        )
        .await
        .unwrap()
        .unwrap();
        assert!(ended.data.is_some());
        assert!(key.is_some());
        add_plugin(host, "MiniParseNextPlugin").await;
        let next = tokio::time::timeout(Duration::from_secs(2), wait_for_next_graph(host, key))
            .await
            .expect("next encounter transition");
        assert!(next.0.data.is_some());
    }
    async fn wait_for_graph_actors(host: &aftercast::Host) -> aftercast::EncounterDetail {
        loop {
            if let Some(active) = host
                .current_zone_encounter()
                .await
                .expect("current encounter")
                && let Ok(graph) = host
                    .mini_graph(active.summary.id, "mini-graph-value", false)
                    .await
                && graph
                    .data
                    .as_ref()
                    .is_some_and(|data| data.values.len() == 2)
            {
                return active;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
    async fn wait_for_next_graph(
        host: &aftercast::Host,
        key: Option<EndedKey>,
    ) -> (aftercast::MiniGraph, Option<EndedKey>) {
        loop {
            let options = TextReportOptions {
                sorting: "mini-graph-value".into(),
                ..Default::default()
            };
            if let Some(next) = fetch_graph(&host.handle(), Some(options), key, false)
                .await
                .expect("next graph")
            {
                return next;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
    #[test]
    fn window_style_snapshot_survives_session_reset() {
        let mut state = State {
            mini_style: "font-size:16px;color:#abcdef;".into(),
            ..Default::default()
        };
        state.publish();
        assert_eq!(
            state.window.frame.lock().expect("frame").style,
            "font-size:16px;color:#abcdef;"
        );
        state.reset_session();
        assert_eq!(
            state.window.frame.lock().expect("frame").style,
            "font-size:16px;color:#abcdef;"
        );
    }
    #[test]
    fn singleton_window_reservations_and_retirement() {
        let mut state = State::default();
        let first = state.window.reserve().expect("first");
        assert!(state.window.reserve().is_none());
        assert!(!state.window.resolve(first + 1, true));
        assert!(state.window.resolve(first, true));
        assert!(state.window.reserve().is_none());
        assert!(
            state.active("settings"),
            "open window continues after navigation"
        );
        state.stop();
        assert!(
            state.active("settings"),
            "embedded panel cannot stop an open window"
        );
        state.window.alive.store(false, Ordering::SeqCst);
        state.observe_window();
        assert!(!state.active("settings"), "last window close stops refresh");
        let second = state.window.reserve().expect("reopen");
        assert_ne!(first, second);
        state.reset_session();
        assert!(!state.window.resolve(second, true));
        assert!(!state.window_open());
    }
    #[test]
    fn clock_interval_boundaries() {
        assert!(due(None, Duration::ZERO, 5));
        assert!(!due(Some(Duration::ZERO), Duration::from_secs(4), 5));
        assert!(due(Some(Duration::ZERO), Duration::from_secs(5), 5));
    }
    #[test]
    fn stale_result_contract() {
        let mut ui = AppState::new("/tmp".into());
        ui.phase = Phase::Ready;
        ui.screen = "analysis";
        ui.revision.session = 7;
        ui.mini_parse.enabled = true;
        let ticket = Ticket {
            session: 7,
            generation: 0,
        };
        ui.mini_parse.pending = Some(ticket);
        ui.mini_parse.pending_settings = Some((String::new(), 5, None));
        assert!(ui.mini_finish(ticket, Ok(Some(("synthetic".into(), None)))));
        assert_eq!(ui.mini_parse.output.as_deref(), Some("synthetic"));
        ui.mini_parse.pending = Some(ticket);
        ui.mini_parse.pending_settings = Some((String::new(), 5, None));
        assert!(ui.mini_finish(ticket, Ok(None)));
        assert_eq!(ui.mini_parse.output.as_deref(), Some("synthetic"));
        ui.mini_parse.pending = Some(ticket);
        ui.mini_parse.pending_settings = Some((String::new(), 5, None));
        assert!(ui.mini_finish(ticket, Ok(Some((String::new(), None)))));
        assert_eq!(ui.mini_parse.output.as_deref(), Some(""));
        ui.mini_parse.pending = Some(ticket);
        ui.mini_parse.pending_settings = Some((String::new(), 5, None));
        assert!(ui.mini_finish(ticket, Ok(Some(("한".repeat(30000), None)))));
        assert!(ui.mini_parse.truncated);
        assert!(ui.mini_parse.output.as_ref().expect("bounded").len() <= 64 * 1024);
        ui.mini_parse.pending = Some(ticket);
        ui.mini_parse.pending_settings = Some((String::new(), 5, None));
        ui.mini_toggle();
        assert_eq!(
            ui.mini_parse.pending,
            Some(ticket),
            "cancellation must not admit overlapping work"
        );
        ui.mini_toggle();
        assert_eq!(
            ui.mini_parse.pending,
            Some(ticket),
            "restart waits for the previous request"
        );
        assert!(!ui.mini_finish(ticket, Ok(Some(("late".into(), None)))));
        assert!(ui.mini_parse.output.is_none());
        let ticket = Ticket {
            session: 7,
            generation: ui.mini_parse.generation,
        };
        ui.mini_parse.pending = Some(ticket);
        ui.mini_parse.pending_settings = Some((String::new(), 5, None));
        ui.revision.session = 8;
        assert!(!ui.mini_finish(ticket, Ok(Some(("old session".into(), None)))));
        ui.revision.session = 7;
        ui.mini_parse.pending = Some(ticket);
        ui.mini_parse.pending_settings = Some((String::new(), 5, None));
        ui.settings.mini_parse.interval_seconds = 10;
        assert!(!ui.mini_finish(ticket, Ok(Some(("old settings".into(), None)))));
        assert!(ui.mini_parse.output.is_none());
        ui.settings.mini_parse.interval_seconds = 5;
        let ticket = Ticket {
            session: 7,
            generation: ui.mini_parse.generation,
        };
        ui.mini_parse.pending = Some(ticket);
        ui.mini_parse.pending_settings = Some((String::new(), 5, None));
        ui.screen = "settings";
        assert!(!ui.mini_finish(ticket, Ok(Some(("late screen".into(), None)))));
        ui.screen = "analysis";
        ui.settings.mini_parse.profile = "removed".into();
        let ticket = Ticket {
            session: 7,
            generation: ui.mini_parse.generation,
        };
        ui.mini_parse.pending = Some(ticket);
        ui.mini_parse.pending_settings =
            Some(("removed".into(), 5, Some(TextReportOptions::default())));
        assert!(!ui.mini_finish(ticket, Ok(Some(("deleted profile".into(), None)))));
        assert!(ui.mini_parse.output.is_none());
    }
    #[test]
    fn combat_signal_retires_inflight_result_and_coalesces() {
        let mut ui = AppState::new("/tmp".into());
        ui.phase = Phase::Ready;
        ui.screen = "analysis";
        ui.revision.session = 7;
        ui.mini_parse.enabled = true;
        ui.mini_parse.output = Some("previous".into());
        let old = Ticket {
            session: 7,
            generation: ui.mini_parse.generation,
        };
        ui.mini_parse.pending = Some(old);
        ui.mini_parse.pending_settings = Some((String::new(), 5, None));
        ui.mini_parse.combat_changed();
        let generation = ui.mini_parse.generation;
        ui.mini_parse.combat_changed();
        assert_eq!(ui.mini_parse.generation, generation);
        assert!(ui.mini_parse.force_refresh);
        assert!(!ui.mini_finish(old, Ok(Some(("stale".into(), None)))));
        assert_eq!(ui.mini_parse.output.as_deref(), Some("previous"));
        assert!(ui.mini_parse.pending.is_none());
        let pending = Ticket {
            session: 7,
            generation: ui.mini_parse.generation,
        };
        ui.mini_parse.pending = Some(pending);
        ui.mini_parse.pending_settings = Some((String::new(), 5, None));
        ui.mini_parse.force_refresh = false;
        ui.mini_parse.combat_changed();
        ui.revision.session = 8;
        assert!(!ui.mini_finish(pending, Ok(Some(("previous session".into(), None)))));
        assert!(ui.mini_parse.output.is_none());
        ui.mini_parse.stop();
        assert!(!ui.mini_parse.force_refresh);
    }
    fn check_profile_cache(before: EndedKey, options: &TextReportOptions) {
        let cache = State {
            last_ended: Some(before),
            last_ended_profile: Some(Some(options.clone())),
            ..Default::default()
        };
        assert_eq!(cache.reusable_end(&Some(options.clone())), Some(before));
        let mut changed = options.clone();
        changed.player_format.push('!');
        assert_eq!(cache.reusable_end(&Some(changed)), None);
    }
    async fn check_catalog_change(
        host: &aftercast::Host,
        key: EndedKey,
        options: &TextReportOptions,
    ) {
        add_plugin(host, "MiniParseRegistryPlugin").await;
        let revision = host
            .text_report_catalog()
            .await
            .expect("updated catalog")
            .revision;
        assert_ne!(revision, key.1);
        assert!(
            fetch(&host.handle(), Some(options.clone()), Some(key))
                .await
                .expect("changed catalog")
                .is_some()
        );
    }
    async fn add_plugin(host: &aftercast::Host, name: &str) {
        let path = camino::Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll",
        );
        let id = host
            .create_plugin(
                aftercast::PluginConfig::builder()
                    .path(path)
                    .type_name(format!("Aftercast.PluginFixture.{name}"))
                    .build(),
            )
            .await
            .expect("plugin");
        host.initialize_plugin(id).await.expect("initialize");
    }
    async fn start_synthetic_host() -> (tempfile::TempDir, aftercast::Host) {
        use aftercast::{Host, HostConfig, extract_bridge};
        use camino::{Utf8Path, Utf8PathBuf};
        let temp = tempfile::tempdir().expect("temp");
        let root = Utf8Path::from_path(temp.path()).expect("UTF8");
        let bridge = root.join("bridge");
        extract_bridge(&bridge).expect("bridge");
        let executable = std::env::current_exe()
            .expect("tests")
            .parent()
            .and_then(std::path::Path::parent)
            .expect("target")
            .join("examples")
            .join(format!("test-application{}", std::env::consts::EXE_SUFFIX));
        let host = Host::start(
            HostConfig::builder()
                .dotnet_root(std::env::var("DOTNET_ROOT").expect("runtime"))
                .bridge_directory(bridge)
                .data_directory(root.join("state"))
                .application_executable(Utf8PathBuf::try_from(executable).expect("UTF8"))
                .build(),
        )
        .await
        .expect("host");
        (temp, host)
    }
    async fn check_completion_signal(host: &aftercast::Host, before: EncounterId) {
        let mut changes = host.handle().combat_changes();
        changes.borrow_and_update();
        add_plugin(host, "MiniParseEndPlugin").await;
        tokio::time::timeout(Duration::from_secs(2), changes.changed())
            .await
            .expect("prompt owner notification")
            .expect("open channel");
        assert!(*changes.borrow_and_update() > 0);
        let mut controller = AppState::new("/tmp/synthetic-mini".into());
        controller.phase = Phase::Ready;
        controller.screen = "analysis";
        controller.revision.session = 7;
        controller.mini_parse.enabled = true;
        controller.settings.mini_parse.interval_seconds = 60;
        controller.mini_parse.last = Some(Duration::ZERO);
        assert!(
            controller
                .mini_begin_on(Duration::from_secs(1), host.handle())
                .is_none()
        );
        controller.mini_parse.combat_changed();
        let (ticket, handle, profile, last) = controller
            .mini_begin_on(Duration::from_secs(1), host.handle())
            .expect("completion bypasses configured interval");
        let final_frame = fetch(&handle, profile, last)
            .await
            .expect("final frame")
            .expect("ended");
        assert_eq!(final_frame.1.expect("ended identity").0, before);
        assert!(controller.mini_finish(ticket, Ok(Some(final_frame))));
        assert!(!controller.mini_parse.force_refresh);
        assert!(
            tokio::time::timeout(Duration::from_millis(50), changes.changed())
                .await
                .is_err(),
            "no duplicate notification"
        );
    }
    #[tokio::test(flavor = "current_thread")]
    #[ignore = "requires managed fixture; run mise run test:gui-state"]
    async fn empty_combat_publishes_notification_and_safe_frame() {
        let (_temp, host) = start_synthetic_host().await;
        add_plugin(&host, "MiniParseRegistryPlugin").await;
        let mut changes = host.handle().combat_changes();
        changes.borrow_and_update();
        add_plugin(&host, "MiniParseEmptyPlugin").await;
        tokio::time::timeout(Duration::from_secs(2), changes.changed())
            .await
            .expect("empty completion notification")
            .expect("channel");
        let current = host
            .current_zone_encounter()
            .await
            .expect("current")
            .expect("ended empty");
        assert_eq!(current.summary.combatants, 0);
        let (empty, _) = fetch_graph(
            &host.handle(),
            Some(TextReportOptions {
                sorting: "mini-extra".into(),
                ..Default::default()
            }),
            None,
            false,
        )
        .await
        .expect("empty graph")
        .expect("empty encounter");
        assert!(empty.no_actors && empty.data.is_none());
        let frame = fetch(&host.handle(), None, None)
            .await
            .expect("empty frame");
        let (_, ended) = frame.expect("empty encounter frame");
        assert_eq!(ended.expect("ended identity").0, current.summary.id);
        host.shutdown().await.expect("shutdown");
    }
    #[tokio::test(flavor = "current_thread")]
    #[ignore = "requires managed fixture; run mise run test:gui-state"]
    async fn live_preview_uses_real_worker_and_synthetic_combat() {
        let (_temp, host) = start_synthetic_host().await;
        assert!(
            fetch(&host.handle(), None, None)
                .await
                .expect("no combat")
                .is_none()
        );
        add_plugin(&host, "MiniParsePlugin").await;
        let options = TextReportOptions {
            player_format: "{report-name}:{report-unicode}".into(),
            sorting: "report-name".into(),
            show_only_allies: false,
            show_allied_info: false,
            allies_format: String::new(),
        };
        let text = fetch(&host.handle(), Some(options.clone()), None)
            .await
            .expect("preview")
            .expect("active");
        assert!(text.0.contains("한글日本😀"));
        let before = host
            .current_zone_encounter()
            .await
            .expect("current")
            .expect("active")
            .summary
            .id;
        check_completion_signal(&host, before).await;
        assert!(host.live_combat().await.expect("idle").is_none());
        assert_eq!(
            host.current_zone_encounter()
                .await
                .expect("ended")
                .expect("last")
                .summary
                .id,
            before
        );
        assert!(
            fetch(&host.handle(), Some(options.clone()), None)
                .await
                .expect("ended preview")
                .expect("last")
                .0
                .contains("한글日本😀")
        );
        let revision = host.text_report_catalog().await.expect("catalog").revision;
        let ended_key = (before, revision);
        check_profile_cache(ended_key, &options);
        let mut failing = options.clone();
        failing.player_format = "{report-fail}".into();
        let failure = fetch(&host.handle(), Some(failing), None)
            .await
            .expect_err("plugin callback failure");
        let mut failed_ui = AppState::new("/tmp/synthetic-mini".into());
        failed_ui.phase = Phase::Ready;
        failed_ui.mini_parse.enabled = true;
        let ticket = Ticket {
            session: failed_ui.revision.session,
            generation: failed_ui.mini_parse.generation,
        };
        failed_ui.mini_parse.pending = Some(ticket);
        failed_ui.mini_parse.pending_settings = Some((String::new(), 5, None));
        assert!(failed_ui.mini_finish(ticket, Err(failure)));
        assert!(failed_ui.mini_parse.error.is_some());
        assert!(
            fetch(&host.handle(), Some(options.clone()), Some(ended_key))
                .await
                .expect("unchanged ended")
                .is_none()
        );
        check_catalog_change(&host, ended_key, &options).await;
        add_plugin(&host, "MiniParseNextPlugin").await;
        assert_ne!(
            host.current_zone_encounter()
                .await
                .expect("next")
                .expect("active")
                .summary
                .id,
            before
        );
        assert!(
            fetch(&host.handle(), Some(options.clone()), None)
                .await
                .expect("next preview")
                .expect("active")
                .0
                .contains("Next")
        );
        let mut missing = options;
        missing.sorting = "no-such-plugin-key".into();
        fetch(&host.handle(), Some(missing), None)
            .await
            .expect_err("missing sort key");
        let handle = host.handle();
        host.shutdown().await.expect("shutdown");
        fetch(&handle, None, None).await.expect_err("closed worker");
    }
}
