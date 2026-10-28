use super::{AppState, Phase, tr};
use crate::components::{Accordion, Button, ButtonVariant, Label, TextArea};
use aftercast::TimerBatchPreview;
use dioxus::{core::spawn_forever, prelude::*};

#[derive(Default)]
pub(super) struct State {
    text: String,
    generation: u64,
    preview: Option<TimerBatchPreview>,
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
    fn timer_config_batch_current(&self, session: u64, generation: u64) -> bool {
        self.revision.session == session && self.timers.config_batch.generation == generation
    }
}
impl AppState {
    fn begin_timer_config_batch_preview(
        &mut self,
        session: u64,
        generation: u64,
    ) -> Option<(u64, u64, String)> {
        if self.screen != "timers" || !self.timer_config_batch_current(session, generation) {
            return None;
        }
        let session = self.begin_timer_request(false)?;
        self.timers.config_batch.cancel();
        Some((
            session,
            self.timers.config_batch.generation,
            self.timers.config_batch.text.clone(),
        ))
    }
    fn receive_timer_config_batch_preview(
        &mut self,
        session: u64,
        generation: u64,
        result: Result<TimerBatchPreview, aftercast::Error>,
    ) -> bool {
        if !self.finish_timer_request(session)
            || !self.timer_config_batch_current(session, generation)
        {
            return false;
        }
        match result {
            Ok(preview) => self.timers.config_batch.preview = Some(preview),
            Err(error) => self.fail(error),
        }
        true
    }
    fn begin_timer_config_batch_import(
        &mut self,
        session: u64,
        generation: u64,
    ) -> Option<(u64, TimerBatchPreview)> {
        if self.screen != "timers" || !self.timer_config_batch_current(session, generation) {
            return None;
        }
        let preview = self
            .timers
            .config_batch
            .preview
            .clone()
            .filter(|item| item.conflicts.is_empty())?;
        let session = self.begin_timer_request(true)?;
        Some((session, preview))
    }
    fn receive_timer_config_batch_import(
        &mut self,
        session: u64,
        generation: u64,
        result: Result<aftercast::TimerRevision, aftercast::Error>,
        message: String,
    ) -> bool {
        if !self.timer_config_batch_current(session, generation) {
            return false;
        }
        match result {
            Ok(_) => {
                self.timers.config_batch.cancel();
                self.timers.loaded = false;
                self.message = message;
            }
            Err(error) => self.fail(error),
        }
        self.finish_timer_request(session)
    }
}
fn preview(mut state: Signal<AppState>, session: u64, generation: u64) {
    let Some(handle) = state.peek().handle() else {
        return;
    };
    let Some((session, generation, text)) = state
        .write()
        .begin_timer_config_batch_preview(session, generation)
    else {
        return;
    };
    spawn_forever(async move {
        let result = handle.preview_timer_config(&text).await;
        state
            .write()
            .receive_timer_config_batch_preview(session, generation, result);
    });
}
fn import(mut state: Signal<AppState>, session: u64, generation: u64) {
    let Some(handle) = state.peek().handle() else {
        return;
    };
    let Some((session, preview)) = state
        .write()
        .begin_timer_config_batch_import(session, generation)
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
            .save_timer_batch(preview.revision, &definitions)
            .await;
        state.write().receive_timer_config_batch_import(
            session,
            generation,
            result,
            tr("timer-config-imported"),
        );
    });
}
#[component]
pub(super) fn Panel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let ready = current.phase == Phase::Ready && !current.timers.busy;
    let text = current.timers.config_batch.text.clone();
    let item = current.timers.config_batch.preview.clone();
    let session = current.revision.session;
    let generation = current.timers.config_batch.generation;
    drop(current);
    rsx! {
        Accordion {
            class: "rounded-md border border-border".to_owned(),
            header: rsx! {
                {tr("timer-config-title")}
            },
            div { class: "space-y-3 p-4",
                p { class: "text-sm text-muted-foreground", {tr("timer-config-help")} }
                Label { html_for: "timer-config", {tr("timer-config-input")} }
                TextArea {
                    id: "timer-config".to_owned(),
                    value: text,
                    disabled: !ready,
                    rows: 4,
                    oninput: move |event: FormEvent| {
                        let mut current = state.write();
                        if current.screen == "timers"
                            && current.timer_config_batch_current(session, generation)
                            && current.phase == Phase::Ready && !current.timers.busy
                        {
                            current.timers.config_batch.edit(event.value());
                        }
                    },
                }
                div { class: "flex flex-wrap gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: !ready,
                        onclick: move |_| preview(state, session, generation),
                        {tr("timer-config-preview")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: state.read().phase != Phase::Ready,
                        onclick: move |_| {
                            let mut current = state.write();
                            if current.screen == "timers"
                                && current.timer_config_batch_current(session, generation)
                                && current.phase == Phase::Ready
                            {
                                current.timers.config_batch.cancel();
                            }
                        },
                        {tr("timer-config-cancel")}
                    }
                }
                if let Some(batch) = item {
                    div { class: "space-y-2 rounded-md border border-border p-3 text-sm",
                        p {
                            {
                                tr(
                                    if batch.conflicts.is_empty() {
                                        "timer-config-ready"
                                    } else {
                                        "timer-config-conflict"
                                    },
                                )
                            }
                        }
                        for entry in batch.items.clone() {
                            div { class: "border-b border-border pb-2",
                                p { "{entry.key}" }
                                if batch.conflicts.contains(&entry.key) {
                                    p { {tr("timer-config-item-conflict")} }
                                }
                                p {
                                    {tr("spell-snippet-details")}
                                    " {entry.definition.timer_value} / {entry.definition.warning_value} / {entry.definition.remove_value}"
                                }
                                p {
                                    {tr("spell-snippet-flags")}
                                    " {entry.definition.restrict_to_me} / {entry.definition.only_master_ticks} / {entry.definition.absolute_timing} / {entry.definition.radial_display} / {entry.definition.modable} / {entry.definition.restrict_to_category} / {entry.definition.active_in_list} / {entry.definition.panel1_display} / {entry.definition.panel2_display}"
                                }
                                p {
                                    {tr("spell-snippet-color")}
                                    " {entry.definition.fill_color_argb}"
                                }
                                p {
                                    {tr("spell-snippet-tooltip")}
                                    " {entry.definition.tooltip}"
                                }
                                p {
                                    {tr("spell-snippet-sound")}
                                    " {entry.definition.start_sound_data} / {entry.definition.warning_sound_data}"
                                }
                            }
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: !ready || !batch.conflicts.is_empty(),
                            onclick: move |_| import(state, session, generation),
                            {tr("timer-config-import")}
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
pub(in crate::app) async fn test_contract(host: &aftercast::Host, ui: &mut AppState) {
    ui.screen = "timers";
    ui.phase = Phase::Ready;
    let session = ui.revision.session;
    ui.timers.config_batch.edit("<Config><SpellTimers><Spell Name='Batch One' Category='UI'/><Spell Name='Batch Two' Category='UI'/></SpellTimers></Config>".into());
    let generation = ui.timers.config_batch.generation;
    let (_, pending, text) = ui
        .begin_timer_config_batch_preview(session, generation)
        .expect("begin preview");
    assert!(
        ui.begin_timer_config_batch_preview(session, pending)
            .is_none()
    );
    let reply = host.preview_timer_config(&text).await;
    ui.timers.config_batch.cancel();
    assert!(
        !ui.receive_timer_config_batch_preview(session, pending, reply),
        "cancelled response"
    );
    let generation = ui.timers.config_batch.generation;
    let (_, pending, text) = ui
        .begin_timer_config_batch_preview(session, generation)
        .expect("preview again");
    let reply = host.preview_timer_config(&text).await;
    assert!(ui.receive_timer_config_batch_preview(session, pending, reply));
    let preview = ui.timers.config_batch.preview.clone().expect("preview");
    let remote = aftercast::TimerDefinition {
        category: "UI-other".into(),
        name: "other".into(),
        ..Default::default()
    };
    host.save_timer_definition(preview.revision, None, &remote)
        .await
        .expect("concurrent edit");
    let (_, selected) = ui
        .begin_timer_config_batch_import(session, pending)
        .expect("import");
    let definitions = selected
        .items
        .iter()
        .map(|item| item.definition.clone())
        .collect::<Vec<_>>();
    let failed = host.save_timer_batch(selected.revision, &definitions).await;
    failed.as_ref().expect_err("stale batch");
    assert!(ui.receive_timer_config_batch_import(session, pending, failed, "saved".into()));
    assert!(ui.timers.config_batch.preview.is_some());
    let (_, pending, text) = ui
        .begin_timer_config_batch_preview(session, pending)
        .expect("refresh");
    let reply = host.preview_timer_config(&text).await;
    assert!(ui.receive_timer_config_batch_preview(session, pending, reply));
    let (_, selected) = ui
        .begin_timer_config_batch_import(session, pending)
        .expect("import again");
    let definitions = selected
        .items
        .iter()
        .map(|item| item.definition.clone())
        .collect::<Vec<_>>();
    let saved = host.save_timer_batch(selected.revision, &definitions).await;
    ui.screen = "analysis";
    assert!(ui.receive_timer_config_batch_import(session, pending, saved, "saved".into()));
    assert!(ui.timers.config_batch.preview.is_none());
    assert!(
        ui.begin_timer_config_batch_preview(session, pending)
            .is_none(),
        "hidden stale event"
    );
    verify_conflict_and_late(host, ui, session).await;
}

#[cfg(test)]
async fn verify_conflict_and_late(host: &aftercast::Host, ui: &mut AppState, session: u64) {
    ui.screen = "timers";
    let generation = ui.timers.config_batch.generation;
    let (_, conflict_generation, text) = ui
        .begin_timer_config_batch_preview(session, generation)
        .expect("conflict preview");
    let reply = host.preview_timer_config(&text).await;
    assert!(ui.receive_timer_config_batch_preview(session, conflict_generation, reply));
    assert_eq!(
        ui.timers
            .config_batch
            .preview
            .as_ref()
            .expect("conflicts")
            .conflicts
            .len(),
        2
    );
    assert!(
        ui.begin_timer_config_batch_import(session, conflict_generation)
            .is_none(),
        "conflicts cannot be imported"
    );
    let generation = ui.timers.config_batch.generation;
    let (_, pending, text) = ui
        .begin_timer_config_batch_preview(session, generation)
        .expect("late");
    let reply = host.preview_timer_config(&text).await;
    ui.revision.session += 1;
    ui.timers = Default::default();
    assert!(!ui.receive_timer_config_batch_preview(session, pending, reply));
}
