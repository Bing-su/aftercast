use super::{AppState, tr};
use crate::components::{Input, Label, Separator};
use dioxus::prelude::*;

#[component]
pub(super) fn Controls() -> Element {
    rsx! {
        Separator { class: "my-6".to_owned() }
        section { class: "space-y-4", "aria-label": tr("appearance-custom-title"),
            h2 { class: "text-sm font-semibold", {tr("appearance-custom-title")} }
            p { class: "text-xs text-muted-foreground", {tr("appearance-custom-help")} }
            SurfaceControls { mini: false }
            SurfaceControls { mini: true }
        }
    }
}
#[component]
fn SurfaceControls(mini: bool) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let appearance = &current.settings.appearance;
    let (font, size, foreground, background) = if mini {
        (
            appearance.mini_font.clone(),
            appearance.mini_size,
            appearance.mini_foreground.clone(),
            appearance.mini_background.clone(),
        )
    } else {
        (
            appearance.grid_font.clone(),
            appearance.grid_size,
            appearance.grid_foreground.clone(),
            appearance.grid_background.clone(),
        )
    };
    let ally_foreground = appearance.ally_foreground.clone();
    let self_background = appearance.self_background.clone();
    let disabled = current.saving || current.file.is_none();
    let key = if mini { "mini" } else { "grid" };
    let font_id = format!("appearance-{key}-font");
    let size_id = format!("appearance-{key}-size");
    drop(current);
    rsx! {
        div { class: "space-y-2 rounded-md border border-border p-3",
            h3 { class: "text-sm font-medium",
                {tr(if mini { "appearance-mini" } else { "appearance-grid" })}
            }
            Label { html_for: font_id.clone(), {tr("appearance-font")} }
            select {
                id: font_id,
                value: font,
                disabled,
                class: "h-9 w-full rounded-md border border-border bg-background px-2",
                onchange: move |event| {
                    let value = event.value();
                    if matches!(value.as_str(), "" | "sans-serif" | "serif" | "monospace") {
                        let mut s = state.write();
                        if mini {
                            s.settings.appearance.mini_font = value;
                        } else {
                            s.settings.appearance.grid_font = value;
                        }
                    }
                },
                option { value: "", {tr("appearance-default")} }
                option { value: "sans-serif", "Sans-serif" }
                option { value: "serif", "Serif" }
                option { value: "monospace", "Monospace" }
            }
            Label { html_for: size_id.clone(), {tr("appearance-size")} }
            select {
                id: size_id,
                value: size.to_string(),
                disabled,
                class: "h-9 w-full rounded-md border border-border bg-background px-2",
                onchange: move |event| {
                    if let Ok(value) = event.value().parse::<u8>()
                        && (value == 0 || (8..=32).contains(&value))
                    {
                        let mut s = state.write();
                        if mini {
                            s.settings.appearance.mini_size = value;
                        } else {
                            s.settings.appearance.grid_size = value;
                        }
                    }
                },
                option { value: "0", {tr("appearance-default")} }
                for value in 8..=32_u8 {
                    option { value: value.to_string(), "{value}px" }
                }
            }
            for (kind, label, text) in [
                ("foreground", "appearance-foreground", foreground),
                ("background", "appearance-background", background),
            ]
            {
                {
                    let id = format!("appearance-{key}-{kind}");
                    rsx! {
                        div { class: "space-y-1",
                            Label { html_for: id.clone(), {tr(label)} }
                            Input {
                                id,
                                value: text,
                                disabled,
                                placeholder: "#RRGGBB".to_owned(),
                                oninput: move |event: FormEvent| {
                                    let value = event.value();
                                    let mut s = state.write();
                                    match (mini, kind) {
                                        (true, "foreground") => s.settings.appearance.mini_foreground = value,
                                        (true, _) => s.settings.appearance.mini_background = value,
                                        (false, "foreground") => s.settings.appearance.grid_foreground = value,
                                        (false, _) => s.settings.appearance.grid_background = value,
                                    }
                                },
                            }
                        }
                    }
                }
            }
            if !mini {
                Label { html_for: "appearance-ally", {tr("appearance-ally")} }
                Input {
                    id: "appearance-ally".to_owned(),
                    value: ally_foreground,
                    disabled,
                    placeholder: "#RRGGBB".to_owned(),
                    oninput: move |event: FormEvent| {
                        state.write().settings.appearance.ally_foreground = event.value();
                    },
                }
                Label { html_for: "appearance-self", {tr("appearance-self")} }
                Input {
                    id: "appearance-self".to_owned(),
                    value: self_background,
                    disabled,
                    placeholder: "#RRGGBB".to_owned(),
                    oninput: move |event: FormEvent| {
                        state.write().settings.appearance.self_background = event.value();
                    },
                }
            }
        }
    }
}
