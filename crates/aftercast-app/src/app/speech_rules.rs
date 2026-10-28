use super::{AppState, Phase, tr};
use crate::components::{Button, ButtonVariant, Input, Label, Separator};
use aftercast::{SpeechRule, SpeechRuleRevision, SpeechRuleSnapshot};
use dioxus::{core::spawn_forever, prelude::*};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Token {
    session: u64,
    generation: u64,
}
#[derive(Default)]
pub(super) struct EditorState {
    snapshot: Option<SpeechRuleSnapshot>,
    draft: Vec<SpeechRule>,
    generation: u64,
    pending: Option<Token>,
    dirty: bool,
    confirm_discard: bool,
    error: Option<String>,
}
impl AppState {
    fn speech_rule_token(&self) -> Token {
        Token {
            session: self.revision.session,
            generation: self.speech_rules.generation,
        }
    }
    fn edit_speech_rule(
        &mut self,
        token: Token,
        structural: bool,
        edit: impl FnOnce(&mut EditorState),
    ) -> bool {
        if self.screen != "settings"
            || self.exit_requested
            || self.phase != Phase::Ready
            || self.speech_rules.pending.is_some()
            || self.speech_rule_token() != token
            || self.speech_rules.snapshot.is_none()
        {
            return false;
        }
        edit(&mut self.speech_rules);
        self.speech_rules.dirty = true;
        self.speech_rules.error = None;
        if structural {
            self.speech_rules.generation += 1;
        }
        true
    }
    fn discard_speech_rules(&mut self, token: Token, confirm: bool) -> bool {
        if self.screen != "settings"
            || self.phase != Phase::Ready
            || self.speech_rule_token() != token
            || !self.speech_rules.confirm_discard
        {
            return false;
        }
        if confirm {
            let Some(snapshot) = self.speech_rules.snapshot.as_ref() else {
                return false;
            };
            self.speech_rules.draft = snapshot.rules.clone();
            self.speech_rules.dirty = false;
            self.speech_rules.generation += 1;
        }
        self.speech_rules.confirm_discard = false;
        true
    }
    fn begin_speech_rules(
        &mut self,
        save: bool,
    ) -> Option<(Token, Option<SpeechRuleRevision>, Vec<SpeechRule>)> {
        if self.screen != "settings"
            || self.exit_requested
            || self.phase != Phase::Ready
            || self.speech_rules.pending.is_some()
            || self.speech_rules.confirm_discard
        {
            return None;
        }
        if save && (!self.speech_rules.dirty || self.speech_rules.snapshot.is_none())
            || !save && self.speech_rules.dirty
        {
            return None;
        }
        self.speech_rules.generation += 1;
        let token = self.speech_rule_token();
        let revision = self
            .speech_rules
            .snapshot
            .as_ref()
            .map(|value| value.revision);
        self.speech_rules.pending = Some(token);
        self.speech_rules.error = None;
        self.phase = Phase::UpdatingCorrections;
        Some((token, revision, self.speech_rules.draft.clone()))
    }
    fn receive_speech_rules(
        &mut self,
        token: Token,
        save: bool,
        result: Result<SpeechRuleSnapshot, aftercast::Error>,
        message: String,
    ) -> bool {
        if self.speech_rule_token() != token || self.speech_rules.pending != Some(token) {
            return false;
        }
        self.speech_rules.pending = None;
        if self.phase == Phase::UpdatingCorrections {
            self.phase = Phase::Ready;
        }
        match result {
            Ok(snapshot) => {
                self.speech_rules.draft = snapshot.rules.clone();
                self.speech_rules.snapshot = Some(snapshot);
                self.speech_rules.dirty = false;
                if save {
                    self.message = message;
                }
            }
            Err(error) => self.speech_rules.error = Some(error.to_string()),
        }
        true
    }
}
fn request(mut state: Signal<AppState>, save: bool) {
    let Some(handle) = state.peek().handle() else {
        return;
    };
    let Some((token, revision, draft)) = state.write().begin_speech_rules(save) else {
        return;
    };
    let message = tr("speech-rules-saved");
    spawn_forever(async move {
        let result = if save {
            match revision {
                Some(revision) => handle.save_speech_rules(revision, &draft).await,
                None => return,
            }
        } else {
            handle.speech_rules().await
        };
        state
            .write()
            .receive_speech_rules(token, save, result, message);
    });
}
#[component]
pub(super) fn Panel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let token = current.speech_rule_token();
    let loaded = current.speech_rules.snapshot.is_some();
    let ready = current.screen == "settings"
        && current.phase == Phase::Ready
        && current.speech_rules.pending.is_none();
    let dirty = current.speech_rules.dirty;
    let confirm = current.speech_rules.confirm_discard;
    let draft = current.speech_rules.draft.clone();
    let error = current.speech_rules.error.clone();
    drop(current);
    rsx! {
        Separator { class: "my-6".to_owned() }
        section { class: "space-y-3",
            h2 { class: "text-sm font-semibold", {tr("speech-rules-title")} }
            p { class: "text-xs text-muted-foreground", {tr("speech-rules-help")} }
            div { class: "flex flex-wrap gap-2",
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: !ready || dirty || confirm,
                    onclick: move |_| request(state, false),
                    {tr("speech-rules-load")}
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: !ready || !loaded || !dirty || confirm,
                    onclick: move |_| request(state, true),
                    {tr("speech-rules-save")}
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: !ready || !loaded || !dirty || confirm,
                    onclick: move |_| {
                        let mut current = state.write();
                        if current.screen == "settings" && current.phase == Phase::Ready
                            && current.speech_rule_token() == token
                        {
                            current.speech_rules.confirm_discard = true;
                        }
                    },
                    {tr("speech-rules-discard")}
                }
            }
            if confirm {
                p { {tr("speech-rules-discard-confirm")} }
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: move |_| {
                        state.write().discard_speech_rules(token, true);
                    },
                    {tr("speech-rules-discard")}
                }
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: move |_| {
                        state.write().discard_speech_rules(token, false);
                    },
                    {tr("speech-rules-cancel")}
                }
            }
            if let Some(error) = error {
                p { role: "alert", "{error}" }
            }
            for (index, rule) in draft.iter().enumerate() {
                div {
                    key: "{index}-{token.generation}",
                    class: "space-y-2 rounded-md border border-border p-3",
                    Label { html_for: format!("speech-pattern-{index}"), {tr("speech-rules-pattern")} }
                    Input {
                        id: format!("speech-pattern-{index}"),
                        value: rule.pattern.clone(),
                        disabled: !ready || confirm,
                        oninput: move |event: FormEvent| {
                            state
                                .write()
                                .edit_speech_rule(
                                    token,
                                    false,
                                    |editor| {
                                        if let Some(item) = editor.draft.get_mut(index) {
                                            item.pattern = event.value();
                                        }
                                    },
                                );
                        },
                    }
                    Label { html_for: format!("speech-replacement-{index}"),
                        {tr("speech-rules-replacement")}
                    }
                    Input {
                        id: format!("speech-replacement-{index}"),
                        value: rule.replacement.clone(),
                        disabled: !ready || confirm,
                        oninput: move |event: FormEvent| {
                            state
                                .write()
                                .edit_speech_rule(
                                    token,
                                    false,
                                    |editor| {
                                        if let Some(item) = editor.draft.get_mut(index) {
                                            item.replacement = event.value();
                                        }
                                    },
                                );
                        },
                    }
                    div { class: "flex flex-wrap gap-2",
                        Button {
                            variant: ButtonVariant::Outline,
                            aria_pressed: rule.active,
                            disabled: !ready || confirm,
                            onclick: move |_| {
                                state
                                    .write()
                                    .edit_speech_rule(
                                        token,
                                        false,
                                        |editor| {
                                            if let Some(item) = editor.draft.get_mut(index) {
                                                item.active = !item.active;
                                            }
                                        },
                                    );
                            },
                            {tr("speech-rules-active")}
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: !ready || confirm || index == 0,
                            onclick: move |_| {
                                state
                                    .write()
                                    .edit_speech_rule(
                                        token,
                                        true,
                                        |editor| {
                                            if index > 0 && index < editor.draft.len() {
                                                editor.draft.swap(index, index - 1);
                                            }
                                        },
                                    );
                            },
                            {tr("speech-rules-up")}
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: !ready || confirm || index + 1 >= draft.len(),
                            onclick: move |_| {
                                state
                                    .write()
                                    .edit_speech_rule(
                                        token,
                                        true,
                                        |editor| {
                                            if index + 1 < editor.draft.len() {
                                                editor.draft.swap(index, index + 1);
                                            }
                                        },
                                    );
                            },
                            {tr("speech-rules-down")}
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: !ready || confirm,
                            onclick: move |_| {
                                state
                                    .write()
                                    .edit_speech_rule(
                                        token,
                                        true,
                                        |editor| {
                                            if index < editor.draft.len() {
                                                editor.draft.remove(index);
                                            }
                                        },
                                    );
                            },
                            {tr("speech-rules-remove")}
                        }
                    }
                }
            }
            Button {
                variant: ButtonVariant::Outline,
                disabled: !ready || !loaded || confirm || draft.len() >= 64,
                onclick: move |_| {
                    state
                        .write()
                        .edit_speech_rule(
                            token,
                            true,
                            |editor| {
                                editor
                                    .draft
                                    .push(SpeechRule {
                                        active: true,
                                        ..Default::default()
                                    })
                            },
                        );
                },
                {tr("speech-rules-add")}
            }
        }
    }
}
#[cfg(test)]
mod tests;
