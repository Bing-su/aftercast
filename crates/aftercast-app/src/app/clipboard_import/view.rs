use super::super::tr;
use super::*;
use crate::components::{Accordion, Button, ButtonVariant, Progress};
use dioxus::{core::spawn_forever, prelude::*};

fn paste(mut state: Signal<AppState>, view: Token, adapter: Adapter) {
    let Some(token) = state.write().begin_clipboard_read(view) else {
        return;
    };
    // Only this explicit action requests provider text. Rendering and tab changes never read anything.
    spawn_forever(async move {
        let result = adapter.read().await;
        state.write().receive_clipboard_read(token, result);
    });
}
fn import(mut state: Signal<AppState>, view: Token) {
    let Some(host) = state.read().handle() else {
        return;
    };
    let Some((token, snapshot)) = state.write().begin_clipboard_import(view) else {
        return;
    };
    let complete = tr("clipboard-complete");
    let canceled = tr("import-cancelled");
    spawn_forever(async move {
        let result = match host.start_clipboard_import(snapshot.text.as_ref()).await {
            Ok(job) => super::super::wait_import(state, job, token.session).await,
            Err(error) => Err(error),
        };
        if state
            .write()
            .receive_clipboard_import(token, result, &complete, &canceled)
        {
            super::super::refresh(state, 0);
        }
    });
}
#[component]
pub(in crate::app) fn Controls() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let adapter = use_context::<Adapter>();
    let current = state.read();
    let view = current.clipboard_view();
    let draft = current.clipboard.draft.clone();
    let pending = current.clipboard.pending.clone();
    let error = current.clipboard.error;
    let editable = current.can_edit_clipboard();
    let can_import = current.can_import_clipboard();
    let canceling = current.cancel_import;
    let progress = current.progress;
    drop(current);
    rsx! {
        Accordion {
            default_open: true,
            class: "my-6 rounded-md border border-border".to_owned(),
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! {
                {tr("clipboard-title")}
            },
            section { class: "space-y-4 p-4", "aria-label": tr("clipboard-title"),
                p { class: "text-sm leading-7 text-muted-foreground", {tr("clipboard-help")} }
                if !adapter.0.available {
                    p {
                        role: "status",
                        class: "text-sm text-muted-foreground",
                        {tr("clipboard-unavailable")}
                    }
                }
                div { class: "flex flex-wrap gap-3",
                    Button {
                        id: "clipboard-paste",
                        disabled: !editable || adapter.busy() || !adapter.0.available,
                        onclick: move |_| paste(state, view, adapter.clone()),
                        {tr("clipboard-paste")}
                    }
                    Button {
                        id: "clipboard-clear",
                        variant: ButtonVariant::Outline,
                        disabled: !editable || draft.is_none(),
                        onclick: move |_| {
                            state.write().clear_clipboard_preview(view);
                        },
                        {tr("clipboard-clear")}
                    }
                }
                if matches!(pending, Some(Pending::Read(..))) {
                    p { role: "status", class: "text-sm", {tr("reading-clipboard")} }
                }
                if let Some(error) = error {
                    p { role: "alert", class: "text-sm text-ctp-red", {tr(error.key())} }
                }
                if let Some(snapshot) = draft {
                    p { class: "text-xs tabular-nums text-muted-foreground",
                        {tr("clipboard-snapshot")}
                        " · {snapshot.text.len()} B (UTF-8) · {snapshot.characters} "
                        {tr("clipboard-characters")}
                    }
                    pre {
                        id: "clipboard-preview",
                        class: "max-h-40 overflow-auto whitespace-pre-wrap break-words rounded-md bg-muted p-3 text-xs",
                        "aria-label": tr("clipboard-preview"),
                        "{snapshot.preview}"
                    }
                    if snapshot.characters > 512 {
                        p { class: "text-xs text-muted-foreground",
                            {tr("clipboard-preview-truncated")}
                        }
                    }
                    Button {
                        id: "clipboard-import",
                        disabled: !can_import,
                        onclick: move |_| import(state, view),
                        {tr("clipboard-import")}
                    }
                } else {
                    p { class: "text-sm text-muted-foreground", {tr("clipboard-no-snapshot")} }
                }
                if let Some(Pending::Import(token, accepted)) = pending {
                    p { role: "status", class: "text-sm tabular-nums",
                        {tr("clipboard-running")}
                        " · {accepted.text.len()} B (UTF-8) · {accepted.characters} "
                        {tr("clipboard-characters")}
                    }
                    Progress {
                        aria_label: tr("clipboard-running"),
                        max: progress.total_bytes,
                        value: progress.bytes_read,
                    }
                    p { class: "text-xs tabular-nums text-muted-foreground",
                        "{progress.bytes_read} / {progress.total_bytes} B (UTF-8) · {progress.records_dispatched} "
                        {tr("records")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: canceling,
                        onclick: move |_| {
                            state.write().cancel_clipboard_import(token);
                        },
                        {tr("cancel")}
                    }
                }
            }
        }
    }
}
