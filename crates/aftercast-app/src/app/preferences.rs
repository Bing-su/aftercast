use super::{AppState, Phase, load_settings, save_settings, tr};
use crate::components::{Button, ButtonVariant, Input, Label, Separator};
use dioxus::core::spawn_forever;
use dioxus::prelude::*;
use dioxus_i18n::prelude::*;

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
