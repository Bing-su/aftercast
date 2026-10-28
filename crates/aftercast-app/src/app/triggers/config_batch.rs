use super::{AppState, Phase, tr};
use crate::components::{Accordion, Button, ButtonVariant, Label, TextArea};
use aftercast::TriggerBatchPreview;
use dioxus::{core::spawn_forever, prelude::*};

#[derive(Default)]
pub(super) struct State {
    text: String,
    generation: u64,
    preview: Option<TriggerBatchPreview>,
}
impl State {
    fn edit(&mut self, value: String) {
        self.text = value;
        self.cancel();
    }
    fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.preview = None;
    }
}
impl AppState {
    fn config_batch_current(&self, session: u64, generation: u64) -> bool {
        self.revision.session == session && self.triggers.config_batch.generation == generation
    }
}
impl AppState {
    fn begin_config_batch_preview(
        &mut self,
        session: u64,
        generation: u64,
    ) -> Option<(u64, u64, String)> {
        if self.screen != "triggers" || !self.config_batch_current(session, generation) {
            return None;
        }
        let session = self.begin_trigger_request(false)?;
        self.triggers.config_batch.cancel();
        Some((
            session,
            self.triggers.config_batch.generation,
            self.triggers.config_batch.text.clone(),
        ))
    }
    fn receive_config_batch_preview(
        &mut self,
        session: u64,
        generation: u64,
        result: Result<TriggerBatchPreview, aftercast::Error>,
    ) -> bool {
        if !self.finish_trigger_request(session) || !self.config_batch_current(session, generation)
        {
            return false;
        }
        match result {
            Ok(preview) => self.triggers.config_batch.preview = Some(preview),
            Err(error) => self.fail(error),
        }
        true
    }
    fn begin_config_batch_import(
        &mut self,
        session: u64,
        generation: u64,
    ) -> Option<(u64, TriggerBatchPreview)> {
        if self.screen != "triggers" || !self.config_batch_current(session, generation) {
            return None;
        }
        let preview = self
            .triggers
            .config_batch
            .preview
            .clone()
            .filter(|item| item.conflicts.is_empty())?;
        let session = self.begin_trigger_request(true)?;
        Some((session, preview))
    }
    fn receive_config_batch_import(
        &mut self,
        session: u64,
        generation: u64,
        result: Result<aftercast::TriggerRevision, aftercast::Error>,
        message: String,
    ) -> bool {
        if !self.config_batch_current(session, generation) {
            return false;
        }
        match result {
            Ok(_) => {
                self.triggers.config_batch.cancel();
                self.triggers.loaded = false;
                self.message = message;
            }
            Err(error) => self.fail(error),
        }
        self.finish_trigger_request(session)
    }
}
fn preview(mut state: Signal<AppState>, session: u64, generation: u64) {
    let Some(handle) = state.peek().handle() else {
        return;
    };
    let Some((session, generation, text)) = state
        .write()
        .begin_config_batch_preview(session, generation)
    else {
        return;
    };
    spawn_forever(async move {
        let result = handle.preview_trigger_config(&text).await;
        state
            .write()
            .receive_config_batch_preview(session, generation, result);
    });
}
fn import(mut state: Signal<AppState>, session: u64, generation: u64) {
    let Some(handle) = state.peek().handle() else {
        return;
    };
    let Some((session, preview)) = state.write().begin_config_batch_import(session, generation)
    else {
        return;
    };
    spawn_forever(async move {
        let definitions = preview
            .items
            .iter()
            .map(|item| item.definition.clone())
            .collect::<Vec<_>>();
        let result = handle
            .save_trigger_batch(preview.revision, &definitions)
            .await;
        state.write().receive_config_batch_import(
            session,
            generation,
            result,
            tr("trigger-config-imported"),
        );
    });
}
#[component]
pub(super) fn Panel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let ready = current.phase == Phase::Ready && !current.triggers.busy;
    let text = current.triggers.config_batch.text.clone();
    let item = current.triggers.config_batch.preview.clone();
    let session = current.revision.session;
    let generation = current.triggers.config_batch.generation;
    drop(current);
    rsx! {
        Accordion {
            class: "rounded-md border border-border".to_owned(),
            header: rsx! {
                {tr("trigger-config-title")}
            },
            div { class: "space-y-3 p-4",
                p { class: "text-sm text-muted-foreground", {tr("trigger-config-help")} }
                Label { html_for: "trigger-config", {tr("trigger-config-input")} }
                TextArea {
                    id: "trigger-config".to_owned(),
                    value: text,
                    disabled: !ready,
                    rows: 4,
                    oninput: move |event: FormEvent| {
                        let mut current = state.write();
                        if current.screen == "triggers"
                            && current.config_batch_current(session, generation)
                            && current.phase == Phase::Ready && !current.triggers.busy
                        {
                            current.triggers.config_batch.edit(event.value());
                        }
                    },
                }
                div { class: "flex flex-wrap gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: !ready,
                        onclick: move |_| preview(state, session, generation),
                        {tr("trigger-config-preview")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: state.read().phase != Phase::Ready,
                        onclick: move |_| {
                            let mut current = state.write();
                            if current.screen == "triggers"
                                && current.config_batch_current(session, generation)
                                && current.phase == Phase::Ready
                            {
                                current.triggers.config_batch.cancel();
                            }
                        },
                        {tr("trigger-config-cancel")}
                    }
                }
                if let Some(batch) = item {
                    div { class: "space-y-2 rounded-md border border-border p-3 text-sm",
                        p {
                            {
                                tr(
                                    if batch.conflicts.is_empty() {
                                        "trigger-config-ready"
                                    } else {
                                        "trigger-config-conflict"
                                    },
                                )
                            }
                        }
                        for entry in batch.items.clone() {
                            div { class: "border-b border-border pb-2",
                                p { "{entry.key}" }
                                if batch.conflicts.contains(&entry.key) {
                                    p { {tr("trigger-config-item-conflict")} }
                                }
                                p {
                                    {tr("trigger-snippet-details")}
                                    " {entry.definition.sound_type} / {entry.definition.timer} / {entry.definition.tabbed} / {entry.definition.active} / {entry.definition.restrict_to_category_zone}"
                                }
                                p {
                                    {tr("trigger-snippet-sound")}
                                    " {entry.definition.sound_data}"
                                }
                                p {
                                    {tr("trigger-snippet-timer")}
                                    " {entry.definition.timer_name}"
                                }
                            }
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: !ready || !batch.conflicts.is_empty(),
                            onclick: move |_| import(state, session, generation),
                            {tr("trigger-config-import")}
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
pub(super) async fn test_contract(host: &aftercast::Host, ui: &mut AppState) {
    ui.screen = "triggers";
    ui.phase = Phase::Ready;
    let session = ui.revision.session;
    ui.triggers.config_batch.edit("<Config><CustomTriggers><Trigger Regex='batch-one' Category='UI'/><Trigger Regex='batch-two' Category='UI'/></CustomTriggers></Config>".into());
    let generation = ui.triggers.config_batch.generation;
    let (_, pending, text) = ui
        .begin_config_batch_preview(session, generation)
        .expect("begin preview");
    assert!(ui.begin_config_batch_preview(session, pending).is_none());
    let reply = host.preview_trigger_config(&text).await;
    ui.triggers.config_batch.cancel();
    assert!(
        !ui.receive_config_batch_preview(session, pending, reply),
        "cancelled response"
    );
    let generation = ui.triggers.config_batch.generation;
    let (_, pending, text) = ui
        .begin_config_batch_preview(session, generation)
        .expect("preview again");
    let reply = host.preview_trigger_config(&text).await;
    assert!(ui.receive_config_batch_preview(session, pending, reply));
    let preview = ui.triggers.config_batch.preview.clone().expect("preview");
    let remote = aftercast::TriggerDefinition {
        category: "UI-other".into(),
        regex: "other".into(),
        ..Default::default()
    };
    host.save_trigger(preview.revision, None, &remote)
        .await
        .expect("concurrent edit");
    let (_, selected) = ui
        .begin_config_batch_import(session, pending)
        .expect("import");
    let definitions = selected
        .items
        .iter()
        .map(|item| item.definition.clone())
        .collect::<Vec<_>>();
    let failed = host
        .save_trigger_batch(selected.revision, &definitions)
        .await;
    failed.as_ref().expect_err("stale batch");
    assert!(ui.receive_config_batch_import(session, pending, failed, "saved".into()));
    assert!(ui.triggers.config_batch.preview.is_some());
    let (_, pending, text) = ui
        .begin_config_batch_preview(session, pending)
        .expect("refresh");
    let reply = host.preview_trigger_config(&text).await;
    assert!(ui.receive_config_batch_preview(session, pending, reply));
    let (_, selected) = ui
        .begin_config_batch_import(session, pending)
        .expect("import again");
    let definitions = selected
        .items
        .iter()
        .map(|item| item.definition.clone())
        .collect::<Vec<_>>();
    let saved = host
        .save_trigger_batch(selected.revision, &definitions)
        .await;
    ui.screen = "analysis";
    assert!(ui.receive_config_batch_import(session, pending, saved, "saved".into()));
    assert!(ui.triggers.config_batch.preview.is_none());
    assert!(
        ui.begin_config_batch_preview(session, pending).is_none(),
        "hidden stale event"
    );
    verify_conflict_and_late(host, ui, session).await;
}

#[cfg(test)]
async fn verify_conflict_and_late(host: &aftercast::Host, ui: &mut AppState, session: u64) {
    ui.screen = "triggers";
    let generation = ui.triggers.config_batch.generation;
    let (_, conflict_generation, text) = ui
        .begin_config_batch_preview(session, generation)
        .expect("conflict preview");
    let reply = host.preview_trigger_config(&text).await;
    assert!(ui.receive_config_batch_preview(session, conflict_generation, reply));
    assert_eq!(
        ui.triggers
            .config_batch
            .preview
            .as_ref()
            .expect("conflicts")
            .conflicts
            .len(),
        2
    );
    assert!(
        ui.begin_config_batch_import(session, conflict_generation)
            .is_none(),
        "conflicts cannot be imported"
    );
    let generation = ui.triggers.config_batch.generation;
    let (_, pending, text) = ui
        .begin_config_batch_preview(session, generation)
        .expect("late");
    let reply = host.preview_trigger_config(&text).await;
    ui.revision.session += 1;
    ui.triggers = Default::default();
    assert!(!ui.receive_config_batch_preview(session, pending, reply));
}
