use super::{AppState, Phase, load_settings, save_settings, tr};
use crate::components::{Button, ButtonVariant, Input, Label, Separator, TextArea};
use dioxus::core::spawn_forever;
use dioxus::prelude::*;
use dioxus_i18n::prelude::*;

impl AppState {
    fn receive_selective_apply(
        &mut self,
        session: u64,
        result: Result<(), aftercast::Error>,
        message: String,
    ) -> bool {
        if self.revision.session != session || self.phase != Phase::UpdatingCorrections {
            return false;
        }
        self.phase = Phase::Ready;
        match result {
            Ok(()) => self.message = message,
            Err(error) => self.fail(error),
        }
        true
    }
}

#[component]
pub(super) fn SettingsPane() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mut language = i18n();
    let disabled = state.read().saving || state.read().file.is_none();
    rsx! {
        section { class: "max-w-3xl p-6 min-[1000px]:px-8 min-[1000px]:py-7",
            div { class: "mb-7 flex items-start justify-between gap-5 [&_h1]:text-2xl [&_h1]:font-semibold [&_h1]:tracking-tight [&_p]:mt-2 [&_p]:max-w-[68ch] [&_p]:text-[13px] [&_p]:leading-7 [&_p]:text-muted-foreground",
                div {
                    h1 { {tr("settings")} }
                    p { {tr("settings-description")} }
                }
            }
            div { class: "[&_h2]:mb-4 [&_h2]:text-[15px] [&_h2]:font-semibold",
                h2 { {tr("appearance")} }
                Label {
                    class: "mt-4 mb-2 text-[13px]".to_owned(),
                    html_for: "language",
                    {tr("language")}
                }
                select {
                    id: "language",
                    class: "h-9 w-65 max-w-full rounded-md border border-border bg-background px-2.5",
                    value: state.read().settings.ui.language.clone(),
                    disabled,
                    onchange: move |event| {
                        let value = event.value();
                        match value
                            .parse()
                            .map_err(|error| format!("{error}"))
                            .and_then(|id| {
                                language.try_set_language(id).map_err(|error| error.to_string())
                            })
                        {
                            Ok(()) => state.write().settings.ui.language = value,
                            Err(error) => state.write().fail(error),
                        }
                    },
                    option { value: "ko-KR", "한국어" }
                    option { value: "en-US", "English" }
                }
                Label {
                    class: "mt-4 mb-2 text-[13px]".to_owned(),
                    html_for: "theme",
                    {tr("theme")}
                }
                select {
                    id: "theme",
                    class: "h-9 w-65 max-w-full rounded-md border border-border bg-background px-2.5",
                    value: state.read().settings.ui.theme.clone(),
                    disabled,
                    onchange: move |event| state.write().settings.ui.theme = event.value(),
                    option { value: "mocha", "Catppuccin Mocha" }
                    option { value: "macchiato", "Catppuccin Macchiato" }
                    option { value: "frappe", "Catppuccin Frappé" }
                    option { value: "latte", "Catppuccin Latte" }
                }
            }
            Separator { class: "my-6".to_owned() }
            div { class: "[&_h2]:mb-4 [&_h2]:text-[15px] [&_h2]:font-semibold",
                h2 { {tr("runtime")} }
                Label {
                    class: "mt-4 mb-2 text-[13px]".to_owned(),
                    html_for: "dotnet-root",
                    {tr("dotnet-root")}
                }
                Input {
                    id: "dotnet-root".to_owned(),
                    value: state.read().settings.host.dotnet_root.clone(),
                    disabled: disabled || state.read().phase != Phase::Idle,
                    oninput: move |event: FormEvent| state.write().settings.host.dotnet_root = event.value(),
                }
                p { class: "mt-2 text-xs text-muted-foreground", {tr("runtime-description")} }
            }
            super::appearance::Controls {}
            Separator { class: "my-6".to_owned() }
            div { class: "space-y-3",
                h2 { class: "text-sm font-semibold", {tr("audio-settings")} }
                p { class: "text-xs text-muted-foreground", {tr("audio-provider-unavailable")} }
                MiscSoundPreferences {}
                TimerCueControl { slot: "start", label: "audio-timer-start" }
                TimerCueControl { slot: "warning", label: "audio-timer-warning" }
                TimerCueControl { slot: "expire", label: "audio-timer-expire" }
                p { class: "text-xs text-muted-foreground", {tr("audio-timer-help")} }
                for (id, label, volume) in [
                    ("sound-volume", "audio-sound-volume", state.read().settings.audio.sound_volume),
                    ("tts-volume", "audio-tts-volume", state.read().settings.audio.tts_volume),
                ]
                {
                    div { class: "flex flex-wrap items-center justify-between gap-3",
                        Label { html_for: id, {tr(label)} }
                        select {
                            id,
                            value: volume.to_string(),
                            disabled: disabled || state.read().phase != Phase::Idle,
                            class: "h-9 rounded-md border border-border bg-background px-2",
                            onchange: move |event| {
                                if state.peek().phase != Phase::Idle || state.peek().saving
                                    || state.peek().file.is_none()
                                {
                                    return;
                                }
                                if let Ok(value) = event.value().parse::<u8>() && value <= 100 {
                                    if id == "sound-volume" {
                                        state.write().settings.audio.sound_volume = value;
                                    } else {
                                        state.write().settings.audio.tts_volume = value;
                                    }
                                }
                            },
                            for value in 0..=100_u8 {
                                option { value: value.to_string(), "{value}%" }
                            }
                        }
                    }
                }
            }
            Separator { class: "my-6".to_owned() }
            div { class: "space-y-4",
                h2 { class: "text-sm font-semibold", {tr("import-defaults")} }
                div { class: "flex flex-wrap items-center justify-between gap-3",
                    Label { html_for: "import-extend-default", {tr("import-extend-default")} }
                    Button {
                        id: "import-extend-default".to_owned(),
                        variant: ButtonVariant::Outline,
                        aria_label: tr("import-extend-default"),
                        aria_pressed: state.read().settings.ui.import_extend_default,
                        disabled,
                        onclick: move |_| {
                            let current = state.peek().settings.ui.import_extend_default;
                            state.write().settings.ui.import_extend_default = !current;
                        },
                        {
                            tr(
                                if state.read().settings.ui.import_extend_default {
                                    "enabled"
                                } else {
                                    "disabled"
                                },
                            )
                        }
                    }
                }
                div { class: "flex flex-wrap items-center justify-between gap-3",
                    Label { html_for: "export-include-logs-default",
                        {tr("export-include-logs-default")}
                    }
                    Button {
                        id: "export-include-logs-default".to_owned(),
                        variant: ButtonVariant::Outline,
                        aria_label: tr("export-include-logs-default"),
                        aria_pressed: state.read().settings.ui.export_include_logs_default,
                        disabled,
                        onclick: move |_| {
                            let current = state.peek().settings.ui.export_include_logs_default;
                            state.write().settings.ui.export_include_logs_default = !current;
                        },
                        {
                            tr(
                                if state.read().settings.ui.export_include_logs_default {
                                    "enabled"
                                } else {
                                    "disabled"
                                },
                            )
                        }
                    }
                }
                div { class: "flex flex-wrap items-center justify-between gap-3",
                    Label { html_for: "archive-trigger-results-default",
                        {tr("act-import-trigger-results")}
                    }
                    Button {
                        id: "archive-trigger-results-default".to_owned(),
                        variant: ButtonVariant::Outline,
                        aria_label: tr("act-import-trigger-results"),
                        aria_pressed: state.read().settings.ui.archive_trigger_results_default,
                        disabled,
                        onclick: move |_| {
                            let current = state.peek().settings.ui.archive_trigger_results_default;
                            state.write().settings.ui.archive_trigger_results_default = !current;
                        },
                        {
                            tr(
                                if state.read().settings.ui.archive_trigger_results_default {
                                    "enabled"
                                } else {
                                    "disabled"
                                },
                            )
                        }
                    }
                }
                p { class: "text-xs leading-6 text-muted-foreground", {tr("import-defaults-help")} }
            }
            Separator { class: "my-6".to_owned() }
            div { class: "space-y-4",
                h2 { class: "text-sm font-semibold", {tr("encounter-recording")} }
                div { class: "flex flex-wrap items-center justify-between gap-3",
                    Label { html_for: "record-encounter-logs", {tr("record-encounter-logs")} }
                    Button {
                        id: "record-encounter-logs".to_owned(),
                        variant: ButtonVariant::Outline,
                        aria_label: tr("record-encounter-logs"),
                        aria_pressed: state.read().settings.host.record_encounter_logs,
                        disabled: disabled || state.read().phase != Phase::Idle,
                        onclick: move |_| {
                            let enabled = state.peek().settings.host.record_encounter_logs;
                            state.write().settings.host.record_encounter_logs = !enabled;
                        },
                        {
                            tr(
                                if state.read().settings.host.record_encounter_logs {
                                    "enabled"
                                } else {
                                    "disabled"
                                },
                            )
                        }
                    }
                }
                div { class: "flex flex-wrap items-center justify-between gap-3",
                    Label { html_for: "original-logline-view-logs", {tr("original-logline-view-logs")} }
                    Button {
                        id: "original-logline-view-logs".to_owned(),
                        variant: ButtonVariant::Outline,
                        aria_label: tr("original-logline-view-logs"),
                        aria_pressed: state.read().settings.host.original_logline_view_logs,
                        disabled: disabled || state.read().phase != Phase::Idle,
                        onclick: move |_| {
                            let original = state.peek().settings.host.original_logline_view_logs;
                            state.write().settings.host.original_logline_view_logs = !original;
                        },
                        {
                            tr(
                                if state.read().settings.host.original_logline_view_logs {
                                    "enabled"
                                } else {
                                    "disabled"
                                },
                            )
                        }
                    }
                }
                p { class: "text-xs leading-6 text-muted-foreground",
                    {tr("recording-settings-help")}
                }
            }
            Separator { class: "my-6".to_owned() }
            div { class: "space-y-3",
                Label { html_for: "selective-mode", {tr("selective-mode")} }
                select {
                    id: "selective-mode",
                    class: "h-9 max-w-full rounded-md border border-border bg-background px-2",
                    disabled: disabled || !matches!(state.read().phase, Phase::Idle | Phase::Ready),
                    value: format!("{:?}", state.read().settings.host.selective_mode),
                    onchange: move |event: FormEvent| {
                        if !matches!(state.read().phase, Phase::Idle | Phase::Ready)
                            || state.read().saving || state.read().file.is_none()
                        {
                            return;
                        }
                        let mode = match event.value().as_str() {
                            "GraphsExports" => aftercast::SelectiveMode::GraphsExports,
                            "GraphsExportsAndAlliedTotals" => {
                                aftercast::SelectiveMode::GraphsExportsAndAlliedTotals
                            }
                            "Encounters" => aftercast::SelectiveMode::Encounters,
                            "EncountersStrict" => aftercast::SelectiveMode::EncountersStrict,
                            _ => aftercast::SelectiveMode::None,
                        };
                        state.write().settings.host.selective_mode = mode;
                    },
                    for (value, label) in [
                        ("None", "selective-none"),
                        ("GraphsExports", "selective-exports"),
                        ("GraphsExportsAndAlliedTotals", "selective-exports-allies"),
                        ("Encounters", "selective-record"),
                        ("EncountersStrict", "selective-record-strict"),
                    ]
                    {
                        option { value, {tr(label)} }
                    }
                }
                Label { html_for: "selective-names", {tr("selective-names")} }
                TextArea {
                    id: "selective-names".to_owned(),
                    value: state.read().settings.host.selective_names.clone(),
                    disabled: disabled || !matches!(state.read().phase, Phase::Idle | Phase::Ready),
                    oninput: move |event: FormEvent| {
                        if matches!(state.read().phase, Phase::Idle | Phase::Ready)
                            && !state.read().saving && state.read().file.is_some()
                        {
                            state.write().settings.host.selective_names = event.value();
                        }
                    },
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: disabled || state.read().phase != Phase::Ready,
                    onclick: move |_| {
                        let mut current = state.write();
                        if current.phase != Phase::Ready || current.saving || current.file.is_none() {
                            return;
                        }
                        let Some(handle) = current.handle() else {
                            return;
                        };
                        let session = current.revision.session;
                        let options = current.settings.host.selective_options();
                        current.phase = Phase::UpdatingCorrections;
                        let message = tr("selective-applied");
                        drop(current);
                        spawn_forever(async move {
                            let result = handle.apply_selective_options(&options).await;
                            let mut current = state.write();
                            current.receive_selective_apply(session, result, message);
                        });
                    },
                    {tr("selective-apply")}
                }
                p { class: "text-xs leading-6 text-muted-foreground", {tr("selective-help")} }
            }
            Separator { class: "my-6".to_owned() }
            div { class: "space-y-3",
                h2 { class: "text-sm font-semibold", {tr("zero-hit-heading")} }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: disabled || state.read().phase != Phase::Idle,
                    aria_pressed: state.read().settings.host.count_zero_damage_as_hit,
                    onclick: move |_| {
                        if state.read().phase != Phase::Idle {
                            return;
                        }
                        let current = state.peek().settings.host.count_zero_damage_as_hit;
                        state.write().settings.host.count_zero_damage_as_hit = !current;
                    },
                    {
                        tr(
                            if state.read().settings.host.count_zero_damage_as_hit {
                                "enabled"
                            } else {
                                "disabled"
                            },
                        )
                    }
                }
                p { class: "text-xs leading-6 text-muted-foreground", {tr("zero-hit-help")} }
            }
            Separator { class: "my-6".to_owned() }
            div { class: "space-y-3",
                h2 { class: "text-sm font-semibold", {tr("idle-end-heading")} }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: disabled || state.read().phase != Phase::Idle,
                    aria_pressed: state.read().settings.host.idle_end_enabled,
                    onclick: move |_| {
                        let current = state.peek().settings.host.idle_end_enabled;
                        state.write().settings.host.idle_end_enabled = !current;
                    },
                    {
                        tr(
                            if state.read().settings.host.idle_end_enabled {
                                "enabled"
                            } else {
                                "disabled"
                            },
                        )
                    }
                }
                div { class: "flex items-center gap-2",
                    Label { html_for: "idle-timer-toggle", {tr("idle-timer-heading")} }
                    Button {
                        id: "idle-timer-toggle",
                        variant: ButtonVariant::Outline,
                        disabled: disabled || state.read().phase != Phase::Idle
                            || !state.read().settings.host.idle_end_enabled,
                        aria_pressed: state.read().settings.host.idle_timer_enabled,
                        onclick: move |_| {
                            let current = state.peek().settings.host.idle_timer_enabled;
                            state.write().settings.host.idle_timer_enabled = !current;
                        },
                        {
                            tr(
                                if state.read().settings.host.idle_timer_enabled {
                                    "enabled"
                                } else {
                                    "disabled"
                                },
                            )
                        }
                    }
                }
                Label { html_for: "idle-end-seconds", {tr("idle-end-seconds")} }
                select {
                    id: "idle-end-seconds",
                    class: "h-9 max-w-full rounded-md border border-border bg-background px-2",
                    disabled: disabled || state.read().phase != Phase::Idle,
                    value: state.read().settings.host.idle_end_seconds.to_string(),
                    onchange: move |event| {
                        if let Ok(seconds) = event.value().parse::<u16>() && (2..=600).contains(&seconds)
                        {
                            state.write().settings.host.idle_end_seconds = seconds;
                        }
                    },
                    for seconds in 2..=600 {
                        option { value: seconds.to_string(), "{seconds}" }
                    }
                }
                p { class: "text-xs leading-6 text-muted-foreground", {tr("idle-end-help")} }
            }
            Separator { class: "my-6".to_owned() }
            div { class: "space-y-3",
                Label { html_for: "main-table-refresh", {tr("main-table-refresh")} }
                select {
                    id: "main-table-refresh",
                    class: "h-9 max-w-full rounded-md border border-border bg-background px-2",
                    disabled,
                    value: state.read().settings.ui.main_table_refresh_seconds.to_string(),
                    onchange: move |event| {
                        if let Ok(seconds) = event.value().parse::<u8>() && seconds <= 60 {
                            state.write().settings.ui.main_table_refresh_seconds = seconds;
                        }
                    },
                    for seconds in 0..=60 {
                        option { value: seconds.to_string(), "{seconds}" }
                    }
                }
                p { class: "text-xs leading-6 text-muted-foreground",
                    {tr("main-table-refresh-help")}
                }
            }
            Separator { class: "my-6".to_owned() }
            div { class: "space-y-3",
                h2 { class: "text-sm font-semibold", {tr("main-table-reverse")} }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled,
                    aria_pressed: state.read().settings.ui.main_table_reverse,
                    onclick: move |_| {
                        let current = state.peek().settings.ui.main_table_reverse;
                        state.write().settings.ui.main_table_reverse = !current;
                    },
                    {
                        tr(
                            if state.read().settings.ui.main_table_reverse {
                                "sort-descending"
                            } else {
                                "sort-ascending"
                            },
                        )
                    }
                }
                p { class: "text-xs leading-6 text-muted-foreground",
                    {tr("main-table-reverse-help")}
                }
            }
            Separator { class: "my-6".to_owned() }
            div { class: "space-y-3",
                h2 { class: "text-sm font-semibold", {tr("main-table-commas")} }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: disabled || state.read().phase != Phase::Idle,
                    aria_pressed: state.read().settings.host.main_table_commas,
                    onclick: move |_| {
                        let current = state.peek().settings.host.main_table_commas;
                        state.write().settings.host.main_table_commas = !current;
                    },
                    {
                        tr(
                            if state.read().settings.host.main_table_commas {
                                "enabled"
                            } else {
                                "disabled"
                            },
                        )
                    }
                }
                p { class: "text-xs leading-6 text-muted-foreground",
                    {tr("main-table-commas-help")}
                }
            }
            Separator { class: "my-6".to_owned() }
            div { class: "space-y-3",
                h2 { class: "text-sm font-semibold", {tr("silence-cut-heading")} }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: disabled || state.read().phase != Phase::Idle,
                    aria_pressed: state.read().settings.host.silence_cut_enabled,
                    onclick: move |_| {
                        let current = state.peek().settings.host.silence_cut_enabled;
                        state.write().settings.host.silence_cut_enabled = !current;
                    },
                    {
                        tr(
                            if state.read().settings.host.silence_cut_enabled {
                                "enabled"
                            } else {
                                "disabled"
                            },
                        )
                    }
                }
                Label { html_for: "silence-cut-seconds", {tr("silence-cut-seconds")} }
                select {
                    id: "silence-cut-seconds",
                    class: "h-9 max-w-full rounded-md border border-border bg-background px-2",
                    disabled: disabled || state.read().phase != Phase::Idle,
                    value: state.read().settings.host.silence_cut_seconds.to_string(),
                    onchange: move |event| {
                        if let Ok(seconds) = event.value().parse::<u8>() && (1..=60).contains(&seconds) {
                            state.write().settings.host.silence_cut_seconds = seconds;
                        }
                    },
                    for seconds in 1..=60 {
                        option { value: seconds.to_string(), "{seconds}" }
                    }
                }
                p { class: "text-xs leading-6 text-muted-foreground", {tr("silence-cut-help")} }
            }
            Separator { class: "my-6".to_owned() }
            super::speech_rules::Panel {}
            super::corrections::CorrectionPanel {}
            Separator { class: "my-6".to_owned() }
            div { class: "flex gap-2",
                Button {
                    id: "save-settings",
                    disabled: !state.read().can_save_settings(),
                    onclick: move |_| save_settings(state),
                    icons::Save {}
                    {tr("save-settings")}
                }
                // Capture the locale provider here; the app-owned task can outlive this settings pane.
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: state.read().saving || state.read().phase != Phase::Idle,
                    onclick: move |_| {
                        spawn_forever(load_settings(state, language));
                    },
                    {tr("reload-settings")}
                }
            }
            p { class: "mt-4 break-all text-xs text-muted-foreground",
                "{state.read().root}/config.toml"
            }
        }
    }
}

fn timer_cue<'a>(
    audio: &'a crate::config::AudioPreferences,
    slot: &str,
) -> &'a crate::config::TimerCuePreferences {
    match slot {
        "start" => &audio.timer_start,
        "warning" => &audio.timer_warning,
        _ => &audio.timer_expire,
    }
}
fn timer_cue_mut<'a>(
    audio: &'a mut crate::config::AudioPreferences,
    slot: &str,
) -> &'a mut crate::config::TimerCuePreferences {
    match slot {
        "start" => &mut audio.timer_start,
        "warning" => &mut audio.timer_warning,
        _ => &mut audio.timer_expire,
    }
}

#[component]
fn TimerCueControl(slot: &'static str, label: &'static str) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let id = format!("timer-cue-{slot}");
    let path_id = format!("timer-cue-{slot}-path");
    let cue = timer_cue(&state.read().settings.audio, slot).clone();
    let disabled =
        state.read().saving || state.read().file.is_none() || state.read().phase != Phase::Idle;
    rsx! {
        div { class: "flex flex-wrap items-center gap-2",
            Label { html_for: id.clone(), {tr(label)} }
            select {
                id,
                class: "h-9 rounded-md border border-border bg-background px-2",
                value: cue.mode.clone(),
                disabled,
                onchange: move |event| {
                    if state.peek().phase == Phase::Idle && !state.peek().saving
                        && state.peek().file.is_some()
                    {
                        let mode = event.value();
                        if matches!(mode.as_str(), "none" | "beep" | "wav" | "tts") {
                            timer_cue_mut(&mut state.write().settings.audio, slot).mode = mode;
                        }
                    }
                },
                option { value: "none", {tr("audio-misc-none")} }
                option { value: "beep", {tr("audio-misc-beep")} }
                option { value: "wav", {tr("audio-misc-wav")} }
                option { value: "tts", {tr("audio-misc-tts")} }
            }
            if cue.mode == "wav" {
                Label { html_for: path_id.clone(), {tr("audio-timer-path")} }
                Input {
                    id: path_id,
                    value: cue.path,
                    disabled,
                    oninput: move |event: FormEvent| {
                        if state.peek().phase == Phase::Idle && !state.peek().saving
                            && state.peek().file.is_some()
                        {
                            timer_cue_mut(&mut state.write().settings.audio, slot).path = event.value();
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn MiscSoundPreferences() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let disabled = state.read().saving || state.read().file.is_none();
    rsx! {
        Label { html_for: "misc-mode", {tr("audio-misc-mode")} }
        select {
            id: "misc-mode",
            class: "h-9 rounded-md border border-border bg-background px-2",
            value: state.read().settings.audio.misc_mode.clone(),
            disabled: disabled || state.read().phase != Phase::Idle,
            onchange: move |event| {
                if state.peek().phase == Phase::Idle && !state.peek().saving
                    && state.peek().file.is_some()
                {
                    let value = event.value();
                    if matches!(value.as_str(), "none" | "beep" | "wav" | "tts") {
                        state.write().settings.audio.misc_mode = value;
                    }
                }
            },
            option { value: "none", {tr("audio-misc-none")} }
            option { value: "beep", {tr("audio-misc-beep")} }
            option { value: "wav", {tr("audio-misc-wav")} }
            option { value: "tts", {tr("audio-misc-tts")} }
        }
        Label { html_for: "misc-text", {tr("audio-misc-text")} }
        Input {
            id: "misc-text".to_owned(),
            value: state.read().settings.audio.misc_text.clone(),
            disabled: disabled || state.read().phase != Phase::Idle,
            oninput: move |event: FormEvent| {
                if state.peek().phase == Phase::Idle && !state.peek().saving
                    && state.peek().file.is_some()
                {
                    state.write().settings.audio.misc_text = event.value();
                }
            },
        }
    }
}

#[cfg(test)]
mod selective_tests {
    use super::*;
    #[test]
    fn timer_cue_draft_save_uses_snapshot_and_preserves_failure() {
        let temporary = tempfile::tempdir().expect("settings fixture");
        let root = camino::Utf8Path::from_path(temporary.path()).expect("UTF8");
        let file = crate::config::ConfigFile::load(root).expect("settings");
        let mut state = AppState::new(root.to_owned());
        state.settings = file.settings.clone();
        state.file = Some(file);
        timer_cue_mut(&mut state.settings.audio, "warning").mode = "tts".into();
        timer_cue_mut(&mut state.settings.audio, "start").mode = "beep".into();
        let (_, snapshot) = state.begin_settings_save().expect("save snapshot");
        assert_eq!(snapshot.settings.audio.settings().timer_warning, "tts");
        timer_cue_mut(&mut state.settings.audio, "warning").mode = "none".into();
        assert_eq!(snapshot.settings.audio.settings().timer_warning, "tts");
        state.revision.session += 1;
        state.receive_settings_save(Err(miette::miette!("synthetic failure")), "saved".into());
        assert_eq!(timer_cue(&state.settings.audio, "warning").mode, "none");
        assert_eq!(timer_cue(&state.settings.audio, "start").mode, "beep");
        assert!(state.error.is_some());
        assert!(!root.join("config.toml").exists());
    }

    #[test]
    fn selective_apply_preserves_draft_and_rejects_old_sessions() {
        let mut state = AppState::new(camino::Utf8PathBuf::from("/synthetic"));
        state.settings.host.selective_names = "Source".into();
        state.revision.session = 4;
        state.phase = Phase::UpdatingCorrections;
        assert!(!state.receive_selective_apply(3, Ok(()), "old".into()));
        assert!(state.phase == Phase::UpdatingCorrections);
        assert!(state.receive_selective_apply(4, Err(aftercast::Error::Closed), "saved".into()));
        assert!(state.phase == Phase::Ready);
        assert_eq!(state.settings.host.selective_names, "Source");
        assert!(state.error.is_some());
    }
}
