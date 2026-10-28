use super::{AppState, Phase, tr};
use crate::components::{Accordion, Button, ButtonVariant, Label, TextArea};
use aftercast::TimerSnippetPreview;
use dioxus::{core::spawn_forever, prelude::*};

#[derive(Default)]
pub(super) struct State {
    text: String,
    generation: u64,
    preview: Option<TimerSnippetPreview>,
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
    fn timer_snippet_current(&self, session: u64, generation: u64) -> bool {
        self.revision.session == session && self.timers.snippet.generation == generation
    }
}
impl AppState {
    fn begin_timer_snippet_preview(
        &mut self,
        session: u64,
        generation: u64,
    ) -> Option<(u64, u64, String)> {
        if self.screen != "timers" || !self.timer_snippet_current(session, generation) {
            return None;
        }
        let session = self.begin_timer_request(false)?;
        self.timers.snippet.cancel();
        Some((
            session,
            self.timers.snippet.generation,
            self.timers.snippet.text.clone(),
        ))
    }
    fn receive_timer_snippet_preview(
        &mut self,
        session: u64,
        generation: u64,
        result: Result<TimerSnippetPreview, aftercast::Error>,
    ) -> bool {
        if !self.finish_timer_request(session) || !self.timer_snippet_current(session, generation) {
            return false;
        }
        match result {
            Ok(preview) => self.timers.snippet.preview = Some(preview),
            Err(error) => self.fail(error),
        }
        true
    }
    fn begin_timer_snippet_import(
        &mut self,
        session: u64,
        generation: u64,
    ) -> Option<(u64, TimerSnippetPreview)> {
        if self.screen != "timers" || !self.timer_snippet_current(session, generation) {
            return None;
        }
        let preview = self
            .timers
            .snippet
            .preview
            .clone()
            .filter(|item| !item.exists)?;
        let session = self.begin_timer_request(true)?;
        Some((session, preview))
    }
    fn receive_timer_snippet_import(
        &mut self,
        session: u64,
        generation: u64,
        result: Result<aftercast::SavedTimer, aftercast::Error>,
        message: String,
    ) -> bool {
        if !self.timer_snippet_current(session, generation) {
            return false;
        }
        match result {
            Ok(_) => {
                self.timers.snippet.cancel();
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
        .begin_timer_snippet_preview(session, generation)
    else {
        return;
    };
    spawn_forever(async move {
        let result = handle.preview_timer_snippet(&text).await;
        state
            .write()
            .receive_timer_snippet_preview(session, generation, result);
    });
}
fn import(mut state: Signal<AppState>, session: u64, generation: u64) {
    let Some(handle) = state.peek().handle() else {
        return;
    };
    let Some((session, preview)) = state
        .write()
        .begin_timer_snippet_import(session, generation)
    else {
        return;
    };
    spawn_forever(async move {
        let result = handle
            .save_timer_definition(preview.revision, None, &preview.definition)
            .await;
        state.write().receive_timer_snippet_import(
            session,
            generation,
            result,
            tr("spell-snippet-imported"),
        );
    });
}
#[component]
pub(super) fn Panel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let ready = current.phase == Phase::Ready && !current.timers.busy;
    let text = current.timers.snippet.text.clone();
    let item = current.timers.snippet.preview.clone();
    let session = current.revision.session;
    let generation = current.timers.snippet.generation;
    drop(current);
    rsx! {
        Accordion {
            class: "rounded-md border border-border".to_owned(),
            header: rsx! {
                {tr("spell-snippet-title")}
            },
            div { class: "space-y-3 p-4",
                p { class: "text-sm text-muted-foreground", {tr("spell-snippet-help")} }
                Label { html_for: "spell-snippet", {tr("spell-snippet-input")} }
                TextArea {
                    id: "spell-snippet".to_owned(),
                    value: text,
                    disabled: !ready,
                    rows: 4,
                    oninput: move |event: FormEvent| {
                        let mut current = state.write();
                        if current.screen == "timers"
                            && current.timer_snippet_current(session, generation)
                            && current.phase == Phase::Ready && !current.timers.busy
                        {
                            current.timers.snippet.edit(event.value());
                        }
                    },
                }
                div { class: "flex flex-wrap gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: !ready,
                        onclick: move |_| preview(state, session, generation),
                        {tr("spell-snippet-preview")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: state.read().phase != Phase::Ready,
                        onclick: move |_| {
                            let mut current = state.write();
                            if current.screen == "timers"
                                && current.timer_snippet_current(session, generation)
                                && current.phase == Phase::Ready
                            {
                                current.timers.snippet.cancel();
                            }
                        },
                        {tr("spell-snippet-cancel")}
                    }
                }
                if let Some(item) = item {
                    div { class: "space-y-2 rounded-md border border-border p-3 text-sm",
                        p { "{item.key}" }
                        p {
                            {tr(if item.exists { "spell-snippet-conflict" } else { "spell-snippet-ready" })}
                        }
                        p {
                            {tr("spell-snippet-details")}
                            " {item.definition.timer_value} / {item.definition.warning_value} / {item.definition.remove_value}"
                        }
                        p {
                            {tr("spell-snippet-flags")}
                            " {item.definition.restrict_to_me} / {item.definition.only_master_ticks} / {item.definition.absolute_timing} / {item.definition.radial_display} / {item.definition.modable} / {item.definition.restrict_to_category} / {item.definition.active_in_list} / {item.definition.panel1_display} / {item.definition.panel2_display}"
                        }
                        p {
                            {tr("spell-snippet-color")}
                            " {item.definition.fill_color_argb}"
                        }
                        p {
                            {tr("spell-snippet-tooltip")}
                            " {item.definition.tooltip}"
                        }
                        p {
                            {tr("spell-snippet-sound")}
                            " {item.definition.start_sound_data} / {item.definition.warning_sound_data}"
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: !ready || item.exists,
                            onclick: move |_| import(state, session, generation),
                            {tr("spell-snippet-import")}
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aftercast::{Host, HostConfig, TimerDefinition, extract_bridge};
    use camino::{Utf8Path, Utf8PathBuf};

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "requires .NET 10; run mise run test:gui-state"]
    async fn spell_snippet_ui_contract_cancel_stale_import_and_session() {
        let temp = tempfile::tempdir().expect("isolated data");
        let root = Utf8Path::from_path(temp.path()).expect("UTF8");
        let bridge = root.join("bridge");
        extract_bridge(&bridge).expect("bridge");
        let executable = std::env::current_exe().expect("test executable");
        let application = executable
            .parent()
            .and_then(std::path::Path::parent)
            .expect("target")
            .join("examples")
            .join(format!("test-application{}", std::env::consts::EXE_SUFFIX));
        let host = Host::start(
            HostConfig::builder()
                .dotnet_root(std::env::var("DOTNET_ROOT").expect("dotnet"))
                .bridge_directory(bridge)
                .data_directory(root.join("state"))
                .application_executable(Utf8PathBuf::try_from(application).expect("UTF8"))
                .build(),
        )
        .await
        .expect("host");
        let mut ui = AppState::new(root.to_owned());
        ui.phase = Phase::Ready;
        ui.screen = "timers";
        ui.revision.session = 1;
        ui.timers
            .snippet
            .edit("<Spell N='one' C='Fixture' T='45' />".into());
        let generation = ui.timers.snippet.generation;
        let (session, pending, input) = ui
            .begin_timer_snippet_preview(1, generation)
            .expect("preview");
        assert!(
            ui.begin_timer_snippet_preview(1, pending).is_none(),
            "duplicate"
        );
        let reply = host.preview_timer_snippet(&input).await;
        ui.timers.snippet.cancel();
        assert!(
            !ui.receive_timer_snippet_preview(session, pending, reply),
            "cancel rejects late preview"
        );
        assert!(ui.timers.snippet.preview.is_none());
        let current = ui.timers.snippet.generation;
        let (session, pending, input) = ui.begin_timer_snippet_preview(1, current).expect("repeat");
        let reply = host.preview_timer_snippet(&input).await;
        assert!(ui.receive_timer_snippet_preview(session, pending, reply));
        let preview = ui.timers.snippet.preview.clone().expect("shown preview");
        let remote = TimerDefinition {
            category: "Other".into(),
            name: "other".into(),
            ..Default::default()
        };
        host.save_timer_definition(preview.revision, None, &remote)
            .await
            .expect("concurrent change");
        let (session, chosen) = ui
            .begin_timer_snippet_import(1, pending)
            .expect("selected import");
        let failure = host
            .save_timer_definition(chosen.revision, None, &chosen.definition)
            .await;
        failure.as_ref().expect_err("stale revision");
        assert!(ui.receive_timer_snippet_import(session, pending, failure, "done".into()));
        assert!(
            ui.timers.snippet.preview.is_some(),
            "failed save preserves preview"
        );
        let (session, pending, input) = ui
            .begin_timer_snippet_preview(1, pending)
            .expect("refresh preview");
        let reply = host.preview_timer_snippet(&input).await;
        assert!(ui.receive_timer_snippet_preview(session, pending, reply));
        let (session, chosen) = ui.begin_timer_snippet_import(1, pending).expect("retry");
        let success = host
            .save_timer_definition(chosen.revision, None, &chosen.definition)
            .await;
        ui.screen = "analysis";
        assert!(ui.receive_timer_snippet_import(session, pending, success, "done".into()));
        assert!(ui.timers.snippet.preview.is_none());
        assert_eq!(host.timer_definitions(0, 16).await.expect("saved").total, 2);
        assert!(
            ui.begin_timer_snippet_preview(1, pending).is_none(),
            "stale hidden event"
        );
        ui.screen = "timers";
        let current = ui.timers.snippet.generation;
        let (session, pending, input) = ui
            .begin_timer_snippet_preview(1, current)
            .expect("preview before stop");
        let reply = host.preview_timer_snippet(&input).await;
        ui.revision.session += 1;
        ui.timers = Default::default();
        assert!(
            !ui.receive_timer_snippet_preview(session, pending, reply),
            "old session"
        );
        host.shutdown().await.expect("shutdown");
    }
}
