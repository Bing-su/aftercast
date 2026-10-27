use super::{AppState, Phase, plugin_ui_action, save_settings, start, tr};
use crate::components::{Badge, Button, ButtonVariant, Input, Label};
use dioxus::prelude::*;

#[component]
pub(super) fn PluginsPane() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let paths = state.read().settings.host.plugins.clone();
    let disabled =
        state.read().phase != Phase::Idle || state.read().saving || state.read().file.is_none();
    rsx! {
        section { class: "p-6 min-[1000px]:px-8 min-[1000px]:py-7",
            div { class: "mb-7 flex items-start justify-between gap-5 [&_h1]:text-2xl [&_h1]:font-semibold [&_h1]:tracking-tight [&_p]:mt-2 [&_p]:max-w-[68ch] [&_p]:text-[13px] [&_p]:leading-7 [&_p]:text-muted-foreground",
                div {
                    h1 { {tr("plugins")} }
                    p { {tr("plugins-description")} }
                }
                Button { disabled, onclick: move |_| add_plugin(state),
                    icons::Plus {}
                    {tr("add-plugin")}
                }
            }
            if paths.is_empty() {
                div { class: "flex min-h-100 flex-col items-center justify-center px-5 py-10 text-center [&_h2]:text-xl [&_h2]:font-medium [&_p]:my-4 [&_p]:max-w-[48ch] [&_p]:leading-8 [&_p]:text-muted-foreground",
                    icons::Puzzle { class: "mb-5 size-11 text-muted-foreground" }
                    h2 { {tr("plugins-empty-title")} }
                    p { {tr("plugins-empty-description")} }
                }
            }
            div { class: "divide-y divide-border",
                for (index, path) in paths.iter().enumerate() {
                    div { class: "flex items-center gap-3.5 py-5",
                        icons::Puzzle { class: "size-5 shrink-0 text-muted-foreground" }
                        div { class: "min-w-0 flex-1",
                            strong { class: "block truncate text-sm", "{path}" }
                            if state.read().settings.host.disabled_plugins.contains(path) {
                                Badge { class: "mt-2".to_owned(), {tr("disabled")} }
                            }
                            if state.read().failed_plugin.as_ref() == Some(path) {
                                Badge { class: "mt-2".to_owned(), {tr("plugin-startup-failed")} }
                            }
                            if let Some(plugin) = state
                                .read()
                                .plugins
                                .iter()
                                .find(|plugin| &plugin.path == path)
                                .cloned()
                            {
                                Badge { class: "mt-2 font-normal".to_owned(),
                                    {tr("plugin-status")}
                                    " {plugin.status}"
                                }
                                div { class: "mt-3 flex flex-wrap items-center gap-2",
                                    Button {
                                        variant: ButtonVariant::Outline,
                                        disabled: state.read().phase != Phase::Ready || state.read().plugin_ui_pending
                                            || plugin.info.settings != aftercast::PluginSettings::Available,
                                        onclick: move |_| plugin_ui_action(state, plugin.id, true),
                                        {tr("open-plugin-settings")}
                                    }
                                    Button {
                                        variant: ButtonVariant::Ghost,
                                        disabled: state.read().phase != Phase::Ready || state.read().plugin_ui_pending,
                                        onclick: move |_| plugin_ui_action(state, plugin.id, false),
                                        {tr("refresh-plugin-status")}
                                    }
                                    span { class: "text-xs text-muted-foreground",
                                        {
                                            tr(
                                                match plugin.info.settings {
                                                    aftercast::PluginSettings::UnsupportedPlatform => {
                                                        "plugin-settings-unsupported"
                                                    }
                                                    aftercast::PluginSettings::NoPage => "plugin-settings-unavailable",
                                                    aftercast::PluginSettings::Available => "plugin-settings-available",
                                                },
                                            )
                                        }
                                    }
                                }
                            }
                            {
                                let path = path.clone();
                                rsx! {
                                    Label {
                                        class: "mt-3 text-xs text-muted-foreground".to_owned(),
                                        html_for: format!("plugin-type-{index}"),
                                        {tr("plugin-type")}
                                    }
                                    Input {
                                        id: format!("plugin-type-{index}"),
                                        value: state.read().settings.host.plugin_types.get(&path).cloned().unwrap_or_default(),
                                        disabled,
                                        placeholder: tr("plugin-type-auto"),
                                        class: "mt-1".to_owned(),
                                        oninput: move |event: FormEvent| {
                                            state.write().settings.host.plugin_types.insert(path.clone(), event.value());
                                        },
                                    }
                                }
                            }
                        }
                        {
                            let path = path.clone();
                            let enabled = !state.read().settings.host.disabled_plugins.contains(&path);
                            rsx! {
                                Button {
                                    variant: ButtonVariant::Outline,
                                    disabled,
                                    aria_pressed: enabled,
                                    onclick: move |_| toggle_plugin(state, &path),
                                    {tr(if enabled { "disable-plugin" } else { "enable-plugin" })}
                                }
                            }
                        }
                        Button {
                            variant: ButtonVariant::Ghost,
                            disabled: disabled || index == 0,
                            onclick: move |_| state.write().settings.host.plugins.swap(index, index - 1),
                            {tr("move-up")}
                        }
                        Button {
                            variant: ButtonVariant::Ghost,
                            disabled,
                            onclick: move |_| {
                                let mut current = state.write();
                                if let Some(path) = current.settings.host.plugins.get(index).cloned() {
                                    current.settings.host.plugins.remove(index);
                                    current.settings.host.disabled_plugins.retain(|entry| entry != &path);
                                }
                            },
                            {tr("remove")}
                        }
                    }
                }
            }
            p { class: "my-6 max-w-[70ch] text-xs leading-7 text-muted-foreground",
                {tr("plugin-restart-policy")}
            }
            if state.read().startup_failed {
                Button { disabled, onclick: move |_| start(state), {tr("retry-plugin-startup")} }
            }
            Button {
                variant: ButtonVariant::Outline,
                disabled: state.read().saving || state.read().file.is_none(),
                onclick: move |_| save_settings(state),
                icons::Save {}
                {tr("save-settings")}
            }
        }
    }
}

fn add_plugin(mut state: Signal<AppState>) {
    spawn(async move {
        let mut dialog = rfd::AsyncFileDialog::new().add_filter(".NET plugin", &["dll"]);
        if let Some(directory) = directories::UserDirs::new()
            .and_then(|dirs| dirs.document_dir().map(std::path::Path::to_owned))
        {
            dialog = dialog.set_directory(directory);
        }
        let Some(file) = dialog.pick_file().await else {
            return;
        };
        if state.read().phase != Phase::Idle || state.read().saving {
            return;
        }
        let path = match camino::Utf8PathBuf::from_path_buf(file.path().to_owned()) {
            Ok(path) => path,
            Err(path) => {
                state
                    .write()
                    .fail(format!("Non-UTF-8 path: {}", path.display()));
                return;
            }
        };
        if state
            .read()
            .settings
            .host
            .plugins
            .iter()
            .any(|existing| existing == path.as_str())
        {
            return;
        }
        state.write().settings.host.plugins.push(path.to_string());
    });
}

fn toggle_plugin(mut state: Signal<AppState>, path: &str) {
    if state.read().phase != Phase::Idle || state.read().saving {
        return;
    }
    let mut current = state.write();
    if current
        .settings
        .host
        .disabled_plugins
        .iter()
        .any(|entry| entry == path)
    {
        current
            .settings
            .host
            .disabled_plugins
            .retain(|entry| entry != path);
    } else {
        current.settings.host.disabled_plugins.push(path.to_owned());
    }
}
