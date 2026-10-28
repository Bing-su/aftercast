mod analysis;
mod avoidance;
mod clipboard_import;
mod companions;
mod death;
mod encounter_context;
mod history;
mod history_sources;
mod import_progress;
mod import_text;
mod interchange;
mod log_window;
mod notifications;
mod plugin_tables;
mod plugins;
mod preferences;
mod reparse;
mod reports;
mod shell;
mod timers;
mod triggers;

use std::time::Duration;

use aftercast::{
    AttackSummary, EncounterDetail, EncounterId, EncounterSummary, HostHandle, ImportProgress,
    PluginConfig, PluginId, PluginStatus,
};
use camino::Utf8PathBuf;
use dioxus::core::spawn_forever;
use dioxus::desktop::{
    WindowCloseBehaviour,
    tao::event::{Event, WindowEvent},
    use_window, use_wry_event_handler,
};
use dioxus::prelude::*;
use dioxus_i18n::prelude::*;
use dioxus_i18n::unic_langid::langid;

use crate::{
    ApplicationRoot,
    config::{ConfigFile, Settings, resolve},
    session::RunningHost,
};

#[derive(Clone, Copy, Default, PartialEq)]
enum Phase {
    #[default]
    Idle,
    Starting,
    Initializing,
    Ready,
    Importing,
    SelectingImport,
    ReadingClipboard,
    Archiving,
    ReadingHistory,
    ReadingTables,
    MakingReport,
    UpdatingTriggers,
    UpdatingTimers,
    ShowingNotification,
    UpdatingNotifications,
    ChangingPlugins,
    ChangingSources,
    Stopping,
}

impl Phase {
    fn key(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Starting => "starting",
            Self::Initializing => "initializing",
            Self::Ready => "ready",
            Self::Importing => "importing",
            Self::SelectingImport => "selecting-import",
            Self::ReadingClipboard => "reading-clipboard",
            Self::Archiving => "archiving",
            Self::ReadingHistory => "reading-history",
            Self::ReadingTables => "reading-tables",
            Self::MakingReport => "making-report",
            Self::UpdatingTriggers => "updating-triggers",
            Self::UpdatingTimers => "updating-timers",
            Self::ShowingNotification => "showing-notification",
            Self::UpdatingNotifications => "updating-notifications",
            Self::ChangingPlugins => "changing-plugins",
            Self::ChangingSources => "changing-sources",
            Self::Stopping => "stopping",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Revision {
    session: u64,
    selection: u64,
}

#[derive(Clone)]
struct LoadedPlugin {
    path: String,
    id: PluginId,
    status: String,
    info: PluginStatus,
}

struct AppState {
    root: Utf8PathBuf,
    settings: Settings,
    file: Option<ConfigFile>,
    saving: bool,
    phase: Phase,
    screen: &'static str,
    running: Option<RunningHost>,
    log_windows: std::sync::Arc<log_window::WindowLifetime>,
    plugins: Vec<LoadedPlugin>,
    triggers: triggers::EditorState,
    timers: timers::EditorState,
    history: history::BrowserState,
    history_sources: history_sources::State,
    plugin_tables: plugin_tables::BrowserState,
    notifications: notifications::NotificationState,
    plugin_ui_pending: bool,
    startup_failed: bool,
    failed_plugin: Option<String>,
    encounters: Vec<EncounterSummary>,
    total: u32,
    offset: u32,
    detail: Option<EncounterDetail>,
    detail_loading: bool,
    attacks: Vec<AttackSummary>,
    combatant: Option<String>,
    category: Option<(String, bool)>,
    selected: Option<EncounterId>,
    loading: bool,
    revision: Revision,
    cancel_import: bool,
    import_path: String,
    import_text: import_text::State,
    interchange: interchange::State,
    reports: reports::State,
    companions: companions::State,
    reparse: reparse::ReparseState,
    clipboard: clipboard_import::State,
    progress: ImportProgress,
    import_progress: import_progress::State,
    message: String,
    error: Option<String>,
    exit_requested: bool,
}

impl AppState {
    fn can_toggle_plugin(&self) -> bool {
        matches!(self.phase, Phase::Idle | Phase::Ready)
            && !self.saving
            && !self.plugin_ui_pending
            && self.file.is_some()
    }

    fn can_save_settings(&self) -> bool {
        !self.saving && self.file.is_some() && self.phase != Phase::ChangingPlugins
    }

    fn begin_settings_save(&mut self) -> Option<(Utf8PathBuf, ConfigFile)> {
        if !self.can_save_settings() {
            return None;
        }
        let mut file = self.file.clone()?;
        file.settings = self.settings.clone();
        self.saving = true;
        Some((self.root.clone(), file))
    }

    fn receive_settings_save(&mut self, result: miette::Result<ConfigFile>, message: String) {
        self.saving = false;
        match result {
            Ok(file) => {
                self.file = Some(file);
                self.message = message;
            }
            Err(error) => self.fail_report(error),
        }
    }

    fn new(root: Utf8PathBuf) -> Self {
        Self {
            root,
            settings: Settings::default(),
            file: None,
            saving: false,
            phase: Phase::Idle,
            screen: "analysis",
            running: None,
            log_windows: Default::default(),
            plugins: Vec::new(),
            triggers: Default::default(),
            timers: Default::default(),
            history: Default::default(),
            history_sources: Default::default(),
            plugin_tables: Default::default(),
            notifications: Default::default(),
            plugin_ui_pending: false,
            startup_failed: false,
            failed_plugin: None,
            encounters: Vec::new(),
            total: 0,
            offset: 0,
            detail: None,
            detail_loading: false,
            attacks: Vec::new(),
            combatant: None,
            category: None,
            selected: None,
            loading: false,
            revision: Revision::default(),
            cancel_import: false,
            import_path: String::new(),
            import_text: Default::default(),
            interchange: Default::default(),
            reports: Default::default(),
            companions: Default::default(),
            reparse: Default::default(),
            clipboard: Default::default(),
            progress: ImportProgress::default(),
            import_progress: Default::default(),
            message: String::new(),
            error: None,
            exit_requested: false,
        }
    }

    fn handle(&self) -> Option<HostHandle> {
        self.running.as_ref().map(|running| running.host.handle())
    }

    fn receive_detail(
        &mut self,
        stamp: Revision,
        result: std::result::Result<Option<EncounterDetail>, aftercast::Error>,
    ) {
        if self.revision != stamp {
            return;
        }
        self.detail_loading = false;
        match result {
            Ok(detail) => self.detail = detail,
            Err(error) => self.fail(error),
        }
    }

    fn fail(&mut self, error: impl std::fmt::Display) {
        let text = error.to_string();
        tracing::error!(error = %text, "desktop operation failed");
        self.error = Some(text);
    }

    fn fail_session(&mut self, session: u64, error: impl std::fmt::Display) {
        // A late diagnostics/clipboard failure must not appear after the user stops or replaces its Host.
        if self.revision.session == session {
            self.fail(error);
        }
    }

    fn fail_report(&mut self, error: miette::Report) {
        // Preserve causes in GUI text, e.g. a TOML path and parse error, without terminal ANSI styling.
        self.fail(
            error
                .chain()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(": "),
        );
    }
}

fn tr(key: &str) -> String {
    i18n().translate(key)
}

#[component]
pub fn App() -> Element {
    let ApplicationRoot(root) = use_context::<ApplicationRoot>();
    let mut state = use_signal(|| AppState::new(root));
    use_context_provider(|| state);
    use_context_provider(clipboard_import::Adapter::default);
    let locale = try_use_init_i18n(|| {
        I18nConfig::new(langid!("ko-KR"))
            .with_fallback(langid!("en-US"))
            .with_locale((langid!("ko-KR"), include_str!("../locales/ko-KR.ftl")))
            .with_locale((langid!("en-US"), include_str!("../locales/en-US.ftl")))
    });
    let progress_lifetime = state.peek().import_progress.hub.clone();
    use_drop(move || progress_lifetime.close());
    let window = use_window();
    let window_id = window.id();
    use_wry_event_handler(move |event, _| {
        if matches!(event, Event::WindowEvent { window_id: id, event: WindowEvent::CloseRequested, .. } if *id == window_id)
        {
            let mut state = state;
            state.write().exit_requested = true;
        }
    });
    use_future(move || load_settings(state, i18n()));
    use_future(move || notifications::poll(state));
    use_future(move || companions::poll(state));
    use_future(move || {
        let window = window.clone();
        async move {
            let mut state = state;
            loop {
                dioxus_sdk::time::sleep(Duration::from_millis(100)).await;
                if !state.read().exit_requested {
                    continue;
                }
                let phase = state.read().phase;
                match phase {
                    Phase::ReadingClipboard => {
                        state.write().abandon_clipboard_read_for_exit();
                    }
                    Phase::Ready
                    | Phase::Initializing
                    | Phase::Importing
                    | Phase::SelectingImport
                    | Phase::Archiving
                    | Phase::ReadingHistory
                    | Phase::ReadingTables
                    | Phase::MakingReport
                    | Phase::ChangingSources
                    | Phase::ChangingPlugins
                    | Phase::UpdatingTimers
                    | Phase::UpdatingTriggers
                    | Phase::ShowingNotification
                    | Phase::UpdatingNotifications => {
                        stop(state);
                    }
                    Phase::Idle if !state.read().saving => {
                        state.read().log_windows.close();
                        state.read().import_progress.close();
                        window.set_close_behavior(WindowCloseBehaviour::WindowCloses);
                        window.close();
                        break;
                    }
                    _ => {}
                }
                if state.read().phase == Phase::Importing {
                    state.write().cancel_import = true;
                }
            }
        }
    });
    if let Err(error) = locale {
        return rsx! {
            pre { role: "alert", "{error}" }
        };
    }
    rsx! {
        style { {include_str!("../assets/tailwind.css")} }
        div {
            class: "{state.read().settings.ui.theme} app-shell flex h-dvh flex-col bg-background text-foreground [color-scheme:dark] [&.latte]:[color-scheme:light]",
            "data-theme": state.read().settings.ui.theme.clone(),
            lang: state.read().settings.ui.language.clone(),
            shell::Header {}
            if let Some(error) = state.read().error.clone() {
                div {
                    class: "flex items-center gap-5 bg-ctp-red/15 px-6 py-3 text-ctp-red [&_strong]:shrink-0 [&_pre]:max-h-30 [&_pre]:flex-1 [&_pre]:overflow-auto [&_pre]:whitespace-pre-wrap [&_pre]:font-sans [&_pre]:text-xs",
                    role: "alert",
                    strong { {tr("operation-failed")} }
                    pre { "{error}" }
                    crate::components::Button {
                        variant: crate::components::ButtonVariant::Ghost,
                        onclick: move |_| state.write().error = None,
                        {tr("dismiss")}
                    }
                }
            }
            div { class: "grid min-h-0 flex-1 grid-cols-[210px_minmax(0,1fr)] min-[1100px]:grid-cols-[265px_minmax(0,1fr)] min-[1280px]:grid-cols-[330px_minmax(0,1fr)]",
                shell::Sidebar {}
                main { class: "min-w-0 overflow-auto",
                    match state.read().screen {
                        "settings" => rsx! {
                            preferences::SettingsPane {}
                        },
                        "plugins" => rsx! {
                            plugins::PluginsPane {}
                        },
                        "history" => rsx! {
                            history::HistoryPane {}
                        },
                        "timers" => rsx! {
                            timers::TimersPane {}
                        },
                        "triggers" => rsx! {
                            triggers::TriggersPane {}
                        },
                        "notifications" => rsx! {
                            notifications::NotificationsPane {}
                        },
                        "import" => rsx! {
                            analysis::ImportPane {}
                        },
                        _ => rsx! {
                            analysis::AnalysisPane {}
                        },
                    }
                }
            }
            shell::Footer {}
        }
    }
}

async fn load_settings(mut state: Signal<AppState>, mut language: I18n) {
    if state.read().phase != Phase::Idle || state.read().saving {
        return;
    }
    state.write().saving = true;
    state.write().file = None;
    let root = state.read().root.clone();
    match tokio::task::spawn_blocking(move || ConfigFile::load(&root)).await {
        Ok(Ok(file)) => {
            let language_id = file
                .settings
                .ui
                .language
                .parse::<dioxus_i18n::unic_langid::LanguageIdentifier>();
            match language_id
                .map_err(|error| error.to_string())
                .and_then(|id| {
                    language
                        .try_set_language(id)
                        .map_err(|error| error.to_string())
                }) {
                Ok(()) => {
                    let mut state = state.write();
                    state.settings = file.settings.clone();
                    let graphs = state.settings.graphs.clone();
                    state.restore_report_graphs(&graphs);
                    state.file = Some(file);
                }
                Err(error) => state.write().fail(error),
            }
        }
        Ok(Err(error)) => state.write().fail_report(error),
        Err(error) => state.write().fail(error),
    }
    state.write().saving = false;
}

fn start(mut state: Signal<AppState>) {
    if state.read().phase != Phase::Idle || state.read().file.is_none() {
        return;
    }
    let (root, settings) = {
        let state = state.read();
        (state.root.clone(), state.settings.clone())
    };
    state.write().phase = Phase::Starting;
    state.write().startup_failed = false;
    state.write().failed_plugin = None;
    state.write().error = None;
    state.write().revision.session += 1;
    let session = state.read().revision.session;
    spawn(async move {
        start_session(state, root, settings, session).await;
    });
}

async fn start_session(
    mut state: Signal<AppState>,
    root: Utf8PathBuf,
    settings: Settings,
    session: u64,
) {
    let running = match RunningHost::start(root.clone(), settings.clone()).await {
        Ok(running) => running,
        Err(error) => {
            state.write().phase = Phase::Idle;
            state.write().startup_failed = true;
            state.write().fail_report(error);
            return;
        }
    };
    let handle = running.host.handle();
    state.write().running = Some(running);
    state.write().phase = Phase::Initializing;
    for path in settings.host.enabled_plugins() {
        if state.read().exit_requested || state.read().revision.session != session {
            break;
        }
        let result = async {
            let plugin = handle
                .create_plugin(
                    PluginConfig::builder()
                        .path(resolve(&root, path))
                        .maybe_type_name(
                            settings
                                .host
                                .plugin_types
                                .get(path)
                                .cloned()
                                .filter(|name| !name.trim().is_empty()),
                        )
                        .build(),
                )
                .await?;
            let status = handle.initialize_plugin(plugin).await?;
            let info = handle.plugin_status(plugin).await?;
            Ok::<_, aftercast::Error>(LoadedPlugin {
                path: path.clone(),
                id: plugin,
                status,
                info,
            })
        }
        .await;
        if state.read().revision.session != session {
            return;
        }
        match result {
            Ok(plugin) => state.write().plugins.push(plugin),
            Err(error) => {
                state.write().startup_failed = true;
                state.write().failed_plugin = Some(path.clone());
                state.write().fail(error);
                stop(state);
                return;
            }
        }
    }
    if state.read().revision.session == session {
        state.write().phase = Phase::Ready;
    }
}

fn stop(mut state: Signal<AppState>) {
    let running = state.write().running.take();
    let Some(running) = running else {
        return;
    };
    {
        let mut state = state.write();
        state.log_windows.invalidate();
        state.import_progress.retire();
        state.phase = Phase::Stopping;
        state.revision.session += 1;
        state.selected = None;
        state.detail = None;
        state.detail_loading = false;
        state.attacks.clear();
        state.combatant = None;
        state.category = None;
        state.encounters.clear();
        state.plugins.clear();
        state.triggers = Default::default();
        state.timers = Default::default();
        state.history_sources.stop();
        state.history = Default::default();
        state.plugin_tables.reset_session();
        state.reparse.reset_session();
        state.clipboard.reset_session();
        state.import_text.reset_session();
        state.interchange.stop();
        state.reports.stop();
        state.companions.reset_session();
        state.notifications = Default::default();
        state.plugin_ui_pending = false;
        state.loading = false;
    }
    spawn(async move {
        let result = running.shutdown().await;
        state.write().phase = Phase::Idle;
        if let Err(error) = result {
            state.write().fail_report(error);
            state.write().exit_requested = false;
            dioxus::desktop::window().set_visible(true);
        }
    });
}

fn refresh(mut state: Signal<AppState>, offset: u32) {
    if state.read().phase != Phase::Ready || state.read().loading {
        return;
    }
    let Some(handle) = state.read().handle() else {
        return;
    };
    let session = state.read().revision.session;
    state.write().loading = true;
    spawn(async move {
        let result = handle.encounters(offset, 128).await;
        if state.read().revision.session != session {
            return;
        }
        state.write().loading = false;
        match result {
            Ok(page) => {
                let mut state = state.write();
                state.encounters = page.items;
                state.total = page.total;
                state.offset = offset;
            }
            Err(error) => state.write().fail(error),
        }
    });
}

fn select_encounter(mut state: Signal<AppState>, id: EncounterId) {
    let Some(handle) = state.read().handle() else {
        return;
    };
    let stamp = {
        let mut state = state.write();
        state.selected = Some(id);
        state.detail = None;
        state.detail_loading = true;
        state.attacks.clear();
        state.combatant = None;
        state.category = None;
        state.revision.selection += 1;
        state.screen = "analysis";
        state.revision
    };
    spawn(async move {
        let result = load_encounter(state, handle, id, stamp).await;
        state.write().receive_detail(stamp, result);
    });
}

async fn load_encounter(
    mut state: Signal<AppState>,
    handle: HostHandle,
    id: EncounterId,
    stamp: Revision,
) -> std::result::Result<Option<EncounterDetail>, aftercast::Error> {
    let detail = handle.encounter_detail(id).await?;
    let mut attacks = Vec::new();
    // Page ACT categories without blocking navigation, e.g. switching encounters cancels this read.
    while detail.is_some() && state.read().revision == stamp {
        let Some(page) = handle
            .encounter_attacks(id, attacks.len() as u32, 256)
            .await?
        else {
            return Ok(None);
        };
        let done = page.items.is_empty() || attacks.len() + page.items.len() >= page.total as usize;
        attacks.extend(page.items);
        if done {
            break;
        }
    }
    if state.read().revision == stamp {
        state.write().attacks = attacks;
    }
    Ok(detail)
}

fn import_log(mut state: Signal<AppState>) {
    let Some(selection) = state.write().select_full_import() else {
        return;
    };
    let language = i18n();
    let configured = state.peek().import_path.trim().to_owned();
    // Own file selection and cleanup at App scope; duplicate clicks cannot open another picker.
    spawn_forever(async move {
        let current = state.read().current_full_selection(selection.token);
        if !current {
            state.write().abandon_full_selection(selection.token);
            return;
        }

        let path = if configured.is_empty() {
            let Some(file) = rfd::AsyncFileDialog::new().pick_file().await else {
                state.write().abandon_full_selection(selection.token);
                return;
            };
            match Utf8PathBuf::from_path_buf(file.path().to_owned()) {
                Ok(path) => path,
                Err(path) => {
                    state.write().reject_full_selection(
                        selection.token,
                        format!("Non-UTF-8 path: {}", path.display()),
                    );
                    return;
                }
            }
        } else {
            resolve(&state.peek().root, &configured)
        };
        let handle = state.read().handle();
        let Some(handle) = handle else {
            state.write().abandon_full_selection(selection.token);
            return;
        };
        let admission = state.write().admit_full_path(selection.token, path.clone());
        let Some(request) = admission else {
            state.write().abandon_full_selection(selection.token);
            return;
        };
        let result = match import_text::start_selected(
            &handle,
            &path,
            None,
            request.options(),
            request.sources.as_ref(),
            request.viewpoint.as_ref(),
        )
        .await
        {
            Ok(job) => wait_import(state, job, selection.token.session).await,
            Err(error) => Err(error),
        };
        let received = state.write().receive_full_import(
            selection.token,
            result,
            &language.translate("import-complete"),
            &language.translate("import-cancelled"),
        );
        if received {
            refresh(state, 0);
        }
    });
}

async fn wait_import(
    state: Signal<AppState>,
    job: aftercast::ImportJob,
    session: u64,
) -> Result<aftercast::ImportReport, aftercast::Error> {
    import_progress::wait(state, job, session).await
}

fn plugin_ui_action(mut state: Signal<AppState>, id: PluginId, show: bool) {
    if state.read().phase != Phase::Ready || state.read().plugin_ui_pending {
        return;
    }
    let Some(handle) = state.read().handle() else {
        return;
    };
    let session = state.read().revision.session;
    state.write().plugin_ui_pending = true;
    let opened_message = tr("plugin-settings-opened");
    // UI requests belong to the app, e.g. changing tabs must not strand the pending state.
    spawn_forever(async move {
        let opened = if show {
            Some(handle.open_plugin_settings(id).await)
        } else {
            None
        };
        let status = handle.plugin_status(id).await;
        if state.read().revision.session != session {
            return;
        }
        state.write().plugin_ui_pending = false;
        match status {
            Ok(info) => {
                if let Some(plugin) = state
                    .write()
                    .plugins
                    .iter_mut()
                    .find(|plugin| plugin.id == id)
                {
                    plugin.info = info;
                }
            }
            Err(error) => state.write().fail(error),
        }
        match opened {
            Some(Ok(())) => state.write().message = opened_message,
            Some(Err(error)) => state.write().fail(error),
            None => {}
        }
    });
}

fn archive_history(mut state: Signal<AppState>, restore: bool) {
    if state.read().phase != Phase::Ready || state.read().loading {
        return;
    }
    let Some(handle) = state.read().handle() else {
        return;
    };
    let session = state.read().revision.session;
    if restore {
        state.write().invalidate_restored_context();
    }
    state.write().phase = Phase::Archiving;
    let message = tr(if restore {
        "history-restored"
    } else {
        "history-saved"
    });
    // History I/O belongs to the application, e.g. changing tabs must not abandon a pending checkpoint.
    spawn_forever(async move {
        let result = if restore {
            handle.restore_history("").await
        } else {
            handle.save_history().await
        };
        if state.read().revision.session != session {
            return;
        }
        // Also invalidate a window queued during restore, before its result became known.
        if restore {
            state.write().invalidate_restored_context();
        }
        state.write().phase = Phase::Ready;
        match result {
            Ok(count) => {
                let mut current = state.write();
                current.message = format!("{message}: {count}");
                current.history.invalidate_snapshot();
            }
            Err(error) => state.write().fail(error),
        }
        refresh(state, 0);
    });
}

fn save_settings(mut state: Signal<AppState>) {
    let Some((root, file)) = state.write().begin_settings_save() else {
        return;
    };
    let saved_message = tr("settings-saved");
    // Settings belong to the app: changing panes must not cancel I/O and strand the global saving flag.
    spawn_forever(async move {
        let result = tokio::task::spawn_blocking(move || file.save(&root)).await;
        let result = result
            .map_err(|error| miette::miette!(error))
            .and_then(|result| result);
        state.write().receive_settings_save(result, saved_message);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_responses_are_rejected_for_both_session_and_selection() {
        let request = Revision {
            session: 1,
            selection: 3,
        };
        let mut state = AppState::new(".".into());
        state.detail_loading = true;
        state.revision = Revision {
            session: 1,
            selection: 4,
        };
        state.receive_detail(request, Err(aftercast::Error::Closed));
        assert!(state.error.is_none());
        assert!(state.detail_loading);
        state.revision = Revision {
            session: 2,
            selection: 3,
        };
        state.receive_detail(request, Err(aftercast::Error::Closed));
        assert!(state.error.is_none());
        let current = state.revision;
        state.receive_detail(current, Err(aftercast::Error::Closed));
        assert!(state.error.is_some());
        assert!(!state.detail_loading);
    }

    #[test]
    fn pending_plugin_toggle_cannot_persist_the_previous_enablement() {
        let directory = tempfile::tempdir().expect("settings directory");
        let root = camino::Utf8Path::from_path(directory.path()).expect("UTF-8 temp path");
        let mut state = AppState::new(root.to_owned());
        state.file = Some(ConfigFile::load(root).expect("new config"));
        state.settings.host.plugins.push("fixture.dll".to_owned());
        for enabled in [false, true] {
            state.phase = Phase::ChangingPlugins;
            assert!(state.begin_settings_save().is_none());
            assert!(!state.saving);
            // Complete the lifecycle before saving, as the async result handler does.
            state.phase = Phase::Ready;
            if enabled {
                state.settings.host.disabled_plugins.clear();
            } else {
                state
                    .settings
                    .host
                    .disabled_plugins
                    .push("fixture.dll".to_owned());
            }
            let (root, file) = state.begin_settings_save().expect("save settled selection");
            assert!(
                state.begin_settings_save().is_none(),
                "no overlapping saves"
            );
            state.file = Some(file.save(&root).expect("persist result"));
            state.saving = false;
            let reloaded = ConfigFile::load(&root).expect("reload persisted config");
            assert_eq!(
                reloaded.settings.host.enabled_plugins().count(),
                usize::from(enabled)
            );
        }
    }

    #[test]
    fn embedded_locales_are_valid_and_have_the_same_messages() {
        use dioxus_i18n::fluent::FluentResource;
        let english = FluentResource::try_new(include_str!("../locales/en-US.ftl").into())
            .expect("English FTL");
        let korean = FluentResource::try_new(include_str!("../locales/ko-KR.ftl").into())
            .expect("Korean FTL");
        let count = |resource: &FluentResource| resource.entries().count();
        assert_eq!(count(&english), count(&korean));
    }

    #[test]
    fn gui_diagnostics_preserve_causes_without_terminal_rendering() {
        let mut state = AppState::new(".".into());
        state.fail_report(
            miette::miette!("unsupported schema_version").wrap_err("Read config.toml"),
        );
        assert_eq!(
            state.error.as_deref(),
            Some("Read config.toml: unsupported schema_version")
        );
    }

    #[test]
    fn late_diagnostics_and_clipboard_errors_do_not_cross_host_sessions() {
        let mut state = AppState::new(".".into());
        state.revision.session = 2;
        state.fail_session(1, aftercast::Error::Closed);
        assert!(state.error.is_none());
        state.fail_session(2, aftercast::Error::Closed);
        assert_eq!(state.error.as_deref(), Some("the host is closed"));
    }
}
