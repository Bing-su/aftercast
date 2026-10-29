use super::{AppState, Phase, tr};
use crate::components::{Accordion, Button, ButtonVariant, Label};
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
    error: Option<String>,
    truncated: bool,
    pending: bool,
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
    last: Option<Duration>,
    last_ended: Option<EndedKey>,
    last_ended_profile: Option<Option<TextReportOptions>>,
    output: Option<String>,
    truncated: bool,
    error: Option<String>,
    window: WindowState,
}
impl State {
    fn reusable_end(&self, profile: &Option<TextReportOptions>) -> Option<EndedKey> {
        (self.last_ended_profile.as_ref() == Some(profile))
            .then_some(self.last_ended)
            .flatten()
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
            error: self.error.clone(),
            truncated: self.truncated,
            pending: self.pending.is_some(),
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
        self.last_ended = None;
        self.last_ended_profile = None;
        self.output = None;
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
        if !self.mini_parse.active(self.screen)
            || self.phase != Phase::Ready
            || self.exit_requested
            || self.loading
            || self.history_sources.holds_resources()
            || self.mini_parse.pending.is_some()
        {
            return None;
        }
        if !due(
            self.mini_parse.last,
            now,
            self.settings.mini_parse.interval_seconds,
        ) {
            return None;
        }
        let handle = self.handle()?;
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
                    self.mini_parse.error = Some(tr("mini-missing-profile"));
                    self.mini_parse.last = Some(now);
                    self.mini_parse.output = None;
                    self.mini_parse.truncated = false;
                    self.mini_parse.last_ended = None;
                    self.mini_parse.last_ended_profile = None;
                    self.mini_parse.publish();
                    return None;
                }
            }
        };
        let ticket = Ticket {
            session: self.revision.session,
            generation: self.mini_parse.generation,
        };
        self.mini_parse.last = Some(now);
        self.mini_parse.pending = Some(ticket);
        self.mini_parse.pending_settings = Some((
            self.settings.mini_parse.profile.clone(),
            self.settings.mini_parse.interval_seconds,
            profile.clone(),
        ));
        self.mini_parse.publish();
        let last_ended = self.mini_parse.reusable_end(&profile);
        Some((ticket, handle, profile, last_ended))
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
        if self.settings.mini_parse.profile != name
            || self.settings.mini_parse.interval_seconds != interval
            || current != profile
        {
            self.mini_parse.invalidate();
            return false;
        }
        if !self.mini_parse.active(self.screen)
            || self.phase != Phase::Ready
            || self.exit_requested
            || self.revision.session != ticket.session
            || self.mini_parse.generation != ticket.generation
        {
            self.mini_parse.output = None;
            self.mini_parse.truncated = false;
            self.mini_parse.publish();
            return false;
        }
        match result {
            Ok(Some((output, ended))) => {
                self.mini_parse.last_ended = ended;
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
pub(super) async fn poll(mut state: Signal<AppState>) {
    let start = Instant::now();
    loop {
        dioxus_sdk::time::sleep(Duration::from_secs(1)).await;
        {
            let mut current = state.write();
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
        let work = state.write().mini_begin(start.elapsed());
        if let Some((ticket, handle, profile, last_ended)) = work {
            spawn_forever(async move {
                let result = fetch(&handle, profile, last_ended).await;
                state.write().mini_finish(ticket, result);
            });
        }
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
    let output = current.mini_parse.output.clone();
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
                if enabled {
                    pre { class: "max-h-80 overflow-auto whitespace-pre-wrap rounded-md border border-border p-3 text-sm",
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
        add_plugin(&host, "MiniParseEndPlugin").await;
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
