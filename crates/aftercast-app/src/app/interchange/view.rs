use super::super::tr;
use super::*;
use crate::components::{Accordion, Button, ButtonVariant, Input, Label};
use dioxus::{core::spawn_forever, prelude::*};
fn execute(mut state: Signal<AppState>, work: Option<(Token, Request)>) {
    let Some((token, request)) = work else { return };
    let Some(handle) = state.read().handle() else {
        return;
    };
    let complete = tr(if matches!(request, Request::Import(..)) {
        "act-import-complete"
    } else {
        "act-export-complete"
    });
    // Accepted I/O belongs to the app and is drained by Host on Stop, even after this panel unmounts.
    spawn_forever(async move {
        let result = perform(&handle, &request).await;
        if state.write().receive_interchange(token, result, &complete)
            && matches!(request, Request::Import(..))
        {
            super::super::refresh(state, 0);
        }
    });
}
fn import(mut state: Signal<AppState>, view: Token) {
    let work = state.write().begin_act_import(view);
    match work {
        Ok(work) => execute(state, work),
        Err(key) => state.write().interchange.error = Some(tr(key)),
    }
}
fn review(mut state: Signal<AppState>, view: Token) {
    let result = state.write().review_act_export(view);
    if let Err(key) = result {
        state.write().interchange.error = Some(tr(key));
    }
}
fn browse(mut state: Signal<AppState>, view: Token, direction: Direction) {
    let Some(picker) = state.write().begin_interchange_picker(view, direction) else {
        return;
    };
    let root = state.read().root.clone();
    let options = state.read().interchange.draft.options;
    spawn_forever(async move {
        if !state.peek().current_interchange_picker(&picker) {
            state.write().receive_interchange_picker(&picker, None);
            return;
        }
        let dialog = rfd::AsyncFileDialog::new()
            .set_directory(root)
            .add_filter("ACT/XML", &["act", "xml"]);
        let file = match direction {
            Direction::Import => dialog.pick_file().await,
            Direction::Export => {
                dialog
                    .set_file_name(if options.format == ActFormat::Gzip {
                        "encounter.act"
                    } else {
                        "encounter.xml"
                    })
                    .save_file()
                    .await
            }
        };
        let path = file.map(|file| Utf8PathBuf::from_path_buf(file.path().to_owned()));
        match path {
            Some(Err(path)) => {
                let accepted = state.write().receive_interchange_picker(&picker, None);
                if accepted {
                    state.write().interchange.error =
                        Some(format!("Non-UTF-8 path: {}", path.display()));
                }
            }
            path => {
                state
                    .write()
                    .receive_interchange_picker(&picker, path.and_then(Result::ok));
            }
        }
    });
}
#[component]
pub(in crate::app) fn Panel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let view = current.interchange_view();
    let ready = current.interchange_ready();
    let blocked = !matches!(current.phase, Phase::Idle | Phase::Ready)
        || current.interchange.picker.is_some()
        || current.interchange.pending.is_some();
    let draft = current.interchange.draft.clone();
    let target = current.interchange_target();
    let confirmation = current.interchange.confirmation.clone();
    let pending = current.interchange.pending.clone();
    let picker = current.interchange.picker.is_some();
    let error = current.interchange.error.clone();
    drop(current);
    rsx! {
        Accordion {
            class: "mt-6 rounded-md border border-border".to_owned(),
            default_open: true,
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! {
                {tr("act-interchange-title")}
            },
            section {
                class: "space-y-5 p-4",
                "aria-label": tr("act-interchange-title"),
                p { class: "max-w-prose text-sm leading-7 text-muted-foreground",
                    {tr("act-interchange-help")}
                }
                div { class: "space-y-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        aria_label: tr("act-import-trigger-results"),
                        aria_pressed: draft.process_triggers,
                        disabled: blocked,
                        onclick: move |_| {
                            state
                                .write()
                                .interchange_edit(
                                    view,
                                    "process-triggers",
                                    (!draft.process_triggers).to_string(),
                                );
                        },
                        {tr("act-import-trigger-results")}
                    }
                    p { class: "text-xs text-muted-foreground",
                        {tr("act-import-trigger-results-help")}
                    }
                    Label { html_for: "act-input", {tr("act-input")} }
                    Input {
                        id: "act-input".to_owned(),
                        value: draft.input,
                        disabled: blocked,
                        oninput: move |event: FormEvent| {
                            state.write().interchange_edit(view, "input", event.value());
                        },
                    }
                    div { class: "flex flex-wrap gap-2",
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: !ready,
                            onclick: move |_| browse(state, view, Direction::Import),
                            {tr("act-browse-input")}
                        }
                        Button {
                            disabled: !ready,
                            onclick: move |_| import(state, view),
                            {tr("act-import")}
                        }
                    }
                }
                div { class: "space-y-2 border-t border-border pt-4",
                    h3 { class: "text-sm font-semibold", {tr("act-export")} }
                    if let Some(target) = target {
                        p { class: "text-sm", "{target.title}" }
                    } else {
                        p { class: "text-sm text-muted-foreground", {tr("act-select-encounter")} }
                    }
                    Label { html_for: "act-output", {tr("act-output")} }
                    Input {
                        id: "act-output".to_owned(),
                        value: draft.output,
                        disabled: blocked,
                        oninput: move |event: FormEvent| {
                            state.write().interchange_edit(view, "output", event.value());
                        },
                    }
                    div { class: "flex flex-wrap gap-2",
                        for (format, key, label) in [(ActFormat::Xml, "xml", "XML"), (ActFormat::Gzip, "gzip", "ACT (gzip)")] {
                            Button {
                                variant: ButtonVariant::Outline,
                                disabled: blocked,
                                aria_pressed: draft.options.format == format,
                                onclick: move |_| {
                                    state.write().interchange_edit(view, "format", key.into());
                                },
                                "{label}"
                            }
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: blocked,
                            aria_pressed: draft.options.include_logs,
                            onclick: move |_| {
                                state
                                    .write()
                                    .interchange_edit(view, "logs", (!draft.options.include_logs).to_string());
                            },
                            {tr("act-include-logs")}
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: !ready,
                            onclick: move |_| browse(state, view, Direction::Export),
                            {tr("act-browse-output")}
                        }
                        Button {
                            disabled: !ready,
                            onclick: move |_| review(state, view),
                            {tr("act-review-export")}
                        }
                    }
                    p { class: "max-w-prose text-xs leading-6 text-muted-foreground",
                        {tr("act-no-overwrite")}
                    }
                }
                if let Some((confirmation_view, export)) = confirmation {
                    div {
                        class: "space-y-3 border-t border-border pt-4",
                        role: "group",
                        "aria-label": tr("act-confirm-title"),
                        p { class: "text-sm font-medium", "{export.target.title}" }
                        p { class: "break-all text-sm", "{export.path}" }
                        p { class: "text-xs",
                            {if export.options.format == ActFormat::Gzip { "ACT (gzip)" } else { "XML" }}
                            " · "
                            {
                                tr(
                                    if export.options.include_logs {
                                        "act-logs-included"
                                    } else {
                                        "act-logs-omitted"
                                    },
                                )
                            }
                        }
                        Button {
                            disabled: !ready,
                            onclick: move |_| {
                                let work = state.write().confirm_act_export(confirmation_view);
                                execute(state, work);
                            },
                            {tr("act-save-new")}
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: !ready,
                            onclick: move |_| {
                                state.write().cancel_act_confirmation(confirmation_view);
                            },
                            {tr("cancel")}
                        }
                    }
                }
                if picker {
                    p { role: "status", {tr("act-choosing")} }
                }
                if let Some((_, request)) = pending {
                    p { role: "status", class: "break-all text-sm",
                        {tr("act-finishing")}
                        " "
                        {
                            match request {
                                Request::Import(path, _) => path.to_string(),
                                Request::Export(export) => export.path.to_string(),
                            }
                        }
                    }
                }
                if let Some(error) = error {
                    p {
                        role: "alert",
                        class: "break-words text-sm text-ctp-red",
                        "{error}"
                    }
                }
            }
        }
    }
}
