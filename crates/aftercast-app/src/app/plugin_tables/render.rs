use super::*;
use aftercast::{TableCell, TableLevel, TableRow};
use chorale_core::{CellValue, ColumnDef, ColumnId, RowId, TableState, Theme};
use chorale_dioxus::{RowCellRenderer, RowCellRenderers, Table, use_table};
use std::{collections::HashMap, sync::Arc};
// Chorale 0.2 requires static IDs. Slots are per-frame positions, never persisted plugin identifiers.
const SLOTS: [&str; 32] = [
    "p0", "p1", "p2", "p3", "p4", "p5", "p6", "p7", "p8", "p9", "p10", "p11", "p12", "p13", "p14",
    "p15", "p16", "p17", "p18", "p19", "p20", "p21", "p22", "p23", "p24", "p25", "p26", "p27",
    "p28", "p29", "p30", "p31",
];
type Row = TableRow;
pub(super) fn next(row: &Row, level: TableLevel) -> Option<TableScope> {
    Some(match level {
        TableLevel::Encounter => TableScope::Combatants(row.encounter),
        TableLevel::Combatant => TableScope::Categories {
            encounter: row.encounter,
            combatant: row.combatant.clone()?,
        },
        TableLevel::Category => TableScope::Attacks {
            encounter: row.encounter,
            combatant: row.combatant.clone()?,
            category: row.category.clone()?,
        },
        TableLevel::Attack => TableScope::Actions {
            encounter: row.encounter,
            combatant: row.combatant.clone()?,
            category: row.category.clone()?,
            attack: row.attack.clone()?,
        },
        TableLevel::Action => return None,
    })
}
fn rgba(argb: i32) -> String {
    let value = argb.cast_unsigned();
    format!(
        "rgba({}, {}, {}, {:.4})",
        (value >> 16) & 255,
        (value >> 8) & 255,
        value & 255,
        f64::from(value >> 24) / 255.0
    )
}
#[cfg(test)]
pub(super) fn cell_style(cell: &TableCell) -> String {
    let mut style = String::new();
    if cell.error.is_some() {
        return style;
    }
    if let Some(value) = cell.foreground_argb {
        style.push_str(&format!("color:{};", rgba(value)));
    }
    if let Some(value) = cell.background_argb {
        style.push_str(&format!("background-color:{};", rgba(value)));
    }
    style
}
fn styled_cell(
    cell: &TableCell,
    appearance: &crate::config::AppearancePreferences,
    ally: bool,
    personal: bool,
    runtime: Option<aftercast::RuntimeColors>,
) -> String {
    if cell.error.is_some() {
        return String::new();
    }
    let mut style = appearance.grid_style();
    let configured = |value: &str| crate::config::valid_color(value) && !value.is_empty();
    let ally_color = runtime.map_or_else(
        || configured(&appearance.ally_foreground).then(|| appearance.ally_foreground.clone()),
        |r| r.ally.map(|v| rgba(v.cast_signed())),
    );
    let personal_color = runtime.map_or_else(
        || configured(&appearance.self_background).then(|| appearance.self_background.clone()),
        |r| r.personal.map(|v| rgba(v.cast_signed())),
    );
    let (invert, lum, sat) = runtime.map_or(
        (
            appearance.invert_plugin_colors,
            f32::from(appearance.plugin_luminosity) / 100.0,
            f32::from(appearance.plugin_saturation) / 100.0,
        ),
        |r| (r.invert, r.luminosity, r.saturation),
    );
    if let Some(color) = ally_color.filter(|_| ally) {
        style.push_str(&format!("color:{color};"));
    } else if let Some(color) = cell.foreground_argb {
        style.push_str(&format!(
            "color:{};",
            rgba(super::color_adjust::adjust_runtime(color, invert, lum, sat))
        ));
    } else if configured(&appearance.grid_foreground) {
        style.push_str(&format!("color:{};", appearance.grid_foreground));
    }
    if let Some(color) = personal_color.filter(|_| personal) {
        style.push_str(&format!("background-color:{color};"));
    } else if let Some(color) = cell.background_argb {
        style.push_str(&format!(
            "background-color:{};",
            rgba(super::color_adjust::adjust_runtime(color, invert, lum, sat))
        ));
    } else if configured(&appearance.grid_background) {
        style.push_str(&format!("background-color:{};", appearance.grid_background));
    }
    style
}
#[component]
fn CellView(cell: TableCell, ally: bool, personal: bool) -> Element {
    let state = use_context::<Signal<AppState>>();
    let current = state.read();
    let style = styled_cell(
        &cell,
        &current.settings.appearance,
        ally,
        personal,
        current.runtime_color_snapshot(),
    );
    drop(current);
    if cell.error.is_some() {
        rsx! {
            span { role: "note", class: "font-medium text-destructive", {tr("table-cell-error")} }
        }
    } else {
        rsx! {
            span { class: "block min-h-6 whitespace-pre-wrap px-1", style,
                {cell.text.as_deref().unwrap_or_default()}
            }
        }
    }
}
#[component]
fn OpenRow(view: View, target: Option<TableScope>) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let disabled = target.is_none() || !current_view(&state.read(), view);
    rsx! {
        crate::components::Button {
            variant: crate::components::ButtonVariant::Ghost,
            disabled,
            onclick: move |_| {
                if let Some(target) = target.clone() {
                    state.write().navigate_table(view, Some(target));
                }
            },
            {tr("table-open-row")}
        }
    }
}
// Keep RSX outside renderer closures so rustfmt and dx fmt agree on their shape.
fn row_link(view: View, target: Option<TableScope>) -> Element {
    rsx! {
        OpenRow { view, target }
    }
}
fn ally_first_column(is_ally: bool, index: usize) -> bool {
    is_ally && index == 0
}
fn row_cell(row: &Row, index: usize) -> Element {
    let Some(cell) = row.cells.get(index) else {
        return rsx! {};
    };
    rsx! {
        CellView {
            cell: cell.clone(),
            ally: ally_first_column(row.is_ally, index),
            personal: row.is_self,
        }
    }
}
#[component]
pub(super) fn Grid(page: TablePage, level: TableLevel, view: View) -> Element {
    let state = use_context::<Signal<AppState>>();
    let grid_style = state.read().settings.appearance.grid_style();
    let labels = page.columns.clone();
    let rows = page
        .rows
        .into_iter()
        .map(|row| (RowId::new(), row))
        .collect::<Vec<_>>();
    let mut definitions = vec![ColumnDef::new(
        ColumnId("open-row"),
        tr("table-context"),
        |_row: &Row| CellValue::Empty,
    )];
    let mut renderers = HashMap::<ColumnId, RowCellRenderer<Row>>::new();
    renderers.insert(
        ColumnId("open-row"),
        Arc::new(move |row, _| row_link(view, next(row, level))),
    );
    for (index, column) in labels.into_iter().enumerate() {
        let Some(slot) = SLOTS.get(index) else {
            continue;
        };
        let id = ColumnId(slot);
        definitions.push(ColumnDef::new(
            id,
            format!("{} [{}]", column.label, column.key),
            move |row: &Row| {
                row.cells
                    .get(index)
                    .and_then(|cell| cell.text.clone())
                    .map_or(CellValue::Empty, CellValue::Text)
            },
        ));
        renderers.insert(id, Arc::new(move |row, _| row_cell(row, index)));
    }
    let handle = use_table(move || {
        let mut table = TableState::new(rows.clone(), definitions.clone());
        table.page_size = PAGE as usize;
        table
    });
    rsx! {
        div {
            class: "plugin-data-table data-table overflow-auto tabular-nums [&_.chorale-root>div:last-child]:hidden",
            style: grid_style,
            Table {
                handle,
                labels: super::super::analysis::table_labels(),
                theme: Theme::Custom,
                row_cell_renderers: RowCellRenderers::new(renderers),
                sort_enabled: false,
                resize_enabled: true,
                inline: true,
            }
        }
    }
}

#[cfg(test)]
mod appearance_tests {
    use super::*;
    #[test]
    fn explicit_plugin_colors_override_only_the_matching_base_color() {
        let appearance = crate::config::AppearancePreferences {
            grid_font: "serif".into(),
            grid_size: 14,
            grid_foreground: "#123456".into(),
            grid_background: "#abcdef".into(),
            ..Default::default()
        };
        assert!(ally_first_column(true, 0));
        assert!(!ally_first_column(true, 1));
        assert!(!ally_first_column(false, 0));
        let mut cell = TableCell {
            text: Some("synthetic".into()),
            foreground_argb: Some(-65536),
            background_argb: None,
            error: None,
        };
        let style = styled_cell(&cell, &appearance, false, false, None);
        assert!(style.contains("font-family:serif;") && style.contains("font-size:14px;"));
        assert!(style.contains("color:rgba(255, 0, 0, 1.0000);") && !style.contains("#123456"));
        assert!(style.contains("background-color:#abcdef;"));
        cell.foreground_argb = None;
        cell.background_argb = Some(-16776961);
        let style = styled_cell(&cell, &appearance, false, false, None);
        assert!(
            style.contains("color:#123456;")
                && style.contains("background-color:rgba(0, 0, 255, 1.0000);")
        );
        assert!(!style.contains("#abcdef"));
        let mut invalid = appearance;
        invalid.grid_foreground = "red;display:none".into();
        cell.foreground_argb = None;
        cell.background_argb = None;
        assert!(!styled_cell(&cell, &invalid, false, false, None).contains("display"));
        cell.foreground_argb = Some(-65536);
        cell.background_argb = Some(-16776961);
        invalid.ally_foreground = "#112233".into();
        invalid.self_background = "#445566".into();
        let emphasized = styled_cell(&cell, &invalid, true, true, None);
        assert!(
            emphasized.contains("color:#112233;")
                && emphasized.contains("background-color:#445566;")
        );
        assert!(!emphasized.contains("rgba("));
        invalid.plugin_luminosity = 20;
        invalid.plugin_saturation = 20;
        cell.foreground_argb = Some(301924352);
        let adjusted = styled_cell(&cell, &invalid, false, false, None);
        assert!(adjusted.contains("color:rgba(255, 102, 102, 0.0667);"));
        let live = aftercast::RuntimeColors {
            invert: false,
            luminosity: 0.345,
            saturation: 0.0,
            ally: Some(0x4008090a),
            personal: Some(0xff010203),
        };
        let runtime = styled_cell(&cell, &invalid, true, true, Some(live));
        assert!(
            runtime.contains("color:rgba(8, 9, 10, 0.2510);")
                && runtime.contains("background-color:rgba(1, 2, 3, 1.0000);")
        );
        assert_eq!(invalid.ally_foreground, "#112233");
        let ally_priority = styled_cell(&cell, &invalid, true, true, None);
        assert!(
            ally_priority.contains("color:#112233;")
                && ally_priority.contains("background-color:#445566;")
        );
        invalid.plugin_luminosity = 0;
        invalid.plugin_saturation = 0;
        let ally_only = styled_cell(&cell, &invalid, true, false, None);
        assert!(
            ally_only.contains("color:#112233;")
                && ally_only.contains("background-color:rgba(0, 0, 255, 1.0000);")
        );
    }
}
