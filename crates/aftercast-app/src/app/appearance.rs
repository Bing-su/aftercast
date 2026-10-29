use super::{AppState, tr};
use crate::components::{Input, Label, Separator};
use dioxus::prelude::*;

#[derive(Default)]
pub(super) struct RuntimeState {
    pub(super) generation: u64,
    session: u64,
    value: Option<aftercast::RuntimeColors>,
}
impl AppState {
    pub(super) fn runtime_color_snapshot(&self) -> Option<aftercast::RuntimeColors> {
        (self.runtime_colors.session == self.revision.session)
            .then_some(self.runtime_colors.value)
            .flatten()
    }
    fn accept_runtime_colors(
        &mut self,
        session: u64,
        generation: u64,
        value: aftercast::RuntimeColors,
    ) -> bool {
        if self.revision.session != session
            || self.runtime_colors.generation != generation
            || self.saving
            || self.exit_requested
            || self.phase != super::Phase::Ready
        {
            return false;
        }
        self.runtime_colors.session = session;
        self.runtime_colors.value = Some(value);
        true
    }
    fn runtime_color_conflict(&self) -> bool {
        let Some(value) = self.runtime_color_snapshot() else {
            return false;
        };
        let appearance = &self.settings.appearance;
        let controls = appearance.color_controls();
        value.invert != appearance.invert_plugin_colors
            || value.luminosity.to_bits()
                != (f32::from(appearance.plugin_luminosity) / 100.0).to_bits()
            || value.saturation.to_bits()
                != (f32::from(appearance.plugin_saturation) / 100.0).to_bits()
            || value.ally != controls.ally.map(|rgb| rgb | 0xff00_0000)
            || value.personal != controls.personal.map(|rgb| rgb | 0xff00_0000)
    }
}
fn runtime_request(current: &AppState) -> Option<(aftercast::HostHandle, u64, u64)> {
    if current.phase != super::Phase::Ready || current.saving || current.exit_requested {
        return None;
    }
    Some((
        current.handle()?,
        current.revision.session,
        current.runtime_colors.generation,
    ))
}
pub(super) async fn poll_runtime(mut state: Signal<AppState>) {
    loop {
        dioxus_sdk::time::sleep(std::time::Duration::from_secs(1)).await;
        let request = runtime_request(&state.read());
        if let Some((handle, session, generation)) = request
            && let Ok(value) = handle.runtime_colors().await
        {
            state
                .write()
                .accept_runtime_colors(session, generation, value);
        }
    }
}

#[component]
pub(super) fn Controls() -> Element {
    let state = use_context::<Signal<AppState>>();
    let conflict = state.read().runtime_color_conflict();
    rsx! {
        Separator { class: "my-6".to_owned() }
        section { class: "space-y-4", "aria-label": tr("appearance-custom-title"),
            h2 { class: "text-sm font-semibold", {tr("appearance-custom-title")} }
            p { class: "text-xs text-muted-foreground", {tr("appearance-custom-help")} }
            if conflict {
                p { role: "status", class: "text-xs text-muted-foreground",
                    {tr("appearance-runtime-conflict")}
                }
            }
            ColorAdjustmentControls {}
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

#[component]
fn ColorAdjustmentControls() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let disabled = current.saving || current.file.is_none();
    let invert = current.settings.appearance.invert_plugin_colors;
    let lum = current.settings.appearance.plugin_luminosity;
    let sat = current.settings.appearance.plugin_saturation;
    drop(current);
    rsx! {
        div { class: "space-y-2 rounded-md border border-border p-3",
            h3 { class: "text-sm font-medium", {tr("appearance-plugin-adjust")} }
            Label { html_for: "appearance-plugin-invert", {tr("appearance-plugin-invert")} }
            select {
                id: "appearance-plugin-invert",
                value: if invert { "true" } else { "false" },
                disabled,
                class: "h-9 w-full rounded-md border border-border bg-background px-2",
                onchange: move |e| {
                    state.write().settings.appearance.invert_plugin_colors = e.value() == "true";
                },
                option { value: "false", {tr("appearance-adjust-off")} }
                option { value: "true", {tr("appearance-adjust-on")} }
            }
            for (kind, value, label) in [("lum", lum, "appearance-plugin-lum"), ("sat", sat, "appearance-plugin-sat")] {
                div { class: "space-y-1",
                    Label { html_for: format!("appearance-plugin-{kind}"), {tr(label)} }
                    select {
                        id: format!("appearance-plugin-{kind}"),
                        value: value.to_string(),
                        disabled,
                        class: "h-9 w-full rounded-md border border-border bg-background px-2",
                        onchange: move |event| {
                            if let Ok(value) = event.value().parse::<i8>() && (-100..=100).contains(&value) {
                                if kind == "lum" {
                                    state.write().settings.appearance.plugin_luminosity = value;
                                } else {
                                    state.write().settings.appearance.plugin_saturation = value;
                                }
                            }
                        },
                        for value in -100..=100_i8 {
                            option { value: value.to_string(), "{value}%" }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod runtime_tests {
    use super::*;
    #[test]
    fn runtime_snapshot_preserves_draft_and_rejects_old_responses() {
        let mut state = AppState::new("/tmp/synthetic".into());
        state.phase = super::super::Phase::Ready;
        state.revision.session = 5;
        state.settings.appearance.ally_foreground = "#123456".into();
        let value = aftercast::RuntimeColors {
            invert: true,
            luminosity: 0.345,
            saturation: 0.0,
            ally: Some(0x4008090a),
            personal: None,
        };
        assert!(state.accept_runtime_colors(5, 0, value));
        assert_eq!(state.runtime_color_snapshot(), Some(value));
        assert!(state.runtime_color_conflict());
        assert_eq!(state.settings.appearance.ally_foreground, "#123456");
        state.runtime_colors.generation = 1;
        assert!(!state.accept_runtime_colors(5, 0, value));
        state.saving = true;
        assert!(!state.accept_runtime_colors(5, 1, value));
        state.saving = false;
        state.revision.session = 6;
        assert_eq!(state.runtime_color_snapshot(), None);
        assert!(!state.accept_runtime_colors(5, 1, value));
        assert_eq!(state.settings.appearance.ally_foreground, "#123456");
    }
}
