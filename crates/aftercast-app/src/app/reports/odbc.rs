use super::super::tr;
use super::*;
use crate::components::{Button, ButtonVariant, Input, Label};
use aftercast::{HierarchyXmlDepth, OdbcPreview, TableLevel, TableScope};
use dioxus::prelude::*;
use std::fmt::Write;

#[derive(Clone)]
pub(super) struct Draft {
    pub(super) all_term: String,
}
impl Default for Draft {
    fn default() -> Self {
        Self {
            all_term: "All".into(),
        }
    }
}
impl AppState {
    pub(super) fn report_odbc_term(&mut self, token: Token, value: String) -> bool {
        if token != self.report_token()
            || self.screen != "analysis"
            || self.reports.pending.is_some()
            || !matches!(self.phase, Phase::Idle | Phase::Ready)
        {
            return false;
        }
        self.reports.odbc.all_term = value;
        self.reports.leave();
        true
    }
    pub(super) fn report_odbc_begin(
        &mut self,
        token: Token,
        current: bool,
    ) -> Result<Option<(Token, Request)>, &'static str> {
        if token != self.report_token() || !self.report_ready() {
            return Ok(None);
        }
        let target = self.selected.ok_or("report-select")?;
        let title = self
            .encounters
            .iter()
            .find(|row| row.id == target)
            .map(|row| format!("{} / {}", row.zone, row.title))
            .ok_or("report-select")?;
        let term = self.reports.odbc.all_term.clone();
        if term.is_empty() || term.len() > 1024 || term.contains('\0') {
            return Err("odbc-invalid-term");
        }
        Ok(Some(
            self.report_admit(Request::Odbc(target, current, term, title)),
        ))
    }
}
#[component]
pub(super) fn Editor() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let token = current.report_token();
    let ready = current.report_ready();
    let blocked =
        current.reports.pending.is_some() || !matches!(current.phase, Phase::Idle | Phase::Ready);
    let term = current.reports.odbc.all_term.clone();
    drop(current);
    rsx! {
        section {
            class: "space-y-4 border-t border-border pt-4",
            "aria-label": tr("odbc-preview-title"),
            h3 { class: "text-sm font-semibold", {tr("odbc-preview-title")} }
            p { class: "max-w-prose text-sm leading-7 text-muted-foreground",
                {tr("odbc-preview-help")}
            }
            Label { html_for: "odbc-all-term", {tr("odbc-all-term")} }
            Input {
                id: "odbc-all-term".to_owned(),
                value: term,
                disabled: blocked,
                oninput: move |event: FormEvent| {
                    state.write().report_odbc_term(token, event.value());
                },
            }
            div { class: "flex flex-wrap gap-2",
                for is_current in [false, true] {
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: !ready,
                        onclick: move |_| {
                            let work = state.write().report_odbc_begin(token, is_current);
                            match work {
                                Ok(work) => super::view::execute(state, work),
                                Err(key) => state.write().reports.error = Some(tr(key)),
                            }
                        },
                        {tr(if is_current { "odbc-current" } else { "odbc-hierarchy" })}
                    }
                }
            }
        }
    }
}
pub(super) async fn render(
    handle: &HostHandle,
    target: EncounterId,
    current: bool,
    all_term: &str,
) -> Result<Option<String>, String> {
    let levels = if current {
        vec![TableLevel::Combatant]
    } else {
        vec![
            TableLevel::Encounter,
            TableLevel::Combatant,
            TableLevel::Category,
            TableLevel::Attack,
            TableLevel::Action,
        ]
    };
    let mut columns = Vec::with_capacity(levels.len());
    for level in levels {
        let catalog = handle
            .table_columns(level)
            .await
            .map_err(|error| error.to_string())?;
        columns.push(
            catalog
                .columns
                .into_iter()
                .map(|column| column.sql_data_name)
                .collect::<Vec<_>>(),
        );
    }
    let plans = if current {
        let Some(catalog) = handle
            .table_report_catalog(&TableScope::Combatants(target))
            .await
            .map_err(|error| error.to_string())?
        else {
            return Ok(None);
        };
        let names = columns.first().ok_or("Missing synthetic columns")?;
        vec![
            handle
                .odbc_current_preview(&catalog.selection, names)
                .await
                .map_err(|error| error.to_string())?,
        ]
    } else {
        let names: [Vec<String>; 5] = columns
            .try_into()
            .map_err(|_columns| "Missing synthetic columns")?;
        let Some(selection) = handle
            .capture_hierarchy_xml(target, HierarchyXmlDepth::AllTables)
            .await
            .map_err(|error| error.to_string())?
        else {
            return Ok(None);
        };
        handle
            .odbc_hierarchy_preview(&selection, &names, all_term)
            .await
            .map_err(|error| error.to_string())?
            .tables
    };
    format_plans(&plans).map(Some)
}
fn format_plans(plans: &[OdbcPreview]) -> Result<String, String> {
    let mut output = String::new();
    for plan in plans {
        if let Some(reset) = &plan.reset_sql {
            writeln!(output, "{reset}").map_err(|e| e.to_string())?;
        }
        writeln!(output, "{}\n{}", plan.create_sql, plan.insert_sql).map_err(|e| e.to_string())?;
        for row in &plan.parameters {
            writeln!(output, "{row:?}").map_err(|e| e.to_string())?;
            if output.len() > 16 * 1024 * 1024 {
                return Err("ODBC preview exceeds 16 MiB".into());
            }
        }
    }
    Ok(output)
}
