use super::{AppState, Token, tr};
use crate::components::{Button, ButtonVariant, Input, Label};
use aftercast::ImportCharacter;
use dioxus::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Legacy,
    Current,
    Manual,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Draft {
    mode: Mode,
    name: String,
}
impl Draft {
    pub(super) fn options(&self) -> Result<Option<ImportCharacter>, &'static str> {
        match self.mode {
            Mode::Legacy => Ok(None),
            Mode::Current => Ok(Some(ImportCharacter::current())),
            Mode::Manual => ImportCharacter::new(self.name.clone())
                .map(Some)
                .map_err(|_error| "import-character-invalid"),
        }
    }
}
impl AppState {
    pub(in crate::app) fn edit_import_character(
        &mut self,
        token: Token,
        field: &str,
        value: &str,
    ) -> bool {
        if self.text_view() != token || !self.can_edit_import_text() {
            return false;
        }
        let draft = &mut self.import_text.character;
        match field {
            "mode" => {
                draft.mode = match value {
                    "legacy" => Mode::Legacy,
                    "current" => Mode::Current,
                    "manual" => Mode::Manual,
                    _ => return false,
                };
                self.import_text.invalidate_view();
            }
            "name" if draft.mode == Mode::Manual => draft.name = value.into(),
            _ => return false,
        }
        true
    }
    fn active_import_character(&self) -> Option<Option<ImportCharacter>> {
        self.import_text
            .full
            .as_ref()
            .map(|request| request.viewpoint.clone())
            .or_else(|| self.reparse.viewpoint_snapshot())
            .or_else(|| self.clipboard.viewpoint_snapshot())
    }
}
fn description(choice: Option<&ImportCharacter>) -> String {
    match choice {
        None => tr("import-character-legacy"),
        Some(choice) if choice.name().is_empty() => tr("import-character-current"),
        Some(choice) => {
            let effective = if choice.name().encode_utf16().count() <= 2 {
                "YOU"
            } else {
                choice.name()
            };
            format!("{} → {effective}", choice.name())
        }
    }
}
#[component]
pub(in crate::app) fn Controls() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let token = current.text_view();
    let draft = current.import_text.character.clone();
    let blocked = !current.can_edit_import_text();
    let active = current.active_import_character();
    let requested = draft.options();
    drop(current);
    rsx! {
        section {
            class: "my-5 space-y-3",
            "aria-label": tr("import-character-title"),
            h2 { class: "text-sm font-semibold", {tr("import-character-title")} }
            p { class: "max-w-prose text-sm leading-6 text-muted-foreground",
                {tr("import-character-help")}
            }
            div {
                class: "flex flex-wrap gap-2",
                role: "group",
                "aria-label": tr("import-character-choice"),
                for (key, mode) in [("legacy", Mode::Legacy), ("current", Mode::Current), ("manual", Mode::Manual)] {
                    Button {
                        id: format!("import-character-{key}"),
                        variant: ButtonVariant::Outline,
                        disabled: blocked,
                        aria_pressed: draft.mode == mode,
                        onclick: move |_| {
                            state.write().edit_import_character(token, "mode", key);
                        },
                        {tr(&format!("import-character-{key}"))}
                    }
                }
            }
            if draft.mode == Mode::Manual {
                div { class: "max-w-xl space-y-2",
                    Label { html_for: "import-character-name", {tr("import-character-name")} }
                    Input {
                        id: "import-character-name",
                        value: draft.name,
                        disabled: blocked,
                        oninput: move |event: FormEvent| {
                            state.write().edit_import_character(token, "name", &event.value());
                        },
                    }
                }
            }
            match requested {
                Ok(choice) => rsx! {
                    p { class: "break-words text-xs leading-6 text-muted-foreground",
                        {tr("import-character-draft")}
                        " · "
                        {description(choice.as_ref())}
                    }
                },
                Err(key) => rsx! {
                    p { role: "alert", class: "text-sm text-ctp-red", {tr(key)} }
                },
            }
            if let Some(choice) = active {
                p { role: "status", class: "break-words text-sm",
                    {tr("import-character-active")}
                    " · "
                    {description(choice.as_ref())}
                }
            }
            p { class: "max-w-prose text-xs leading-6 text-muted-foreground",
                {tr("import-character-session")}
            }
        }
    }
}
#[cfg(test)]
pub(in crate::app) mod tests;
