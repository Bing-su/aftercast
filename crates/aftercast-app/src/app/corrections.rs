use super::{AppState, Phase, tr};
use crate::components::{Button, ButtonVariant, Input, Label};
use aftercast::{CorrectionRevision, CorrectionSnapshot, RedirectDefinition, RenameDefinition};
use dioxus::{core::spawn_forever, prelude::*};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Token {
    session: u64,
    generation: u64,
}
type AdmittedCorrectionRequest = (
    Token,
    Option<CorrectionRevision>,
    Vec<RenameDefinition>,
    Vec<RedirectDefinition>,
);
#[derive(Default)]
pub(super) struct EditorState {
    snapshot: Option<CorrectionSnapshot>,
    renames: Vec<RenameDefinition>,
    redirects: Vec<RedirectDefinition>,
    dirty: bool,
    pending: Option<Token>,
    generation: u64,
    error: Option<String>,
    confirm_discard: bool,
}
impl AppState {
    fn correction_token(&self) -> Token {
        Token {
            session: self.revision.session,
            generation: self.corrections.generation,
        }
    }
    fn correction_edit(
        &mut self,
        token: Token,
        structural: bool,
        edit: impl FnOnce(&mut EditorState),
    ) -> bool {
        if self.screen != "settings"
            || self.exit_requested
            || self.phase != Phase::Ready
            || self.corrections.pending.is_some()
            || self.correction_token() != token
            || !self
                .corrections
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.editable)
        {
            return false;
        }
        edit(&mut self.corrections);
        self.corrections.dirty = true;
        self.corrections.error = None;
        if structural {
            self.corrections.generation += 1;
        }
        true
    }
    fn request_correction_discard(&mut self, token: Token) -> bool {
        if self.screen != "settings"
            || self.phase != Phase::Ready
            || self.correction_token() != token
            || !self.corrections.dirty
        {
            return false;
        }
        self.corrections.confirm_discard = true;
        true
    }
    fn confirm_correction_discard(&mut self, token: Token, confirm: bool) -> bool {
        if self.screen != "settings"
            || self.phase != Phase::Ready
            || self.correction_token() != token
            || !self.corrections.confirm_discard
        {
            return false;
        }
        if confirm {
            let Some(snapshot) = self.corrections.snapshot.clone() else {
                return false;
            };
            self.corrections.renames = snapshot.renames;
            self.corrections.redirects = snapshot.redirects;
            self.corrections.dirty = false;
            self.corrections.generation += 1;
        }
        self.corrections.confirm_discard = false;
        true
    }
    fn begin_corrections(&mut self, save: bool) -> Option<AdmittedCorrectionRequest> {
        if self.screen != "settings"
            || self.exit_requested
            || self.phase != Phase::Ready
            || self.corrections.pending.is_some()
        {
            return None;
        }
        if save
            && (!self.corrections.dirty
                || !self
                    .corrections
                    .snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.editable))
        {
            return None;
        }
        if !save && self.corrections.dirty {
            return None;
        }
        self.corrections.generation += 1;
        let token = self.correction_token();
        self.corrections.pending = Some(token);
        self.corrections.error = None;
        self.phase = Phase::UpdatingCorrections;
        Some((
            token,
            self.corrections
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.revision),
            self.corrections.renames.clone(),
            self.corrections.redirects.clone(),
        ))
    }
    fn receive_corrections(
        &mut self,
        token: Token,
        result: Result<CorrectionSnapshot, aftercast::Error>,
    ) -> bool {
        if self.corrections.pending != Some(token)
            || self.correction_token() != token
            || self.phase != Phase::UpdatingCorrections
        {
            return false;
        }
        self.corrections.pending = None;
        self.phase = Phase::Ready;
        match result {
            Ok(snapshot) => {
                self.corrections.renames = snapshot.renames.clone();
                self.corrections.redirects = snapshot.redirects.clone();
                self.corrections.snapshot = Some(snapshot);
                self.corrections.dirty = false;
                self.corrections.confirm_discard = false;
                self.corrections.generation += 1;
            }
            Err(error) => self.corrections.error = Some(error.to_string()),
        }
        true
    }
}
fn request(mut state: Signal<AppState>, save: bool) {
    let Some(handle) = state.read().handle() else {
        return;
    };
    let Some((token, revision, renames, redirects)) = state.write().begin_corrections(save) else {
        return;
    };
    spawn_forever(async move {
        let result = if save {
            handle
                .save_corrections(
                    revision.expect("save requires a loaded snapshot"),
                    &renames,
                    &redirects,
                )
                .await
        } else {
            handle.corrections().await
        };
        state.write().receive_corrections(token, result);
    });
}
#[component]
pub(super) fn CorrectionPanel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let token = current.correction_token();
    let ready = current.phase == Phase::Ready && !current.exit_requested;
    let busy = current.corrections.pending.is_some();
    let snapshot = current.corrections.snapshot.clone();
    let editable = ready && snapshot.as_ref().is_some_and(|value| value.editable);
    let dirty = current.corrections.dirty;
    let confirm_discard = current.corrections.confirm_discard;
    let error = current.corrections.error.clone();
    let renames = current.corrections.renames.clone();
    let redirects = current.corrections.redirects.clone();
    drop(current);
    rsx! {
        section { class: "space-y-4", "aria-label": tr("correction-title"),
            h2 { class: "text-sm font-semibold", {tr("correction-title")} }
            p { class: "text-xs leading-6 text-muted-foreground", {tr("correction-help")} }
            if !ready && !busy {
                p { class: "text-sm", {tr("correction-start")} }
            }
            if busy {
                p { role: "status", {tr("correction-working")} }
            }
            if let Some(error) = error {
                p {
                    role: "alert",
                    class: "break-words text-sm text-destructive",
                    "{error}"
                }
            }
            div { class: "flex flex-wrap gap-2",
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: !ready || dirty,
                    onclick: move |_| request(state, false),
                    {tr("correction-load")}
                }
                Button {
                    disabled: !editable || !dirty,
                    onclick: move |_| request(state, true),
                    {tr("correction-save")}
                }
                if dirty {
                    Button {
                        variant: ButtonVariant::Ghost,
                        disabled: !ready,
                        onclick: move |_| {
                            state.write().request_correction_discard(token);
                        },
                        {tr("correction-discard")}
                    }
                }
            }
            if confirm_discard {
                p { class: "text-sm", {tr("correction-discard-confirm")} }
                div { class: "flex gap-2",
                    Button {
                        disabled: !ready,
                        onclick: move |_| {
                            state.write().confirm_correction_discard(token, true);
                        },
                        {tr("correction-discard-confirm-button")}
                    }
                    Button {
                        variant: ButtonVariant::Ghost,
                        disabled: !ready,
                        onclick: move |_| {
                            state.write().confirm_correction_discard(token, false);
                        },
                        {tr("cancel")}
                    }
                }
            }
            if snapshot.as_ref().is_some_and(|value| !value.editable) {
                p { role: "alert", class: "text-sm", {tr("correction-unsupported")} }
            }
            if snapshot.is_some() {
                h3 { class: "text-sm font-semibold", {tr("correction-renames")} }
                for (index, rule) in renames.iter().enumerate() {
                    div {
                        key: "rename-{index}",
                        class: "space-y-2 rounded-md border border-border p-3",
                        Label { html_for: format!("rename-before-{index}"), {tr("correction-before")} }
                        Input {
                            id: format!("rename-before-{index}"),
                            value: rule.before.clone(),
                            disabled: !editable,
                            oninput: move |event: FormEvent| {
                                state
                                    .write()
                                    .correction_edit(
                                        token,
                                        false,
                                        |editor| {
                                            if let Some(rule) = editor.renames.get_mut(index) {
                                                rule.before = event.value();
                                            }
                                        },
                                    );
                            },
                        }
                        Label { html_for: format!("rename-after-{index}"), {tr("correction-after")} }
                        Input {
                            id: format!("rename-after-{index}"),
                            value: rule.after.clone(),
                            disabled: !editable,
                            oninput: move |event: FormEvent| {
                                state
                                    .write()
                                    .correction_edit(
                                        token,
                                        false,
                                        |editor| {
                                            if let Some(rule) = editor.renames.get_mut(index) {
                                                rule.after = event.value();
                                            }
                                        },
                                    );
                            },
                        }
                        div { class: "flex gap-2",
                            Button {
                                variant: ButtonVariant::Outline,
                                disabled: !editable,
                                aria_pressed: rule.active,
                                onclick: move |_| {
                                    state
                                        .write()
                                        .correction_edit(
                                            token,
                                            false,
                                            |editor| {
                                                if let Some(rule) = editor.renames.get_mut(index) {
                                                    rule.active = !rule.active;
                                                }
                                            },
                                        );
                                },
                                {tr(if rule.active { "enabled" } else { "disabled" })}
                            }
                            Button {
                                variant: ButtonVariant::Ghost,
                                disabled: !editable,
                                onclick: move |_| {
                                    state
                                        .write()
                                        .correction_edit(
                                            token,
                                            true,
                                            |editor| {
                                                if index < editor.renames.len() {
                                                    editor.renames.remove(index);
                                                }
                                            },
                                        );
                                },
                                {tr("remove")}
                            }
                        }
                    }
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: !editable || renames.len() >= 1024,
                    onclick: move |_| {
                        state
                            .write()
                            .correction_edit(
                                token,
                                true,
                                |editor| {
                                    editor
                                        .renames
                                        .push(RenameDefinition {
                                            active: true,
                                            ..Default::default()
                                        })
                                },
                            );
                    },
                    {tr("correction-add-rename")}
                }
                h3 { class: "text-sm font-semibold", {tr("correction-redirects")} }
                for (index, rule) in redirects.iter().enumerate() {
                    div {
                        key: "redirect-{index}",
                        class: "space-y-2 rounded-md border border-border p-3",
                        Label { html_for: format!("redirect-ability-{index}"),
                            {tr("correction-ability")}
                        }
                        Input {
                            id: format!("redirect-ability-{index}"),
                            value: rule.ability.clone(),
                            disabled: !editable,
                            oninput: move |event: FormEvent| {
                                state
                                    .write()
                                    .correction_edit(
                                        token,
                                        false,
                                        |editor| {
                                            if let Some(rule) = editor.redirects.get_mut(index) {
                                                rule.ability = event.value();
                                            }
                                        },
                                    );
                            },
                        }
                        Label { html_for: format!("redirect-destination-{index}"),
                            {tr("correction-destination")}
                        }
                        Input {
                            id: format!("redirect-destination-{index}"),
                            value: rule.destination.clone(),
                            disabled: !editable,
                            oninput: move |event: FormEvent| {
                                state
                                    .write()
                                    .correction_edit(
                                        token,
                                        false,
                                        |editor| {
                                            if let Some(rule) = editor.redirects.get_mut(index) {
                                                rule.destination = event.value();
                                            }
                                        },
                                    );
                            },
                        }
                        Label { html_for: format!("redirect-type-{index}"), {tr("correction-type")} }
                        select {
                            id: format!("redirect-type-{index}"),
                            class: "h-9 max-w-full rounded-md border border-border bg-background px-2",
                            disabled: !editable,
                            value: rule.r#type.to_string(),
                            onchange: move |event: FormEvent| {
                                if let Ok(value) = event.value().parse::<i32>() {
                                    state
                                        .write()
                                        .correction_edit(
                                            token,
                                            false,
                                            |editor| {
                                                if let Some(rule) = editor.redirects.get_mut(index) {
                                                    rule.r#type = value;
                                                }
                                            },
                                        );
                                }
                            },
                            for (value, label) in [
                                (0, "correction-type-all"),
                                (1, "correction-type-damage"),
                                (2, "correction-type-healing"),
                                (3, "correction-type-other"),
                            ]
                            {
                                option { value: "{value}", {tr(label)} }
                            }
                            if !(0..=3).contains(&rule.r#type) {
                                option { value: rule.r#type.to_string(), "{rule.r#type}" }
                            }
                        }
                        div { class: "flex gap-2",
                            Button {
                                variant: ButtonVariant::Outline,
                                disabled: !editable,
                                aria_pressed: rule.active,
                                onclick: move |_| {
                                    state
                                        .write()
                                        .correction_edit(
                                            token,
                                            false,
                                            |editor| {
                                                if let Some(rule) = editor.redirects.get_mut(index) {
                                                    rule.active = !rule.active;
                                                }
                                            },
                                        );
                                },
                                {tr(if rule.active { "enabled" } else { "disabled" })}
                            }
                            Button {
                                variant: ButtonVariant::Ghost,
                                disabled: !editable,
                                onclick: move |_| {
                                    state
                                        .write()
                                        .correction_edit(
                                            token,
                                            true,
                                            |editor| {
                                                if index < editor.redirects.len() {
                                                    editor.redirects.remove(index);
                                                }
                                            },
                                        );
                                },
                                {tr("remove")}
                            }
                        }
                    }
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: !editable || redirects.len() >= 1024,
                    onclick: move |_| {
                        state
                            .write()
                            .correction_edit(
                                token,
                                true,
                                |editor| {
                                    editor
                                        .redirects
                                        .push(RedirectDefinition {
                                            active: true,
                                            ..Default::default()
                                        })
                                },
                            );
                    },
                    {tr("correction-add-redirect")}
                }
            }
        }
    }
}
#[cfg(test)]
mod tests;
