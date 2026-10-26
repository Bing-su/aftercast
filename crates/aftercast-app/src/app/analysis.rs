use super::{AppState, Phase, tr};
use crate::components::{Button, ButtonVariant, Input, Label, Progress};
use chorale_core::{CellValue, ColumnDef, ColumnId, Labels, RowId, TableState, Theme};
use chorale_dioxus::{Table, use_table};
use dioxus::prelude::*;

#[component]
pub(super) fn AnalysisPane() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let detail = state.read().detail.clone();
    rsx! {
        section { class: "p-6 min-[1000px]:px-8 min-[1000px]:py-7",
            div { class: "mb-7 flex items-start justify-between gap-5 [&_h1]:text-2xl [&_h1]:font-semibold [&_h1]:tracking-tight [&_p]:mt-2 [&_p]:max-w-[68ch] [&_p]:text-[13px] [&_p]:leading-7 [&_p]:text-muted-foreground",
                div {
                    h1 { {tr("analysis")} }
                    p { {tr("analysis-description")} }
                }
            }
            div { class: "mb-6",
                Label {
                    class: "mt-4 mb-2 text-[13px]".to_owned(),
                    html_for: "log-path",
                    {tr("log-path")}
                }
                Input {
                    id: "log-path".to_owned(),
                    value: state.read().import_path.clone(),
                    placeholder: tr("log-path-hint"),
                    disabled: state.read().phase == Phase::Importing,
                    oninput: move |event: FormEvent| state.write().import_path = event.value(),
                }
            }
            if state.read().phase == Phase::Importing {
                div {
                    class: "mb-6 border border-border bg-ctp-mantle p-5 [&_p]:mt-2 [&_p]:text-xs [&_p]:text-muted-foreground [&_p]:tabular-nums",
                    role: "status",
                    div { class: "flex items-center justify-between",
                        strong { {tr("importing")} }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: state.read().cancel_import,
                            onclick: move |_| state.write().cancel_import = true,
                            {tr("cancel")}
                        }
                    }
                    Progress {
                        class: "mt-4".to_owned(),
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
            if let Some(detail) = detail {
                div { class: "mb-4 flex flex-wrap items-baseline gap-4 [&_h2]:text-lg [&_h2]:font-semibold [&_span]:text-xs [&_span]:text-muted-foreground",
                    h2 { "{detail.summary.title}" }
                    span { "{detail.summary.zone}" }
                }
                div { class: "summary-strip mb-5 flex gap-9 border-t border-border py-5 tabular-nums [&_span]:block [&_span]:text-xs [&_span]:text-muted-foreground [&_strong]:mt-1 [&_strong]:block [&_strong]:text-lg [&_strong]:font-medium",
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
                ContributionChart {
                    rows: detail
                        .combatants
                        .iter()
                        .filter(|row| row.damage > 0)
                        .map(|row| (row.name.clone(), row.damage))
                        .collect(),
                }
                {
                    rsx! {
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
                }
                p { class: "my-6 max-w-[70ch] text-xs leading-7 text-muted-foreground",
                    {tr("aggregate-scope")}
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
                .sortable(),
                ColumnDef::new(ColumnId("healing"), tr("healing"), |row: &Participant| {
                    CellValue::Integer(row.healing)
                })
                .sortable(),
                ColumnDef::new(ColumnId("dps"), "DPS", |row: &Participant| {
                    CellValue::Float(row.dps)
                })
                .sortable(),
                ColumnDef::new(ColumnId("deaths"), tr("deaths"), |row: &Participant| {
                    CellValue::Integer(i64::from(row.deaths))
                })
                .sortable(),
            ],
        )
    });
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
    let page = handle.signal().read().page;
    let pages = handle.signal().read().total_pages().max(1);
    let count = handle.signal().read().filtered_row_count();
    // shortcut: hide Chorale 0.2's hardcoded English footer; remove when its row-count label is localizable.
    rsx! {
        div { class: "data-table overflow-auto tabular-nums [&_.chorale-root>div:last-child]:hidden",
            Table {
                handle,
                labels,
                theme: Theme::Custom,
                sort_enabled: true,
                resize_enabled: true,
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
            h3 { {tr("chart-description")} }
            dioxus_charts::BarChart {
                series: vec![values],
                labels,
                horizontal_bars: true,
                highest,
                width: "100%",
                height: "220px",
                viewbox_width: 700,
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
