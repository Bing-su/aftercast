use super::{AppState, Phase};
use crate::components::{Accordion, Button, ButtonVariant, Label, TextArea};
use aftercast::MixedConfigPreview;
use dioxus::{core::spawn_forever, prelude::*};

#[derive(Default)]
pub(super) struct State {
    text: String,
    generation: u64,
    preview: Option<MixedConfigPreview>,
}
impl State {
    fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.preview = None;
    }
    fn edit(&mut self, text: String) {
        self.text = text;
        self.cancel();
    }
}
impl AppState {
    fn mixed_current(&self, session: u64, generation: u64) -> bool {
        self.revision.session == session && self.triggers.mixed_config.generation == generation
    }
    fn mixed_requires_retirement(
        &self,
        session: u64,
        result: &Result<(), aftercast::Error>,
    ) -> bool {
        self.revision.session == session
            && matches!(
                result,
                Err(aftercast::Error::MixedConfigHostFault(_)
                    | aftercast::Error::WorkerFailed(_)
                    | aftercast::Error::Closed
                    | aftercast::Error::InvalidTriggerRequest(_))
            )
    }
    fn begin_mixed_preview(&mut self, session: u64, generation: u64) -> Option<(u64, u64, String)> {
        if self.screen != "triggers" || !self.mixed_current(session, generation) {
            return None;
        }
        let session = self.begin_trigger_request(false)?;
        self.triggers.mixed_config.cancel();
        Some((
            session,
            self.triggers.mixed_config.generation,
            self.triggers.mixed_config.text.clone(),
        ))
    }
    fn receive_mixed_preview(
        &mut self,
        session: u64,
        generation: u64,
        result: Result<MixedConfigPreview, aftercast::Error>,
    ) -> bool {
        if !self.finish_trigger_request(session) || !self.mixed_current(session, generation) {
            return false;
        }
        match result {
            Ok(preview) => self.triggers.mixed_config.preview = Some(preview),
            Err(error) => self.fail(error),
        }
        true
    }
    fn begin_mixed_import(
        &mut self,
        session: u64,
        generation: u64,
    ) -> Option<(u64, MixedConfigPreview)> {
        if self.screen != "triggers" || !self.mixed_current(session, generation) {
            return None;
        }
        let preview = self
            .triggers
            .mixed_config
            .preview
            .clone()
            .filter(|p| p.trigger_conflicts.is_empty() && p.timer_conflicts.is_empty())?;
        let session = self.begin_trigger_request(true)?;
        Some((session, preview))
    }
    fn receive_mixed_import(
        &mut self,
        session: u64,
        generation: u64,
        result: Result<(), aftercast::Error>,
    ) -> bool {
        if !self.mixed_current(session, generation) {
            return false;
        }
        match result {
            Ok(()) => {
                self.triggers.mixed_config.cancel();
                self.triggers.loaded = false;
                self.timers.loaded = false;
                self.message = "Mixed Config imported.".into();
            }
            Err(error) => {
                self.triggers.mixed_config.cancel();
                self.fail(error);
            }
        }
        self.finish_trigger_request(session)
    }
}
fn preview(mut state: Signal<AppState>, session: u64, generation: u64) {
    let Some(handle) = state.peek().handle() else {
        return;
    };
    let Some((session, generation, text)) = state.write().begin_mixed_preview(session, generation)
    else {
        return;
    };
    spawn_forever(async move {
        let result = handle.preview_mixed_config(&text).await;
        state
            .write()
            .receive_mixed_preview(session, generation, result);
    });
}
fn import(mut state: Signal<AppState>, session: u64, generation: u64) {
    let Some(handle) = state.peek().handle() else {
        return;
    };
    let Some((session, preview)) = state.write().begin_mixed_import(session, generation) else {
        return;
    };
    spawn_forever(async move {
        let result = handle.import_mixed_config(&preview).await;
        if state.peek().mixed_requires_retirement(session, &result) {
            let draft = state.peek().triggers.mixed_config.text.clone();
            if let Err(error) = result {
                state.write().fail(error);
            }
            super::super::stop(state);
            state.write().triggers.mixed_config.text = draft;
        } else {
            state
                .write()
                .receive_mixed_import(session, generation, result);
        }
    });
}
#[component]
pub(super) fn Panel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let ready = current.phase == Phase::Ready && !current.triggers.busy;
    let text = current.triggers.mixed_config.text.clone();
    let item = current.triggers.mixed_config.preview.clone();
    let session = current.revision.session;
    let generation = current.triggers.mixed_config.generation;
    drop(current);
    rsx! {
        Accordion {
            class: "rounded-md border border-border".to_owned(),
            header: rsx! { "Mixed Trigger / Spell Config" },
            div { class: "space-y-3 p-4",
                p { class: "text-sm text-muted-foreground",
                    "Preview does not change settings. Import adds both sections; existing keys are not overwritten."
                }
                Label { html_for: "mixed-config", "Config XML" }
                TextArea {
                    id: "mixed-config".to_owned(),
                    value: text,
                    disabled: !ready,
                    rows: 4,
                    oninput: move |event: FormEvent| {
                        let mut current = state.write();
                        if current.screen == "triggers" && current.mixed_current(session, generation)
                            && current.phase == Phase::Ready && !current.triggers.busy
                        {
                            current.triggers.mixed_config.edit(event.value());
                        }
                    },
                }
                div { class: "flex flex-wrap gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: !ready,
                        onclick: move |_| preview(state, session, generation),
                        "Preview"
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: state.read().phase != Phase::Ready,
                        onclick: move |_| {
                            let mut current = state.write();
                            if current.screen == "triggers" && current.mixed_current(session, generation)
                                && current.phase == Phase::Ready
                            {
                                current.triggers.mixed_config.cancel();
                            }
                        },
                        "Cancel"
                    }
                }
                if let Some(batch) = item {
                    div { class: "space-y-2 rounded-md border border-border p-3 text-sm",
                        p { "Triggers" }
                        for entry in batch.triggers.clone() {
                            p {
                                "{entry.key}"
                                if batch.trigger_conflicts.contains(&entry.key) {
                                    " (already exists)"
                                }
                            }
                        }
                        p { "Timers" }
                        for entry in batch.timers.clone() {
                            p {
                                "{entry.key}"
                                if batch.timer_conflicts.contains(&entry.key) {
                                    " (already exists)"
                                }
                            }
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: !ready || !batch.trigger_conflicts.is_empty() || !batch.timer_conflicts.is_empty(),
                            onclick: move |_| import(state, session, generation),
                            "Import"
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
    let xml = "<Config><CustomTriggers><Trigger Regex='mixed-controller' Category='Fixture'/></CustomTriggers><SpellTimers><Spell Name='mixed-controller' Category='Fixture'/></SpellTimers></Config>";
    ui.triggers.mixed_config.edit(xml.into());
    let generation = ui.triggers.mixed_config.generation;
    let (_, pending, text) = ui.begin_mixed_preview(session, generation).unwrap();
    let reply = host.preview_mixed_config(&text).await;
    ui.triggers.mixed_config.cancel();
    assert!(!ui.receive_mixed_preview(session, pending, reply));
    assert!(ui.triggers.mixed_config.preview.is_none());
    let generation = ui.triggers.mixed_config.generation;
    let (_, pending, text) = ui.begin_mixed_preview(session, generation).unwrap();
    let reply = host.preview_mixed_config(&text).await;
    assert!(ui.receive_mixed_preview(session, pending, reply));
    ui.triggers.mixed_config.edit(xml.to_owned() + " ");
    assert!(
        ui.begin_mixed_import(session, pending).is_none(),
        "editing invalidates snapshot"
    );
    let generation = ui.triggers.mixed_config.generation;
    let (_, pending, text) = ui.begin_mixed_preview(session, generation).unwrap();
    let reply = host.preview_mixed_config(&text).await;
    assert!(ui.receive_mixed_preview(session, pending, reply));
    verify_stale_and_commit(host, ui, session, pending, xml).await;
}

#[cfg(test)]
async fn verify_stale_and_commit(
    host: &aftercast::Host,
    ui: &mut AppState,
    session: u64,
    pending: u64,
    xml: &str,
) {
    let stale = ui.triggers.mixed_config.preview.clone().unwrap();
    let current = host.timer_definitions(0, 16).await.unwrap();
    let extra = aftercast::TimerDefinition {
        name: "mixed-controller-extra".into(),
        category: "Fixture".into(),
        ..Default::default()
    };
    host.save_timer_definition(current.revision, None, &extra)
        .await
        .unwrap();
    let (_, selected) = ui.begin_mixed_import(session, pending).unwrap();
    assert!(host.import_mixed_config(&stale).await.is_err());
    let stale_result = host.import_mixed_config(&selected).await;
    assert!(stale_result.is_err());
    assert!(ui.receive_mixed_import(session, pending, stale_result));
    assert!(ui.triggers.mixed_config.preview.is_none());
    assert_eq!(ui.triggers.mixed_config.text, xml.to_owned() + " ");
    verify_trigger_stale(host, ui, session).await;
    let generation = ui.triggers.mixed_config.generation;
    let (_, pending, text) = ui.begin_mixed_preview(session, generation).unwrap();
    let reply = host.preview_mixed_config(&text).await;
    assert!(ui.receive_mixed_preview(session, pending, reply));
    let (_, selected) = ui.begin_mixed_import(session, pending).unwrap();
    assert!(
        ui.begin_mixed_import(session, pending).is_none(),
        "duplicate click"
    );
    let result = host.import_mixed_config(&selected).await;
    assert!(ui.receive_mixed_import(session, pending, result));
    assert!(ui.triggers.mixed_config.preview.is_none());
    assert_eq!(ui.triggers.mixed_config.text, xml.to_owned() + " ");
    let generation = ui.triggers.mixed_config.generation;
    let (_, pending, text) = ui.begin_mixed_preview(session, generation).unwrap();
    let reply = host.preview_mixed_config(&text).await;
    verify_late_fault(ui, session, pending, reply);
}

#[cfg(test)]
async fn verify_trigger_stale(host: &aftercast::Host, ui: &mut AppState, session: u64) {
    let generation = ui.triggers.mixed_config.generation;
    let (_, pending, text) = ui.begin_mixed_preview(session, generation).unwrap();
    let reply = host.preview_mixed_config(&text).await;
    assert!(ui.receive_mixed_preview(session, pending, reply));
    let current = host.triggers(0, 16).await.unwrap();
    let extra = aftercast::TriggerDefinition {
        category: "Fixture".into(),
        regex: "mixed-controller-extra".into(),
        ..Default::default()
    };
    host.save_trigger(current.revision, None, &extra)
        .await
        .unwrap();
    let (_, selected) = ui.begin_mixed_import(session, pending).unwrap();
    let result = host.import_mixed_config(&selected).await;
    result.as_ref().expect_err("stale trigger revision");
    assert!(ui.receive_mixed_import(session, pending, result));
    assert!(ui.triggers.mixed_config.preview.is_none());
}

#[cfg(test)]
fn verify_late_fault(
    ui: &mut AppState,
    session: u64,
    pending: u64,
    reply: Result<MixedConfigPreview, aftercast::Error>,
) {
    let fault = Err(aftercast::Error::MixedConfigHostFault("injected".into()));
    assert!(ui.mixed_requires_retirement(session, &fault));
    ui.triggers.mixed_config.cancel();
    assert!(
        ui.mixed_requires_retirement(session, &fault),
        "late fault still retires owner"
    );
    ui.revision.session += 1;
    assert!(
        !ui.mixed_requires_retirement(session, &fault),
        "old fault cannot stop new session"
    );
    assert!(!ui.receive_mixed_preview(session, pending, reply));
    ui.revision.session = session;
    ui.triggers.busy = false;
    ui.phase = Phase::Ready;
}
