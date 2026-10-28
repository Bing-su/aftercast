use super::super::tr;
use super::*;
use crate::components::{Button, ButtonVariant, Input, Label};
use aftercast::{EncounterSummary, HierarchyHtmlLevel, HierarchyHtmlReport, TableLevel};
use dioxus::prelude::*;
fn begin(mut state: Signal<AppState>, token: Token, render: bool) {
    let result = state.write().report_html_begin(token, render);
    match result {
        Ok(work) => super::view::execute(state, work),
        Err(key) => state.write().reports.error = Some(tr(key)),
    }
}
fn toggle(mut state: Signal<AppState>, token: Token, id: EncounterId) {
    let result = state.write().report_html_toggle(token, id);
    if let Err(key) = result {
        state.write().reports.error = Some(tr(key));
    }
}
fn name(row: &EncounterSummary) -> String {
    format!(
        "{} / {} · {}",
        row.zone,
        row.title,
        super::super::encounter_context::format_time(row.start_time)
    )
}
#[component]
pub(super) fn Editor() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let token = current.report_token();
    let ready = current.report_ready();
    let blocked = !current.report_html_editable(token);
    let draft = current.reports.html.clone();
    let rows = current.encounters.clone();
    let selected = if draft.owner == Some(current.revision.session) {
        draft.selected.clone()
    } else {
        vec![]
    };
    let page = draft.page.min(rows.len().div_ceil(16).saturating_sub(1));
    drop(current);
    rsx! {
        section {
            class: "space-y-4 border-t border-border pt-4",
            "aria-label": tr("html-report-title"),
            h3 { class: "text-sm font-semibold", {tr("html-report-title")} }
            p { class: "max-w-prose text-sm leading-7 text-muted-foreground",
                {tr("html-report-help")}
            }
            h4 { class: "text-sm font-medium", {tr("html-report-select")} }
            p { class: "text-xs text-muted-foreground", {tr("html-report-select-help")} }
            div { class: "flex flex-wrap gap-2",
                for row in rows.iter().skip(page * 16).take(16) {
                    {
                        let id = row.id;
                        let chosen = selected.iter().any(|entry| entry.id == id);
                        rsx! {
                            Button {
                                variant: ButtonVariant::Outline,
                                class: "h-auto min-h-9 max-w-full break-all whitespace-normal py-2 text-left".to_owned(),
                                disabled: !ready,
                                aria_pressed: Some(chosen),
                                onclick: move |_| toggle(state, token, id),
                                {name(row)}
                            }
                        }
                    }
                }
            }
            div { class: "flex items-center gap-2",
                Button {
                    variant: ButtonVariant::Ghost,
                    disabled: blocked || page == 0,
                    onclick: move |_| {
                        state.write().report_html_page(token, page.saturating_sub(1));
                    },
                    {tr("previous")}
                }
                span { class: "text-xs text-muted-foreground",
                    "{page+1} / {rows.len().div_ceil(16).max(1)} · {selected.len()}/64"
                }
                Button {
                    variant: ButtonVariant::Ghost,
                    disabled: blocked || (page + 1) * 16 >= rows.len(),
                    onclick: move |_| {
                        state.write().report_html_page(token, page + 1);
                    },
                    {tr("next")}
                }
            }
            for row in &selected {
                {
                    let id = row.id;
                    rsx! {
                        div { class: "flex flex-wrap items-center gap-2 text-xs",
                            span { class: "min-w-0 break-all", {name(row)} }
                            Button {
                                variant: ButtonVariant::Ghost,
                                disabled: !ready,
                                aria_label: Some(format!("{} {}", tr("html-report-remove"), name(row))),
                                onclick: move |_| toggle(state, token, id),
                                {tr("html-report-remove")}
                            }
                        }
                    }
                }
            }
            div { class: "flex flex-wrap gap-2",
                for level in html::LEVELS {
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked,
                        aria_pressed: Some(draft.last == level),
                        onclick: move |_| {
                            state.write().report_html_depth(token, level);
                        },
                        {tr(html::label(level))}
                    }
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: !ready,
                    onclick: move |_| begin(state, token, false),
                    {tr("html-report-load")}
                }
            }
            for level in html::LEVELS.into_iter().take(draft.last as usize) {
                LevelEditor { level }
            }
            Button { disabled: !ready, onclick: move |_| begin(state, token, true),
                {tr("report-generate")}
            }
        }
    }
}
#[component]
fn LevelEditor(level: TableLevel) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let token = current.report_token();
    let blocked = !current.report_html_editable(token);
    let draft = current
        .reports
        .html
        .levels
        .iter()
        .zip(html::LEVELS)
        .find(|(_, entry)| *entry == level)
        .map(|(draft, _)| draft.clone())
        .unwrap_or_default();
    let catalog = current
        .reports
        .html
        .catalogs
        .iter()
        .find(|catalog| catalog.level == level)
        .cloned();
    drop(current);
    rsx! {
        section {
            class: "space-y-3 rounded-md border border-border p-3",
            "aria-label": tr(html::label(level)),
            h4 { class: "text-sm font-medium", {tr(html::label(level))} }
            if level != TableLevel::Category {
                Field {
                    token,
                    level,
                    field: "primary",
                    label: "table-primary",
                    value: draft.primary.clone(),
                    disabled: blocked,
                }
                Field {
                    token,
                    level,
                    field: "secondary",
                    label: "table-secondary",
                    value: draft.secondary.clone(),
                    disabled: blocked,
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: blocked,
                    aria_pressed: Some(draft.descending),
                    onclick: move |_| {
                        state
                            .write()
                            .report_html_edit(
                                token,
                                level,
                                "descending",
                                (!draft.descending).to_string(),
                            );
                    },
                    {tr("sort-descending")}
                }
            } else {
                p { class: "text-xs text-muted-foreground", {tr("html-report-category-order")} }
            }
            if let Some(catalog) = catalog {
                p { class: "break-all text-xs text-muted-foreground",
                    {tr("report-sort-keys")}
                    ": "
                    {
                        catalog
                            .columns
                            .iter()
                            .filter(|column| column.can_sort)
                            .map(|column| column.key.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    }
                }
                div { class: "flex flex-wrap gap-2",
                    for column in catalog.columns.iter().skip(draft.page * 16).take(16) {
                        {
                            let key = column.key.clone();
                            let chosen = draft.columns.contains(&key);
                            rsx! {
                                Button {
                                    variant: ButtonVariant::Outline,
                                    class: "h-auto min-h-9 max-w-full break-all whitespace-normal py-2 text-left".to_owned(),
                                    disabled: blocked || (!chosen && !column.can_read),
                                    aria_pressed: Some(chosen),
                                    onclick: move |_| {
                                        state.write().report_html_edit(token, level, "column", key.clone());
                                    },
                                    "{column.label} [{column.key}]"
                                }
                            }
                        }
                    }
                }
                div { class: "flex items-center gap-2",
                    Button {
                        variant: ButtonVariant::Ghost,
                        disabled: blocked || draft.page == 0,
                        onclick: move |_| {
                            state
                                .write()
                                .report_html_edit(
                                    token,
                                    level,
                                    "page",
                                    draft.page.saturating_sub(1).to_string(),
                                );
                        },
                        {tr("previous")}
                    }
                    span { class: "text-xs text-muted-foreground",
                        "{draft.page+1} / {catalog.columns.len().div_ceil(16).max(1)}"
                    }
                    Button {
                        variant: ButtonVariant::Ghost,
                        disabled: blocked || (draft.page + 1) * 16 >= catalog.columns.len(),
                        onclick: move |_| {
                            state
                                .write()
                                .report_html_edit(token, level, "page", (draft.page + 1).to_string());
                        },
                        {tr("next")}
                    }
                }
                for (index, key) in draft.columns.iter().enumerate() {
                    {
                        let up = key.clone();
                        let down = key.clone();
                        let remove = key.clone();
                        let missing = !catalog
                            .columns
                            .iter()
                            .any(|column| column.key == *key && column.can_read);
                        rsx! {
                            div { class: "flex flex-wrap items-center gap-2 text-xs",
                                span { class: "min-w-0 break-all", "{index+1}. {key}" }
                                if missing {
                                    span { class: "text-destructive", {tr("table-report-missing-key")} }
                                }
                                Button {
                                    variant: ButtonVariant::Ghost,
                                    disabled: blocked || index == 0,
                                    aria_label: Some(format!("{} {}", tr("table-up"), key)),
                                    onclick: move |_| {
                                        state.write().report_html_edit(token, level, "up", up.clone());
                                    },
                                    {tr("table-up")}
                                }
                                Button {
                                    variant: ButtonVariant::Ghost,
                                    disabled: blocked || index + 1 == draft.columns.len(),
                                    aria_label: Some(format!("{} {}", tr("table-down"), key)),
                                    onclick: move |_| {
                                        state.write().report_html_edit(token, level, "down", down.clone());
                                    },
                                    {tr("table-down")}
                                }
                                Button {
                                    variant: ButtonVariant::Ghost,
                                    disabled: blocked,
                                    aria_label: Some(format!("{} {}", tr("table-report-remove"), key)),
                                    onclick: move |_| {
                                        state.write().report_html_edit(token, level, "column", remove.clone());
                                    },
                                    {tr("table-report-remove")}
                                }
                            }
                        }
                    }
                }
            } else {
                p { class: "text-xs text-muted-foreground", {tr("html-report-load-first")} }
            }
        }
    }
}
#[component]
fn Field(
    token: Token,
    level: TableLevel,
    field: &'static str,
    label: &'static str,
    value: String,
    disabled: bool,
) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let id = format!("hierarchy-html-{}-{field}", level as u32);
    rsx! {
        div { class: "space-y-2",
            Label { html_for: id.clone(), {tr(label)} }
            Input {
                id,
                value,
                disabled,
                oninput: move |event: FormEvent| {
                    state.write().report_html_edit(token, level, field, event.value());
                },
            }
        }
    }
}
pub(super) fn snapshot(
    report: &HierarchyHtmlReport,
    levels: &[HierarchyHtmlLevel],
    selected: &[EncounterSummary],
) -> Element {
    rsx! {
        div { class: "space-y-2 text-xs text-muted-foreground",
            p { {tr("html-report-snapshot")} }
            for (index, row) in selected.iter().enumerate() {
                p { class: "break-words",
                    "{index+1}. "
                    {name(row)}
                }
            }
            for level in levels {
                p { class: "break-all",
                    {tr(html::label(level.level))}
                    ": "
                    {level.columns.join(", ")}
                    " · "
                    {
                        level
                            .sort
                            .as_ref()
                            .map_or_else(
                                || "—".into(),
                                |sort| {
                                    format!(
                                        "{} / {} · {}",
                                        sort.primary,
                                        sort.secondary.as_deref().unwrap_or("—"),
                                        if sort.descending { "↓" } else { "↑" },
                                    )
                                },
                            )
                    }
                }
            }
            p {
                "{report.selection.table_count()} "
                {tr("html-report-counts")}
                " · {report.selection.row_count()} "
                {tr("table-report-rows")}
            }
        }
    }
}
