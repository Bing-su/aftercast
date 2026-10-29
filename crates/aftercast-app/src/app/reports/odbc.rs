use super::super::tr;
use super::*;
use crate::components::{Button, ButtonVariant, Input, Label};
use aftercast::{HierarchyXmlDepth, OdbcPreview, OdbcRewriteRule, TableLevel, TableScope};
use dioxus::prelude::*;
use std::fmt::Write;

#[derive(Clone)]
pub(super) struct Draft {
    pub(super) all_term: String,
    connection: String,
    find: String,
    replacement: String,
    pub(super) rules: Vec<OdbcRewriteRule>,
}
impl Default for Draft {
    fn default() -> Self {
        Self {
            all_term: "All".into(),
            connection: "^offline-preview$".into(),
            find: String::new(),
            replacement: String::new(),
            rules: Vec::new(),
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
    pub(super) fn report_odbc_rule_edit(
        &mut self,
        token: Token,
        field: &str,
        value: String,
    ) -> bool {
        if token != self.report_token()
            || self.screen != "analysis"
            || self.reports.pending.is_some()
            || !matches!(self.phase, Phase::Idle | Phase::Ready)
        {
            return false;
        }
        match field {
            "connection" => self.reports.odbc.connection = value,
            "find" => self.reports.odbc.find = value,
            "replacement" => self.reports.odbc.replacement = value,
            _ => return false,
        }
        self.reports.leave();
        true
    }
    pub(super) fn report_odbc_rule_add(&mut self, token: Token) -> Result<bool, &'static str> {
        if token != self.report_token()
            || self.screen != "analysis"
            || self.reports.pending.is_some()
            || !matches!(self.phase, Phase::Idle | Phase::Ready)
        {
            return Ok(false);
        }
        let draft = &mut self.reports.odbc;
        if draft.rules.len() >= 16
            || draft.find.is_empty()
            || draft.find.len() > 512
            || draft.connection.len() > 512
            || draft.replacement.len() > 1024
            || draft.find.contains('\0')
            || draft.connection.contains('\0')
            || draft.replacement.contains('\0')
        {
            return Err("odbc-rule-invalid");
        }
        draft.rules.push(OdbcRewriteRule {
            connection_pattern: draft.connection.clone(),
            find: draft.find.clone(),
            replacement: draft.replacement.clone(),
        });
        self.reports.leave();
        Ok(true)
    }
    pub(super) fn report_odbc_rule_remove(&mut self, token: Token, index: usize) -> bool {
        if token != self.report_token()
            || self.screen != "analysis"
            || self.reports.pending.is_some()
            || !matches!(self.phase, Phase::Idle | Phase::Ready)
            || index >= self.reports.odbc.rules.len()
        {
            return false;
        }
        self.reports.odbc.rules.remove(index);
        self.reports.leave();
        true
    }
    pub(super) fn report_odbc_batch_begin(
        &mut self,
        token: Token,
    ) -> Result<Option<(Token, Request)>, &'static str> {
        if token != self.report_token() || !self.report_ready() {
            return Ok(None);
        }
        let selection = &self.reports.html;
        if selection.owner != Some(self.revision.session) || selection.selected.is_empty() {
            return Err("odbc-batch-select");
        }
        let ids: Vec<_> = selection.selected.iter().map(|row| row.id).collect();
        let term = self.reports.odbc.all_term.clone();
        if term.is_empty() || term.len() > 1024 || term.contains('\0') {
            return Err("odbc-invalid-term");
        }
        let rules = self.reports.odbc.rules.clone();
        self.reports.html.graph_candidates = None;
        let title = format!("ODBC · {} encounters", ids.len());
        Ok(Some(
            self.report_admit(Request::OdbcBatch(ids, term, rules, title)),
        ))
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
    let connection = current.reports.odbc.connection.clone();
    let find = current.reports.odbc.find.clone();
    let replacement = current.reports.odbc.replacement.clone();
    let rules = current.reports.odbc.rules.clone();
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
            p { class: "text-sm text-muted-foreground", {tr("odbc-batch-help")} }
            for (field, value, label) in [
                ("connection", connection, "odbc-rule-connection"),
                ("find", find, "odbc-rule-find"),
                ("replacement", replacement, "odbc-rule-replacement"),
            ]
            {
                div { class: "space-y-1",
                    Label { html_for: format!("odbc-rule-{field}"), {tr(label)} }
                    Input {
                        id: format!("odbc-rule-{field}"),
                        value,
                        disabled: blocked,
                        oninput: move |event: FormEvent| {
                            state.write().report_odbc_rule_edit(token, field, event.value());
                        },
                    }
                }
            }
            Button {
                variant: ButtonVariant::Outline,
                disabled: blocked,
                onclick: move |_| {
                    let result = state.write().report_odbc_rule_add(token);
                    if let Err(key) = result {
                        state.write().reports.error = Some(tr(key));
                    }
                },
                {tr("odbc-rule-add")}
            }
            for (index, rule) in rules.iter().enumerate() {
                div { class: "flex items-center gap-2",
                    span { class: "break-all text-sm",
                        "{rule.connection_pattern} · {rule.find} → {rule.replacement}"
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked,
                        onclick: move |_| {
                            state.write().report_odbc_rule_remove(token, index);
                        },
                        {tr("odbc-rule-remove")}
                    }
                }
            }
            Button {
                variant: ButtonVariant::Outline,
                disabled: !ready,
                onclick: move |_| {
                    let work = state.write().report_odbc_batch_begin(token);
                    match work {
                        Ok(work) => super::view::execute(state, work),
                        Err(key) => state.write().reports.error = Some(tr(key)),
                    }
                },
                {tr("odbc-batch")}
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
pub(super) async fn render_batch(
    handle: &HostHandle,
    encounters: &[EncounterId],
    all_term: &str,
    rules: &[OdbcRewriteRule],
) -> Result<Option<String>, String> {
    let mut columns = Vec::new();
    for level in [
        TableLevel::Encounter,
        TableLevel::Combatant,
        TableLevel::Category,
        TableLevel::Attack,
        TableLevel::Action,
    ] {
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
    let names: [Vec<String>; 5] = columns
        .try_into()
        .map_err(|_columns| "Missing synthetic columns")?;
    let Some(selection) = handle
        .capture_hierarchy_html(encounters, TableLevel::Action)
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let plan = handle
        .odbc_batch_preview(&selection, &names, all_term, rules)
        .await
        .map_err(|error| error.to_string())?;
    format_plans(&plan.tables).map(Some)
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
