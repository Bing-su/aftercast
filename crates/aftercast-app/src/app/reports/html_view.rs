use super::super::tr;
use super::*;
use crate::components::{Button, ButtonVariant, Input, Label};
use aftercast::{
    EncounterSummary, HierarchyHtmlGraphOptions, HierarchyHtmlLevel, HierarchyHtmlReport,
    TableLevel,
};
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
            GraphEditor {}
            Button { disabled: !ready, onclick: move |_| begin(state, token, true),
                {tr("report-generate")}
            }
        }
    }
}
#[component]
fn GraphEditor() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let token = current.report_token();
    let disabled = !current.report_html_editable(token);
    let draft = current.reports.html.clone();
    drop(current);
    rsx! {
        section { class: "space-y-3 rounded-md border border-border p-3",
            h4 { class: "text-sm font-medium", {tr("html-graph-title")} }
            p { class: "text-xs text-muted-foreground", {tr("html-graph-help")} }
            div { class: "flex flex-wrap gap-2",
                Button {
                    variant: ButtonVariant::Outline,
                    disabled,
                    aria_pressed: Some(!draft.graph_levels.iter().any(|selected| *selected)),
                    onclick: move |_| {
                        state.write().report_html_graph_edit(token, "clear", String::new());
                    },
                    {tr("html-graph-none")}
                }
                for (index, (level, selected)) in html::LEVELS.into_iter().zip(draft.graph_levels).enumerate() {
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled,
                        aria_pressed: Some(selected),
                        onclick: move |_| {
                            state.write().report_html_graph_edit(token, "toggle", index.to_string());
                        },
                        {tr(html::label(level))}
                    }
                }
            }
            if draft.graph_levels.iter().any(|selected| *selected) {
                if draft.graph_levels[0] {
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled,
                        aria_pressed: Some(draft.graph_simple),
                        onclick: move |_| {
                            state
                                .write()
                                .report_html_graph_edit(token, "simple", (!draft.graph_simple).to_string());
                        },
                        {
                            if draft.graph_simple {
                                tr("html-graph-simple")
                            } else {
                                tr("html-graph-advanced")
                            }
                        }
                    }
                    if draft.graph_simple {
                        GraphField {
                            token,
                            disabled,
                            field: "simple_column",
                            label: "html-graph-simple-column",
                            value: draft.graph_simple_column.clone(),
                        }
                        div { class: "flex flex-wrap gap-2",
                            for column in draft
                                .catalogs
                                .iter()
                                .filter(|catalog| catalog.level == TableLevel::Combatant)
                                .flat_map(|catalog| catalog.columns.iter().filter(|column| column.can_sort))
                            {
                                {
                                    let key = column.key.clone();
                                    let display = key.clone();
                                    rsx! {
                                        Button {
                                            variant: ButtonVariant::Outline,
                                            disabled,
                                            onclick: move |_| {
                                                state.write().report_html_graph_edit(token, "simple_column", key.clone());
                                            },
                                            "{display}"
                                        }
                                    }
                                }
                            }
                        }
                        p { class: "text-xs text-muted-foreground", {tr("html-graph-sql-help")} }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled,
                            aria_pressed: Some(draft.graph_only_allies),
                            onclick: move |_| {
                                state
                                    .write()
                                    .report_html_graph_edit(
                                        token,
                                        "only_allies",
                                        (!draft.graph_only_allies).to_string(),
                                    );
                            },
                            {tr("html-graph-allies")}
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled,
                            aria_pressed: Some(draft.graph_totals),
                            onclick: move |_| {
                                state
                                    .write()
                                    .report_html_graph_edit(token, "totals", (!draft.graph_totals).to_string());
                            },
                            {tr("html-graph-totals")}
                        }
                    }
                }
                if draft.graph_levels[0] || draft.graph_levels[1] {
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled,
                        onclick: move |_| {
                            let result = state.write().report_html_graph_candidates_begin(token);
                            match result {
                                Ok(work) => super::view::execute(state, work),
                                Err(key) => state.write().reports.error = Some(tr(key)),
                            }
                        },
                        {tr("html-graph-load-candidates")}
                    }
                    if let Some(candidates) = draft.graph_candidates.as_ref() {
                        if draft.graph_levels[0] {
                            GraphScope {
                                token,
                                disabled,
                                field: "actors",
                                label: "html-graph-actors",
                                available: candidates.actors.clone(),
                                selected: draft.graph_actors.clone(),
                            }
                        }
                        if draft.graph_levels[1] {
                            GraphScope {
                                token,
                                disabled,
                                field: "categories",
                                label: "html-graph-categories",
                                available: candidates.categories.clone(),
                                selected: draft.graph_categories.clone(),
                            }
                        }
                    } else {
                        p { class: "text-xs text-muted-foreground",
                            {tr("html-graph-candidates-help")}
                        }
                    }
                }
                GraphField {
                    token,
                    disabled,
                    field: "window",
                    label: "html-graph-window",
                    value: draft.graph_window.clone(),
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled,
                    aria_pressed: Some(draft.graph_rolling),
                    onclick: move |_| {
                        state
                            .write()
                            .report_html_graph_edit(
                                token,
                                "rolling",
                                (!draft.graph_rolling).to_string(),
                            );
                    },
                    {tr("html-graph-rolling")}
                }
                if draft.graph_levels[2] {
                    GraphField {
                        token,
                        disabled,
                        field: "distribution",
                        label: "html-graph-distribution",
                        value: draft.graph_distribution.clone(),
                    }
                    div { class: "flex flex-wrap gap-2",
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled,
                            onclick: move |_| {
                                state.write().report_html_graph_edit(token, "distribution", "Resist".into());
                            },
                            "Resist"
                        }
                        for column in draft
                            .catalogs
                            .iter()
                            .filter(|catalog| catalog.level == TableLevel::Attack)
                            .flat_map(|catalog| catalog.columns.iter().filter(|column| column.can_sort))
                        {
                            {
                                let key = column.key.clone();
                                let display = key.clone();
                                rsx! {
                                    Button {
                                        variant: ButtonVariant::Outline,
                                        disabled,
                                        onclick: move |_| {
                                            state.write().report_html_graph_edit(token, "distribution", key.clone());
                                        },
                                        "{display}"
                                    }
                                }
                            }
                        }
                    }
                    p { class: "text-xs text-muted-foreground", {tr("html-graph-sql-help")} }
                }
                if draft.graph_levels[3] {
                    GraphField {
                        token,
                        disabled,
                        field: "action_sort",
                        label: "html-graph-action",
                        value: draft.graph_action_sort.clone(),
                    }
                    div { class: "flex flex-wrap gap-2",
                        for column in draft
                            .catalogs
                            .iter()
                            .filter(|catalog| catalog.level == TableLevel::Action)
                            .flat_map(|catalog| catalog.columns.iter().filter(|column| column.can_sort))
                        {
                            {
                                let key = column.key.clone();
                                let display = key.clone();
                                rsx! {
                                    Button {
                                        variant: ButtonVariant::Outline,
                                        disabled,
                                        onclick: move |_| {
                                            state.write().report_html_graph_edit(token, "action_sort", key.clone());
                                        },
                                        "{display}"
                                    }
                                }
                            }
                        }
                    }
                }
                GraphField {
                    token,
                    disabled,
                    field: "all",
                    label: "html-graph-all",
                    value: draft.graph_all.clone(),
                }
                if draft.graph_levels[2] {
                    GraphField {
                        token,
                        disabled,
                        field: "ward",
                        label: "html-graph-ward",
                        value: draft.graph_ward.clone(),
                    }
                }
            }
        }
    }
}
#[component]
fn GraphScope(
    token: Token,
    disabled: bool,
    field: &'static str,
    label: &'static str,
    available: Vec<String>,
    selected: Option<Vec<String>>,
) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let mode = if field == "actors" {
        "actors_mode"
    } else {
        "categories_mode"
    };
    let mut shown = available.clone();
    if let Some(values) = &selected {
        for value in values {
            if !shown.contains(value) {
                shown.push(value.clone());
            }
        }
    }
    rsx! {
        div { class: "space-y-2",
            h5 { class: "text-sm font-medium", {tr(label)} }
            div { class: "flex flex-wrap gap-2",
                Button {
                    variant: ButtonVariant::Outline,
                    disabled,
                    aria_pressed: Some(selected.is_none()),
                    onclick: move |_| {
                        state.write().report_html_graph_scope(token, mode, "all");
                    },
                    {tr("html-graph-all-items")}
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled,
                    aria_pressed: Some(selected.is_some()),
                    onclick: move |_| {
                        state.write().report_html_graph_scope(token, mode, "custom");
                    },
                    {tr("html-graph-choose")}
                }
            }
            if let Some(chosen) = selected {
                div { class: "flex flex-wrap gap-2",
                    for value in shown {
                        {
                            let key = value.clone();
                            let pressed = chosen.contains(&value);
                            let missing = !available.contains(&value);
                            rsx! {
                                Button {
                                    variant: ButtonVariant::Outline,
                                    disabled,
                                    aria_pressed: Some(pressed),
                                    onclick: move |_| {
                                        state.write().report_html_graph_scope(token, field, &key);
                                    },
                                    "{value}"
                                    if missing {
                                        " · "
                                        {tr("html-graph-missing")}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
#[component]
fn GraphField(
    token: Token,
    disabled: bool,
    field: &'static str,
    label: &'static str,
    value: String,
) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let id = format!("hierarchy-graph-{field}");
    rsx! {
        div { class: "space-y-2",
            Label { html_for: id.clone(), {tr(label)} }
            Input {
                id,
                value,
                disabled,
                oninput: move |event: FormEvent| {
                    state.write().report_html_graph_edit(token, field, event.value());
                },
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
    graphs: Option<&HierarchyHtmlGraphOptions>,
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
            if let Some(graph) = graphs {
                {graph_snapshot(graph)}
            }
            if graphs.is_none() {
                p {
                    "{report.selection.table_count()} "
                    {tr("html-report-counts")}
                    " · {report.selection.row_count()} "
                    {tr("table-report-rows")}
                }
            }
        }
    }
}

fn graph_snapshot(graph: &HierarchyHtmlGraphOptions) -> Element {
    rsx! {
        if graph.simple && graph.levels.contains(&TableLevel::Combatant) {
            p {
                {tr("html-graph-simple")}
                ": {graph.simple_column} · "
                {tr("html-graph-allies")}
                ": {graph.only_allies} · "
                {tr("html-graph-totals")}
                ": {graph.totals}"
            }
        }
        if graph.levels.contains(&TableLevel::Combatant) {
            if !graph.simple {
                p { {tr("html-graph-advanced")} }
            }
            p {
                {tr("html-graph-actors")}
                ": "
                {
                    graph
                        .selected_combatants
                        .as_ref()
                        .map_or_else(
                            || tr("html-graph-all-items"),
                            |values| {
                                if values.is_empty() {
                                    tr("html-graph-no-items")
                                } else {
                                    values.join(", ")
                                }
                            },
                        )
                }
            }
        }
        if graph.levels.contains(&TableLevel::Category) {
            p {
                {tr("html-graph-categories")}
                ": "
                {
                    graph
                        .solo_categories
                        .as_ref()
                        .map_or_else(
                            || tr("html-graph-all-items"),
                            |values| {
                                if values.is_empty() {
                                    tr("html-graph-no-items")
                                } else {
                                    values.join(", ")
                                }
                            },
                        )
                }
            }
        }
        p {
            {tr("html-graph-title")}
            ": "
            {
                graph
                    .levels
                    .iter()
                    .map(|level| tr(html::label(*level)))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
            if graph.levels.contains(&TableLevel::Category)
                || (graph.levels.contains(&TableLevel::Combatant) && !graph.simple)
            {
                " · {graph.window}s · "
                {if graph.rolling { tr("html-graph-rolling") } else { tr("html-graph-bucket") }}
            }
            " · {graph.all_label}"
            if graph.levels.contains(&TableLevel::Attack) {
                " · {graph.distribution_column} · {graph.ward_prefix}"
            }
            if graph.levels.contains(&TableLevel::Action) {
                " · {graph.action_sort_column}"
            }
        }
    }
}
