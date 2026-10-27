use super::{AppState, Phase, import_log, tr};
use crate::components::{Accordion, Button, ButtonVariant, Input, Label, Progress};
use chorale_core::{Alignment, CellValue, ColumnDef, ColumnId, Labels, RowId, TableState, Theme};
use chorale_dioxus::{Table, use_table};
use dioxus::prelude::*;

#[component]
pub(super) fn AnalysisPane() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let detail = state.read().detail.clone();
    rsx! {
        section { class: "p-6 min-[1000px]:px-8 min-[1000px]:py-7",
            div { class: "mb-7 flex items-start justify-between gap-5 [&_h1]:text-[28px] [&_h1]:font-semibold [&_h1]:tracking-tight [&_p]:mt-2 [&_p]:max-w-[68ch] [&_p]:text-sm [&_p]:leading-7 [&_p]:text-muted-foreground",
                div {
                    if let Some(detail) = &detail {
                        p { class: "!mt-0 !mb-2", "{detail.summary.zone} / {detail.summary.title}" }
                    }
                    h1 {
                        {
                            state
                                .read()
                                .category
                                .as_ref()
                                .map(|(name, _)| name.clone())
                                .or_else(|| state.read().combatant.clone())
                                .unwrap_or_else(|| tr("analysis"))
                        }
                    }
                    if detail.is_none() {
                        p { {tr("analysis-description")} }
                    }
                }
            }
            if let Some(detail) = detail {
                div { class: "summary-strip mb-6 flex flex-wrap gap-x-9 gap-y-3 border-y border-border py-4 tabular-nums [&_span]:text-xs [&_span]:text-muted-foreground [&_strong]:ml-3 [&_strong]:text-sm [&_strong]:font-medium",
                    div {
                        span { {tr("damage")} }
                        strong { "{detail.summary.damage}" }
                    }
                    div {
                        span { {tr("healing")} }
                        strong { "{detail.summary.healed}" }
                    }
                    div {
                        span { {tr("participants")} }
                        strong { "{detail.summary.combatants}" }
                    }
                    div {
                        span { {tr("duration")} }
                        strong { "{detail.summary.duration_ticks / 10_000_000} s" }
                    }
                }
                if let Some(combatant) = state.read().combatant.clone() {
                    AttackTable {
                        key: "{state.read().revision.selection}-{combatant}-{state.read().category:?}-{state.read().settings.ui.language}-{state.read().settings.ui.theme}",
                        rows: attack_rows(&state.read().attacks, &combatant, state.read().category.as_ref()),
                    }
                } else {
                    ParticipantTable {
                        key: "{state.read().revision.session}-{state.read().revision.selection}-{state.read().settings.ui.language}-{state.read().settings.ui.theme}",
                        rows: detail
                            .combatants
                            .iter()
                            .map(|row| Participant {
                                name: row.name.clone(),
                                damage: row.damage,
                                healing: row.healed,
                                dps: row.enc_dps,
                                deaths: row.deaths,
                            })
                            .collect(),
                    }
                }
                Accordion {
                    class: "contribution-panel mt-5 border border-border rounded-md".to_owned(),
                    default_open: true,
                    trigger_class: "px-4 py-3 font-semibold".to_owned(),
                    header: rsx! {
                        {tr("chart-description")}
                    },
                    ContributionChart {
                        rows: detail
                            .combatants
                            .iter()
                            .filter(|row| row.damage > 0)
                            .map(|row| (row.name.clone(), row.damage))
                            .collect(),
                    }
                }
                for id in [detail.summary.id] {
                    super::encounter_context::EncounterContext {
                        key: "{state.read().revision.session}-{state.read().revision.selection}-{state.read().settings.ui.language}-{state.read().settings.ui.theme}",
                        id,
                        title: format!("{} / {}", detail.summary.zone, detail.summary.title),
                    }
                }
                p { class: "my-6 max-w-[70ch] text-xs leading-7 text-muted-foreground",
                    {tr("aggregate-scope")}
                }
            } else if state.read().detail_loading {
                p {
                    role: "status",
                    class: "py-12 text-sm text-muted-foreground",
                    {tr("loading-selection")}
                }
            } else {
                div { class: "flex min-h-100 flex-col items-center justify-center px-5 py-10 text-center [&_h2]:text-xl [&_h2]:font-medium [&_p]:my-4 [&_p]:max-w-[48ch] [&_p]:leading-8 [&_p]:text-muted-foreground",
                    icons::ChartNoAxesCombined { class: "mb-5 size-11 text-muted-foreground" }
                    h2 { {tr("analysis-empty-title")} }
                    p { {tr("analysis-empty-description")} }
                    Button {
                        variant: ButtonVariant::Outline,
                        onclick: move |_| state.write().screen = "plugins",
                        {tr("configure-plugins")}
                    }
                }
            }
        }
    }
}

#[component]
pub(super) fn ImportPane() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    rsx! {
        section { class: "max-w-5xl p-8",
            h1 { class: "text-2xl font-semibold", {tr("import")} }
            p { class: "mt-2 mb-8 text-sm leading-7 text-muted-foreground",
                {tr("import-description")}
            }
            Label { html_for: "log-path", class: "mb-2".to_owned(), {tr("log-path")} }
            Input {
                id: "log-path".to_owned(),
                value: state.read().import_path.clone(),
                placeholder: tr("log-path-hint"),
                disabled: state.read().phase == Phase::Importing,
                oninput: move |event: FormEvent| state.write().import_path = event.value(),
            }
            div { class: "my-5 flex gap-3",
                Button {
                    disabled: state.read().phase != Phase::Ready || state.read().plugins.is_empty(),
                    onclick: move |_| import_log(state),
                    icons::FileUp {}
                    {tr("import-log")}
                }
                if state.read().phase == Phase::Importing {
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: state.read().cancel_import,
                        onclick: move |_| state.write().cancel_import = true,
                        {tr("cancel")}
                    }
                }
            }
            if state.read().phase == Phase::Importing {
                div {
                    role: "status",
                    class: "space-y-3 py-4 text-sm text-muted-foreground",
                    p { {tr("importing")} }
                    Progress {
                        aria_label: tr("importing"),
                        max: state.read().progress.total_bytes,
                        value: state.read().progress.bytes_read,
                    }
                    p {
                        "{state.read().progress.bytes_read} / {state.read().progress.total_bytes} B · {state.read().progress.records_dispatched} "
                        {tr("records")}
                    }
                }
            }
            div { class: "my-5 flex flex-wrap gap-3",
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: state.read().phase != Phase::Ready || state.read().loading,
                    onclick: move |_| super::archive_history(state, false),
                    {tr("save-history")}
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: state.read().phase != Phase::Ready || state.read().loading
                        || state.read().plugins.is_empty(),
                    onclick: move |_| super::archive_history(state, true),
                    {tr("restore-history")}
                }
            }
            p { class: "text-sm text-muted-foreground", {tr("history-help")} }
            p { class: "mt-8 border-t border-border pt-5 text-sm text-muted-foreground",
                {tr("export-planned")}
            }
        }
    }
}

#[derive(Clone, PartialEq)]
struct AttackRow {
    name: String,
    category: String,
    direction: bool,
    swings: i32,
    damage: i64,
    hits: i32,
    critical: i32,
}

fn attack_rows(
    rows: &[aftercast::AttackSummary],
    combatant: &str,
    category: Option<&(String, bool)>,
) -> Vec<AttackRow> {
    // Keep ACT's aggregate rows distinct, e.g. outgoing damage and healing can both contain an All row.
    rows.iter()
        .filter(|row| {
            row.combatant == combatant
                && category.is_none_or(|(name, outgoing)| {
                    &row.category == name && row.outgoing == *outgoing
                })
        })
        .map(|row| AttackRow {
            name: row.attack.clone(),
            category: row.category.clone(),
            direction: row.outgoing,
            swings: row.swings,
            damage: row.damage,
            hits: row.hits,
            critical: row.crit_hits,
        })
        .collect()
}

#[component]
fn AttackTable(rows: Vec<AttackRow>) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let handle = use_table(move || {
        TableState::new(
            rows.iter()
                .cloned()
                .map(|row| (RowId::new(), row))
                .collect(),
            vec![
                ColumnDef::new(ColumnId("attack"), tr("attack"), |row: &AttackRow| {
                    CellValue::Text(row.name.clone())
                })
                .sortable(),
                ColumnDef::new(ColumnId("category"), tr("category"), |row: &AttackRow| {
                    CellValue::Text(row.category.clone())
                })
                .sortable(),
                ColumnDef::new(ColumnId("direction"), tr("direction"), |row: &AttackRow| {
                    CellValue::Text(tr(if row.direction {
                        "outgoing"
                    } else {
                        "incoming"
                    }))
                })
                .sortable(),
                ColumnDef::new(ColumnId("swings"), tr("swings"), |row: &AttackRow| {
                    CellValue::Integer(row.swings.into())
                })
                .alignment(Alignment::Right)
                .sortable(),
                ColumnDef::new(ColumnId("damage"), tr("total"), |row: &AttackRow| {
                    CellValue::Integer(row.damage)
                })
                .alignment(Alignment::Right)
                .sortable(),
                ColumnDef::new(ColumnId("hits"), tr("hits"), |row: &AttackRow| {
                    CellValue::Integer(row.hits.into())
                })
                .alignment(Alignment::Right)
                .sortable(),
                ColumnDef::new(
                    ColumnId("critical"),
                    tr("critical-hits"),
                    |row: &AttackRow| CellValue::Integer(row.critical.into()),
                )
                .alignment(Alignment::Right)
                .sortable(),
            ],
        )
    });
    let page = handle.signal().read().page;
    let pages = handle.signal().read().total_pages().max(1);
    rsx! {
        h2 { class: "mb-3 text-sm font-semibold", {tr("attack-summary")} }
        p { class: "mb-4 text-xs leading-6 text-muted-foreground", {tr("category-scope")} }
        div { class: "data-table overflow-auto tabular-nums [&_.chorale-root>div:last-child]:hidden",
            Table {
                handle,
                labels: table_labels(),
                theme: Theme::Custom,
                sort_enabled: true,
                resize_enabled: true,
                inline: true,
            }
        }
        div { class: "flex items-center gap-3 border-x border-b border-border bg-ctp-mantle px-3 py-2 text-xs text-muted-foreground",
            Button {
                variant: ButtonVariant::Ghost,
                disabled: page == 0,
                onclick: move |_| {
                    if let Err(error) = handle.set_page(page.saturating_sub(1)) {
                        state.write().fail(error);
                    }
                },
                {tr("previous")}
            }
            span { "{page + 1} / {pages}" }
            Button {
                variant: ButtonVariant::Ghost,
                disabled: page + 1 >= pages,
                onclick: move |_| {
                    if let Err(error) = handle.set_page(page + 1) {
                        state.write().fail(error);
                    }
                },
                {tr("next")}
            }
        }
    }
}

fn table_labels() -> Labels {
    let mut labels = Labels::default();
    labels.previous_page_label = tr("previous");
    labels.next_page_label = tr("next");
    labels.go_to_page_label = tr("page");
    labels.page_size_all_label = tr("all");
    labels.no_rows_label = tr("no-rows");
    labels.sort_ascending_label = tr("sort-ascending");
    labels.sort_descending_label = tr("sort-descending");
    labels.sort_none_label = tr("unsorted");
    let separator = tr("of");
    labels.page_count =
        std::sync::Arc::new(move |page, total| format!("{page} {separator} {total}"));
    labels
}

#[derive(Clone, PartialEq)]
struct Participant {
    name: String,
    damage: i64,
    healing: i64,
    dps: f64,
    deaths: i32,
}

#[component]
fn ParticipantTable(rows: Vec<Participant>) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let handle = use_table(move || {
        TableState::new(
            rows.iter()
                .cloned()
                .map(|row| (RowId::new(), row))
                .collect(),
            vec![
                ColumnDef::new(ColumnId("name"), tr("name"), |row: &Participant| {
                    CellValue::Text(row.name.clone())
                })
                .sortable(),
                ColumnDef::new(ColumnId("damage"), tr("damage"), |row: &Participant| {
                    CellValue::Integer(row.damage)
                })
                .alignment(Alignment::Right)
                .sortable(),
                ColumnDef::new(ColumnId("healing"), tr("healing"), |row: &Participant| {
                    CellValue::Integer(row.healing)
                })
                .alignment(Alignment::Right)
                .sortable(),
                ColumnDef::new(ColumnId("dps"), "DPS", |row: &Participant| {
                    CellValue::Float(row.dps)
                })
                .alignment(Alignment::Right)
                .sortable(),
                ColumnDef::new(ColumnId("deaths"), tr("deaths"), |row: &Participant| {
                    CellValue::Integer(i64::from(row.deaths))
                })
                .alignment(Alignment::Right)
                .sortable(),
            ],
        )
    });
    let page = handle.signal().read().page;
    let pages = handle.signal().read().total_pages().max(1);
    let count = handle.signal().read().filtered_row_count();
    // shortcut: hide Chorale 0.2's hardcoded English footer; remove when its row-count label is localizable.
    rsx! {
        div { class: "data-table overflow-auto tabular-nums [&_.chorale-root>div:last-child]:hidden",
            Table {
                handle,
                labels: table_labels(),
                theme: Theme::Custom,
                sort_enabled: true,
                resize_enabled: true,
                inline: true,
            }
            div { class: "flex items-center gap-3 border-x border-b border-border bg-ctp-mantle px-3 py-2 text-xs text-muted-foreground",
                Button {
                    variant: ButtonVariant::Ghost,
                    disabled: page == 0,
                    onclick: move |_| {
                        if let Err(error) = handle.set_page(page.saturating_sub(1)) {
                            state.write().fail(error);
                        }
                    },
                    {tr("previous")}
                }
                span { "{page + 1} / {pages}" }
                Button {
                    variant: ButtonVariant::Ghost,
                    disabled: page + 1 >= pages,
                    onclick: move |_| {
                        if let Err(error) = handle.set_page(page + 1) {
                            state.write().fail(error);
                        }
                    },
                    {tr("next")}
                }
                span { class: "ml-auto",
                    "{count} "
                    {tr("participants")}
                }
            }
        }
    }
}

#[component]
fn ContributionChart(mut rows: Vec<(String, i64)>) -> Element {
    rows.sort_by_key(|row| std::cmp::Reverse(row.1));
    rows.truncate(12);
    if rows.is_empty() {
        return rsx! {};
    }
    let labels: Vec<String> = rows.iter().map(|row| row.0.clone()).collect();
    // The chart is an approximate overview; the adjacent table retains exact signed i64 totals.
    let values: Vec<f32> = rows.iter().map(|row| row.1 as f32).collect();
    let highest = values.iter().copied().fold(1.0, f32::max);
    let format_label: fn(f32) -> String = |value| format!("{value:.0}");
    rsx! {
        section {
            class: "pt-2 pb-6 [&_h3]:mb-2 [&_h3]:text-xs [&_h3]:text-muted-foreground [&_text]:fill-ctp-subtext1 [&_text]:text-[11px]",
            "aria-label": tr("chart-description"),
            dioxus_charts::BarChart {
                series: vec![values],
                labels,
                horizontal_bars: true,
                highest,
                width: "100%",
                height: "180px",
                viewbox_width: 1000,
                viewbox_height: 220,
                padding_left: 130,
                padding_right: 40,
                padding_top: 12,
                padding_bottom: 30,
                label_interpolation: format_label,
                class_bar: "stroke-ctp-mauve",
                class_grid_line: "stroke-ctp-surface1",
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::attack_rows;
    use aftercast::AttackSummary;

    #[test]
    fn attack_filter_preserves_parser_categories_direction_and_aggregate_rows() {
        let row = AttackSummary {
            combatant: "A".into(),
            category: "Custom damage".into(),
            attack: "All".into(),
            outgoing: true,
            damage: 42,
            duration_ticks: 0,
            dps: 0.0,
            enc_dps: 0.0,
            swings: 2,
            hits: 1,
            crit_hits: 1,
            misses: 1,
            blocked: 0,
        };
        let incoming = AttackSummary {
            outgoing: false,
            damage: -1,
            ..row.clone()
        };
        let other_category = AttackSummary {
            category: "Custom healing".into(),
            ..row.clone()
        };
        let other_person = AttackSummary {
            combatant: "B".into(),
            ..row.clone()
        };
        let rows = [row, incoming, other_category, other_person];
        assert_eq!(attack_rows(&rows, "A", None).len(), 3);
        let selected = attack_rows(&rows, "A", Some(&("Custom damage".into(), false)));
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "All");
        assert_eq!(selected[0].damage, -1);
        assert!(attack_rows(&rows, "missing", None).is_empty());
    }
}
