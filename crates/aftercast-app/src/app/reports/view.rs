use super::super::tr;
use super::*;
use crate::components::{Accordion, Button, ButtonVariant, Input, Label, TextArea};
use dioxus::{core::spawn_forever, prelude::*};
pub(super) fn execute(mut state: Signal<AppState>, work: Option<(Token, Request)>) {
    let Some((token, request)) = work else {
        return;
    };
    let Some(handle) = state.read().handle() else {
        return;
    };
    // Accepted tasks outlive panel navigation; their token decides whether a reply may be displayed.
    let saved = tr("report-saved");
    let canceled = tr("report-canceled");
    spawn_forever(async move {
        let result = perform(&handle, &request).await;
        let complete = if matches!(result, Ok(Reply::Save(Saved::Canceled))) {
            &canceled
        } else {
            &saved
        };
        state.write().report_receive(token, result, complete);
    });
}
fn begin(mut state: Signal<AppState>, token: Token, render: bool) {
    let result = state.write().report_begin(token, render);
    match result {
        Ok(work) => execute(state, work),
        Err(key) => state.write().reports.error = Some(tr(key)),
    }
}
#[component]
pub(in crate::app) fn Panel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let token = current.report_token();
    let ready = current.report_ready();
    let draft = current.reports.draft.clone();
    let catalog = current.reports.catalog.clone();
    let output = current.reports.output.clone();
    let preview = current.report_preview().cloned();
    let pending = current.reports.pending.clone();
    let confirmation = current.reports.confirmation.clone();
    let error = current.reports.error.clone();
    let page = current.reports.preset_page;
    let blocked = pending.is_some() || !matches!(current.phase, Phase::Idle | Phase::Ready);
    drop(current);
    rsx! {
        Accordion {
            class: "mt-6 rounded-md border border-border".to_owned(),
            default_open: false,
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! {
                {tr("report-title")}
            },
            section { class: "space-y-5 p-4", "aria-label": tr("report-title"),
                p { class: "max-w-prose text-sm leading-7 text-muted-foreground",
                    {tr("report-help")}
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: !ready,
                    onclick: move |_| begin(state, token, false),
                    {tr("report-load")}
                }
                if let Some(catalog) = catalog {
                    div { class: "space-y-3",
                        h3 { class: "text-sm font-medium", {tr("report-presets")} }
                        div { class: "flex flex-wrap gap-2",
                            Button {
                                variant: ButtonVariant::Outline,
                                disabled: !ready,
                                onclick: move |_| {
                                    state.write().report_preset(token, None);
                                },
                                {tr("report-default")}
                            }
                            for preset in catalog.presets.iter().skip(page * 16).take(16) {
                                {
                                    let index = preset.index as usize;
                                    let label = format!("{} · {}", index + 1, preset.options.sorting);
                                    rsx! {
                                        Button {
                                            variant: ButtonVariant::Outline,
                                            class: "h-auto min-h-9 max-w-full break-all whitespace-normal py-2 text-left".to_owned(),
                                            disabled: !ready,
                                            onclick: move |_| {
                                                state.write().report_preset(token, Some(index));
                                            },
                                            {label}
                                        }
                                    }
                                }
                            }
                        }
                        if catalog.presets.len() > 16 {
                            div { class: "flex gap-2",
                                Button {
                                    variant: ButtonVariant::Ghost,
                                    disabled: !ready || page == 0,
                                    onclick: move |_| {
                                        if state.read().report_token() == token {
                                            state.write().reports.preset_page = page.saturating_sub(1);
                                        }
                                    },
                                    {tr("previous")}
                                }
                                Button {
                                    variant: ButtonVariant::Ghost,
                                    disabled: !ready || (page + 1) * 16 >= catalog.presets.len(),
                                    onclick: move |_| {
                                        if state.read().report_token() == token {
                                            state.write().reports.preset_page = page + 1;
                                        }
                                    },
                                    {tr("next")}
                                }
                            }
                        }
                        p { class: "break-all text-xs text-muted-foreground",
                            {tr("report-variables")}
                            ": "
                            {catalog.player_variables.join(", ")}
                        }
                        p { class: "break-all text-xs text-muted-foreground",
                            {tr("report-group-variables")}
                            ": "
                            {catalog.allied_variables.join(", ")}
                        }
                        p { class: "break-all text-xs text-muted-foreground",
                            {tr("report-sort-keys")}
                            ": "
                            {catalog.sort_columns.join(", ")}
                        }
                    }
                }
                div { class: "space-y-2",
                    Label { html_for: "report-player", {tr("report-player")} }
                    TextArea {
                        id: "report-player".to_owned(),
                        value: draft.player_format,
                        disabled: blocked,
                        oninput: move |event: FormEvent| {
                            state.write().report_edit(token, "player", event.value());
                        },
                    }
                    Label { html_for: "report-allied", {tr("report-allied")} }
                    TextArea {
                        id: "report-allied".to_owned(),
                        rows: 2,
                        value: draft.allies_format,
                        disabled: blocked,
                        oninput: move |event: FormEvent| {
                            state.write().report_edit(token, "allied", event.value());
                        },
                    }
                    Label { html_for: "report-sort", {tr("report-sort")} }
                    Input {
                        id: "report-sort".to_owned(),
                        value: draft.sorting,
                        disabled: blocked,
                        oninput: move |event: FormEvent| {
                            state.write().report_edit(token, "sort", event.value());
                        },
                    }
                    div { class: "flex flex-wrap gap-2",
                        Button {
                            variant: ButtonVariant::Outline,
                            aria_pressed: Some(draft.show_only_allies),
                            disabled: blocked,
                            onclick: move |_| {
                                state.write().report_edit(token, "only", (!draft.show_only_allies).to_string());
                            },
                            {tr("report-only-allies")}
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            aria_pressed: Some(draft.show_allied_info),
                            disabled: blocked,
                            onclick: move |_| {
                                state.write().report_edit(token, "group", (!draft.show_allied_info).to_string());
                            },
                            {tr("report-include-group")}
                        }
                    }
                    Button {
                        disabled: !ready,
                        onclick: move |_| begin(state, token, true),
                        {tr("report-generate")}
                    }
                }
                super::table_view::Editor {}
                if let Some((active, _)) = pending {
                    div { class: "flex items-center gap-3", role: "status",
                        p { class: "text-sm text-muted-foreground", {tr("report-working")} }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: active != token,
                            onclick: move |_| {
                                state.write().report_cancel(token);
                            },
                            {tr("cancel")}
                        }
                    }
                }
                if let Some(error) = error {
                    p {
                        class: "break-words text-sm text-destructive",
                        role: "alert",
                        {if error == "report-unavailable" { tr(&error) } else { error }}
                    }
                }
                if let Some(preview) = preview {
                    div { class: "space-y-3 border-t border-border pt-4",
                        h3 { class: "text-sm font-semibold", {tr("report-preview")} }
                        p { class: "text-sm", "{preview.title}" }
                        p { class: "text-xs text-muted-foreground",
                            {tr("report-snapshot")}
                            ": {preview.report.description()} · {preview.report.text().len()} UTF-8 bytes"
                        }
                        if let Content::Table(snapshot, options) = preview.report.as_ref() {
                            p { class: "break-words text-xs text-muted-foreground",
                                "{snapshot.rows} "
                                {tr("table-report-rows")}
                                " · XML: {options.selected_rows.len()} · HTML: "
                                {
                                    options
                                        .columns
                                        .as_ref()
                                        .map(|keys| keys.join(", "))
                                        .unwrap_or_else(|| "—".into())
                                }
                            }
                        }
                        pre { class: "max-h-72 overflow-auto whitespace-pre-wrap break-words rounded-md bg-background p-3 text-sm",
                            {preview.report.preview_text()}
                        }
                        if preview.report.preview_text().len() < preview.report.text().len() {
                            p { class: "text-xs text-muted-foreground",
                                {tr("report-preview-truncated")}
                            }
                        }
                        Label { html_for: "report-output", {tr("report-output")} }
                        Input {
                            id: "report-output".to_owned(),
                            value: output,
                            disabled: blocked,
                            oninput: move |event: FormEvent| {
                                state.write().report_edit(token, "path", event.value());
                            },
                        }
                        p { class: "max-w-prose text-xs text-muted-foreground",
                            {tr("report-save-help")}
                        }
                        Button {
                            disabled: !ready,
                            onclick: move |_| {
                                let result = state.write().report_review_save(token);
                                if let Err(key) = result {
                                    state.write().reports.error = Some(tr(key));
                                }
                            },
                            {tr("report-review-save")}
                        }
                    }
                }
                if let Some((review, preview, path)) = confirmation {
                    div { class: "space-y-3 border-t border-border pt-4",
                        h3 { class: "text-sm font-semibold", {tr("report-confirm-title")} }
                        p { class: "text-sm", "{preview.title}" }
                        p { class: "break-words text-xs text-muted-foreground",
                            "{preview.report.description()} · {preview.report.text().len()} UTF-8 bytes"
                        }
                        p { class: "break-all text-sm", "{path}" }
                        div { class: "flex gap-2",
                            Button {
                                disabled: !ready,
                                onclick: move |_| {
                                    let work = state.write().report_confirm_save(review);
                                    execute(state, work);
                                },
                                {tr("report-save")}
                            }
                            Button {
                                variant: ButtonVariant::Outline,
                                disabled: !ready,
                                onclick: move |_| {
                                    state.write().report_cancel_review(review);
                                },
                                {tr("cancel")}
                            }
                        }
                    }
                }
            }
        }
    }
}
