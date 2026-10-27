use super::super::tr;
use super::*;
use crate::components::{Button, ButtonVariant, Input, Label};
use aftercast::{TableLevel, TableReportFormat};
use dioxus::prelude::*;
fn begin(mut state: Signal<AppState>, token: Token, render: bool) {
    let result = state.write().report_table_begin(token, render);
    match result {
        Ok(work) => super::view::execute(state, work),
        Err(key) => state.write().reports.error = Some(tr(key)),
    }
}
#[component]
pub(super) fn Editor() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let token = current.report_token();
    let ready = current.report_ready();
    let draft = current.reports.table.clone();
    let catalog = current.report_table_catalog().cloned();
    let blocked =
        current.reports.pending.is_some() || !matches!(current.phase, Phase::Idle | Phase::Ready);
    drop(current);
    rsx! {
        section {
            class: "space-y-4 border-t border-border pt-4",
            "aria-label": tr("table-report-title"),
            h3 { class: "text-sm font-semibold", {tr("table-report-title")} }
            p { class: "max-w-prose text-sm leading-7 text-muted-foreground",
                {tr("table-report-help")}
            }
            div { class: "flex flex-wrap gap-2",
                for (key, level, label) in [
                    ("combatant", TableLevel::Combatant, "table-report-combatant"),
                    ("category", TableLevel::Category, "table-report-category"),
                    ("attack", TableLevel::Attack, "table-report-attack"),
                    ("action", TableLevel::Action, "table-report-action"),
                ]
                {
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked,
                        aria_pressed: Some(draft.level == level),
                        onclick: move |_| {
                            state.write().report_table_edit(token, "level", key.into());
                        },
                        {tr(label)}
                    }
                }
                Button {
                    variant: ButtonVariant::Ghost,
                    disabled: !ready,
                    onclick: move |_| {
                        state.write().report_table_copy(token);
                    },
                    {tr("table-report-copy-scope")}
                }
            }
            if draft.level as u32 >= 2 {
                Field {
                    token,
                    field: "actor",
                    label: "table-report-actor",
                    value: draft.actor.clone(),
                    disabled: blocked,
                }
            }
            if draft.level as u32 >= 3 {
                Field {
                    token,
                    field: "category",
                    label: "table-report-category-key",
                    value: draft.category.clone(),
                    disabled: blocked,
                }
            }
            if draft.level as u32 >= 4 {
                Field {
                    token,
                    field: "attack",
                    label: "table-report-attack-key",
                    value: draft.attack.clone(),
                    disabled: blocked,
                }
            }
            Field {
                token,
                field: "name",
                label: "table-report-caption",
                value: draft.name.clone(),
                disabled: blocked,
            }
            div { class: "flex flex-wrap gap-2",
                for (key, format, label) in [
                    ("xml", TableReportFormat::TableXml, "table-report-xml"),
                    ("html", TableReportFormat::StaticHtml, "table-report-html"),
                ]
                {
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked,
                        aria_pressed: Some(draft.format == format),
                        onclick: move |_| {
                            state.write().report_table_edit(token, "format", key.into());
                        },
                        {tr(label)}
                    }
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: !ready,
                    onclick: move |_| begin(state, token, false),
                    {tr("table-report-load")}
                }
            }
            Field {
                token,
                field: "primary",
                label: "table-primary",
                value: draft.primary.clone(),
                disabled: blocked,
            }
            Field {
                token,
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
                        .report_table_edit(token, "descending", (!draft.descending).to_string());
                },
                {tr("sort-descending")}
            }
            p { class: "text-xs text-muted-foreground", {tr("table-report-sort-help")} }
            if let Some(catalog) = catalog {
                p { class: "break-words text-sm",
                    {tables::context(catalog.selection.scope())}
                    " · {catalog.rows.len()} "
                    {tr("table-report-rows")}
                }
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
                if draft.format == TableReportFormat::TableXml {
                    p { class: "text-xs text-muted-foreground", {tr("table-report-xml-help")} }
                    div { class: "flex flex-wrap gap-2",
                        for row in catalog.rows.iter().skip(draft.page * 16).take(16) {
                            {
                                let index = row.index;
                                let label = format!(
                                    "{} · {} / {} / {} · {}",
                                    index,
                                    row.combatant.as_deref().unwrap_or("—"),
                                    row.category.as_deref().unwrap_or("—"),
                                    row.attack.as_deref().unwrap_or("—"),
                                    row.time_sorter.map_or_else(|| "—".into(), |value| value.to_string()),
                                );
                                rsx! {
                                    Button {
                                        variant: ButtonVariant::Outline,
                                        class: "h-auto min-h-9 max-w-full break-all whitespace-normal py-2 text-left".to_owned(),
                                        disabled: blocked,
                                        aria_pressed: Some(draft.selected.contains(&index)),
                                        onclick: move |_| {
                                            state.write().report_table_edit(token, "row", index.to_string());
                                        },
                                        {label}
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
                                    .report_table_edit(token, "page", draft.page.saturating_sub(1).to_string());
                            },
                            {tr("previous")}
                        }
                        span { class: "text-xs text-muted-foreground",
                            "{draft.page+1} / {catalog.rows.len().div_ceil(16).max(1)} · {draft.selected.len()}"
                        }
                        Button {
                            variant: ButtonVariant::Ghost,
                            disabled: blocked || (draft.page + 1) * 16 >= catalog.rows.len(),
                            onclick: move |_| {
                                state.write().report_table_edit(token, "page", (draft.page + 1).to_string());
                            },
                            {tr("next")}
                        }
                    }
                } else {
                    p { class: "text-xs text-muted-foreground", {tr("table-report-html-help")} }
                    div { class: "flex flex-wrap gap-2",
                        for column in catalog.columns.iter() {
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
                                            state.write().report_table_edit(token, "column", key.clone());
                                        },
                                        "{column.label} [{column.key}]"
                                    }
                                }
                            }
                        }
                    }
                    for (index, key) in draft.columns.iter().enumerate() {
                        {
                            let up = key.clone();
                            let down = key.clone();
                            let remove = key.clone();
                            let missing = !catalog.columns.iter().any(|column| &column.key == key);
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
                                            state.write().report_table_edit(token, "up", up.clone());
                                        },
                                        {tr("table-up")}
                                    }
                                    Button {
                                        variant: ButtonVariant::Ghost,
                                        disabled: blocked || index + 1 == draft.columns.len(),
                                        aria_label: Some(format!("{} {}", tr("table-down"), key)),
                                        onclick: move |_| {
                                            state.write().report_table_edit(token, "down", down.clone());
                                        },
                                        {tr("table-down")}
                                    }
                                    Button {
                                        variant: ButtonVariant::Ghost,
                                        disabled: blocked,
                                        aria_label: Some(format!("{} {}", tr("table-report-remove"), key)),
                                        onclick: move |_| {
                                            state.write().report_table_edit(token, "column", remove.clone());
                                        },
                                        {tr("table-report-remove")}
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Button { disabled: !ready, onclick: move |_| begin(state, token, true),
                {tr("table-report-generate")}
            }
        }
    }
}
#[component]
fn Field(
    token: Token,
    field: &'static str,
    label: &'static str,
    value: String,
    disabled: bool,
) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let id = format!("table-report-{field}");
    rsx! {
        div { class: "space-y-2",
            Label { html_for: id.clone(), {tr(label)} }
            Input {
                id,
                value,
                disabled,
                oninput: move |event: FormEvent| {
                    state.write().report_table_edit(token, field, event.value());
                },
            }
        }
    }
}
