use super::{AppState, Phase, tr};
use crate::components::{Accordion, Button, ButtonVariant, Label, TextArea};
use aftercast::TriggerSnippetPreview;
use dioxus::{core::spawn_forever, prelude::*};

#[derive(Default)]
pub(super) struct State {
    text: String,
    generation: u64,
    preview: Option<TriggerSnippetPreview>,
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
    fn snippet_current(&self, session: u64, generation: u64) -> bool {
        self.revision.session == session && self.triggers.snippet.generation == generation
    }
}
impl AppState {
    fn begin_snippet_preview(
        &mut self,
        session: u64,
        generation: u64,
    ) -> Option<(u64, u64, String)> {
        if self.screen != "triggers" || !self.snippet_current(session, generation) {
            return None;
        }
        let session = self.begin_trigger_request(false)?;
        self.triggers.snippet.cancel();
        Some((
            session,
            self.triggers.snippet.generation,
            self.triggers.snippet.text.clone(),
        ))
    }
    fn receive_snippet_preview(
        &mut self,
        session: u64,
        generation: u64,
        result: Result<TriggerSnippetPreview, aftercast::Error>,
    ) -> bool {
        if !self.finish_trigger_request(session) || !self.snippet_current(session, generation) {
            return false;
        }
        match result {
            Ok(preview) => self.triggers.snippet.preview = Some(preview),
            Err(error) => self.fail(error),
        }
        true
    }
    fn begin_snippet_import(
        &mut self,
        session: u64,
        generation: u64,
    ) -> Option<(u64, TriggerSnippetPreview)> {
        if self.screen != "triggers" || !self.snippet_current(session, generation) {
            return None;
        }
        let preview = self
            .triggers
            .snippet
            .preview
            .clone()
            .filter(|item| !item.exists)?;
        let session = self.begin_trigger_request(true)?;
        Some((session, preview))
    }
    fn receive_snippet_import(
        &mut self,
        session: u64,
        generation: u64,
        result: Result<aftercast::TriggerRevision, aftercast::Error>,
        message: String,
    ) -> bool {
        if !self.snippet_current(session, generation) {
            return false;
        }
        match result {
            Ok(_) => {
                self.triggers.snippet.cancel();
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
    let Some((session, generation, text)) =
        state.write().begin_snippet_preview(session, generation)
    else {
        return;
    };
    spawn_forever(async move {
        let result = handle.preview_trigger_snippet(&text).await;
        state
            .write()
            .receive_snippet_preview(session, generation, result);
    });
}
fn import(mut state: Signal<AppState>, session: u64, generation: u64) {
    let Some(handle) = state.peek().handle() else {
        return;
    };
    let Some((session, preview)) = state.write().begin_snippet_import(session, generation) else {
        return;
    };
    spawn_forever(async move {
        let result = handle
            .save_trigger(preview.revision, None, &preview.definition)
            .await;
        state.write().receive_snippet_import(
            session,
            generation,
            result,
            tr("trigger-snippet-imported"),
        );
    });
}
#[component]
pub(super) fn Panel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let ready = current.phase == Phase::Ready && !current.triggers.busy;
    let text = current.triggers.snippet.text.clone();
    let item = current.triggers.snippet.preview.clone();
    let session = current.revision.session;
    let generation = current.triggers.snippet.generation;
    drop(current);
    rsx! {
        Accordion {
            class: "rounded-md border border-border".to_owned(),
            header: rsx! {
                {tr("trigger-snippet-title")}
            },
            div { class: "space-y-3 p-4",
                p { class: "text-sm text-muted-foreground", {tr("trigger-snippet-help")} }
                Label { html_for: "trigger-snippet", {tr("trigger-snippet-input")} }
                TextArea {
                    id: "trigger-snippet".to_owned(),
                    value: text,
                    disabled: !ready,
                    rows: 4,
                    oninput: move |event: FormEvent| {
                        let mut current = state.write();
                        if current.screen == "triggers" && current.snippet_current(session, generation)
                            && current.phase == Phase::Ready && !current.triggers.busy
                        {
                            current.triggers.snippet.edit(event.value());
                        }
                    },
                }
                div { class: "flex flex-wrap gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: !ready,
                        onclick: move |_| preview(state, session, generation),
                        {tr("trigger-snippet-preview")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: state.read().phase != Phase::Ready,
                        onclick: move |_| {
                            let mut current = state.write();
                            if current.screen == "triggers" && current.snippet_current(session, generation)
                                && current.phase == Phase::Ready
                            {
                                current.triggers.snippet.cancel();
                            }
                        },
                        {tr("trigger-snippet-cancel")}
                    }
                }
                if let Some(item) = item {
                    div { class: "space-y-2 rounded-md border border-border p-3 text-sm",
                        p { "{item.key}" }
                        p {
                            {
                                tr(
                                    if item.exists {
                                        "trigger-snippet-conflict"
                                    } else {
                                        "trigger-snippet-ready"
                                    },
                                )
                            }
                        }
                        p {
                            {tr("trigger-snippet-details")}
                            " {item.definition.sound_type} / {item.definition.timer} / {item.definition.tabbed} / {item.definition.active} / {item.definition.restrict_to_category_zone}"
                        }
                        p {
                            {tr("trigger-snippet-sound")}
                            " {item.definition.sound_data}"
                        }
                        p {
                            {tr("trigger-snippet-timer")}
                            " {item.definition.timer_name}"
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: !ready || item.exists,
                            onclick: move |_| import(state, session, generation),
                            {tr("trigger-snippet-import")}
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
    use aftercast::{Host, HostConfig, TriggerDefinition, extract_bridge};
    use camino::{Utf8Path, Utf8PathBuf};

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "requires .NET 10; run mise run test:gui-state"]
    async fn snippet_ui_contract_cancel_stale_import_and_session() {
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
        ui.screen = "triggers";
        ui.revision.session = 1;
        ui.triggers
            .snippet
            .edit("<Trigger R='one' C='Fixture' Ta='T' />".into());
        let generation = ui.triggers.snippet.generation;
        let (session, pending, input) = ui.begin_snippet_preview(1, generation).expect("preview");
        assert!(ui.begin_snippet_preview(1, pending).is_none(), "duplicate");
        let reply = host.preview_trigger_snippet(&input).await;
        ui.triggers.snippet.cancel();
        assert!(
            !ui.receive_snippet_preview(session, pending, reply),
            "cancel rejects late preview"
        );
        assert!(ui.triggers.snippet.preview.is_none());
        let current = ui.triggers.snippet.generation;
        let (session, pending, input) = ui.begin_snippet_preview(1, current).expect("repeat");
        let reply = host.preview_trigger_snippet(&input).await;
        assert!(ui.receive_snippet_preview(session, pending, reply));
        let preview = ui.triggers.snippet.preview.clone().expect("shown preview");
        let remote = TriggerDefinition {
            category: "Other".into(),
            regex: "other".into(),
            ..Default::default()
        };
        host.save_trigger(preview.revision, None, &remote)
            .await
            .expect("concurrent change");
        let (session, chosen) = ui
            .begin_snippet_import(1, pending)
            .expect("selected import");
        let failure = host
            .save_trigger(chosen.revision, None, &chosen.definition)
            .await;
        failure.as_ref().expect_err("stale revision");
        assert!(ui.receive_snippet_import(session, pending, failure, "done".into()));
        assert!(
            ui.triggers.snippet.preview.is_some(),
            "failed save preserves preview"
        );
        let (session, pending, input) = ui
            .begin_snippet_preview(1, pending)
            .expect("refresh preview");
        let reply = host.preview_trigger_snippet(&input).await;
        assert!(ui.receive_snippet_preview(session, pending, reply));
        let (session, chosen) = ui.begin_snippet_import(1, pending).expect("retry");
        let success = host
            .save_trigger(chosen.revision, None, &chosen.definition)
            .await;
        ui.screen = "analysis";
        assert!(ui.receive_snippet_import(session, pending, success, "done".into()));
        assert!(ui.triggers.snippet.preview.is_none());
        assert_eq!(host.triggers(0, 16).await.expect("saved").total, 2);
        assert!(
            ui.begin_snippet_preview(1, pending).is_none(),
            "stale hidden event"
        );
        ui.screen = "triggers";
        let current = ui.triggers.snippet.generation;
        let (session, pending, input) = ui
            .begin_snippet_preview(1, current)
            .expect("preview before stop");
        let reply = host.preview_trigger_snippet(&input).await;
        ui.revision.session += 1;
        ui.triggers = Default::default();
        assert!(
            !ui.receive_snippet_preview(session, pending, reply),
            "old session"
        );
        host.shutdown().await.expect("shutdown");
    }
}
