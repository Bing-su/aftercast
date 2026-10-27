use super::*;
use aftercast::{
    TableLevel, TableReportCatalog, TableReportFormat, TableReportOptions, TableScope, TableSort,
};
#[derive(Clone, Debug)]
pub(super) struct Draft {
    pub level: TableLevel,
    pub actor: String,
    pub category: String,
    pub attack: String,
    pub name: String,
    pub format: TableReportFormat,
    pub selected: Vec<u32>,
    pub columns: Vec<String>,
    columns_edited: bool,
    pub primary: String,
    pub secondary: String,
    pub descending: bool,
    pub page: usize,
}
impl Default for Draft {
    fn default() -> Self {
        Self {
            level: TableLevel::Combatant,
            actor: String::new(),
            category: String::new(),
            attack: String::new(),
            name: String::new(),
            format: TableReportFormat::TableXml,
            selected: vec![],
            columns: vec![],
            columns_edited: false,
            primary: String::new(),
            secondary: String::new(),
            descending: false,
            page: 0,
        }
    }
}
impl Draft {
    fn move_column(&mut self, key: &str, up: bool) -> bool {
        let Some(index) = self.columns.iter().position(|value| value == key) else {
            return false;
        };
        let other = if up {
            index.checked_sub(1)
        } else {
            index
                .checked_add(1)
                .filter(|next| *next < self.columns.len())
        };
        let Some(other) = other else {
            return false;
        };
        self.columns.swap(index, other);
        self.columns_edited = true;
        true
    }
}
pub(super) fn context(scope: &TableScope) -> String {
    match scope {
        TableScope::Encounters => "Encounters".into(),
        TableScope::Combatants(_) => "Combatants".into(),
        TableScope::Categories { combatant, .. } => combatant.clone(),
        TableScope::Attacks {
            combatant,
            category,
            ..
        } => format!("{combatant} / {category}"),
        TableScope::Actions {
            combatant,
            category,
            attack,
            ..
        } => format!("{combatant} / {category} / {attack}"),
    }
}
impl AppState {
    pub(super) fn report_table_scope(&self) -> Option<TableScope> {
        let encounter = self.selected?;
        let draft = &self.reports.table;
        Some(match draft.level {
            TableLevel::Combatant => TableScope::Combatants(encounter),
            TableLevel::Category => TableScope::Categories {
                encounter,
                combatant: draft.actor.clone(),
            },
            TableLevel::Attack => TableScope::Attacks {
                encounter,
                combatant: draft.actor.clone(),
                category: draft.category.clone(),
            },
            TableLevel::Action => TableScope::Actions {
                encounter,
                combatant: draft.actor.clone(),
                category: draft.category.clone(),
                attack: draft.attack.clone(),
            },
            TableLevel::Encounter => return None,
        })
    }
    pub(super) fn report_table_catalog(&self) -> Option<&Arc<TableReportCatalog>> {
        self.reports
            .table_catalog
            .as_ref()
            .filter(|catalog| Some(catalog.selection.scope()) == self.report_table_scope().as_ref())
    }
    pub(super) fn report_table_edit(&mut self, token: Token, field: &str, value: String) -> bool {
        if token != self.report_token()
            || self.screen != "analysis"
            || self.reports.pending.is_some()
            || !matches!(self.phase, Phase::Idle | Phase::Ready)
        {
            return false;
        }
        let draft = &mut self.reports.table;
        match field {
            "level" => {
                draft.level = match value.as_str() {
                    "combatant" => TableLevel::Combatant,
                    "category" => TableLevel::Category,
                    "attack" => TableLevel::Attack,
                    "action" => TableLevel::Action,
                    _ => return false,
                }
            }
            "actor" => draft.actor = value,
            "category" => draft.category = value,
            "attack" => draft.attack = value,
            "name" => draft.name = value,
            "primary" => draft.primary = value,
            "secondary" => draft.secondary = value,
            "descending" if value == "true" || value == "false" => {
                draft.descending = value == "true"
            }
            "format" => {
                draft.format = match value.as_str() {
                    "xml" => TableReportFormat::TableXml,
                    "html" => TableReportFormat::StaticHtml,
                    _ => return false,
                }
            }
            "column" => {
                if let Some(index) = draft.columns.iter().position(|key| key == &value) {
                    draft.columns.remove(index);
                } else if draft.columns.len() < 32
                    && self.reports.table_catalog.as_ref().is_some_and(|catalog| {
                        catalog
                            .columns
                            .iter()
                            .any(|column| column.key == value && column.can_read)
                    })
                {
                    draft.columns.push(value);
                } else {
                    return false;
                }
                draft.columns_edited = true;
            }
            "up" | "down" => {
                if !draft.move_column(&value, field == "up") {
                    return false;
                }
            }
            "row" => {
                let Ok(index) = value.parse::<u32>() else {
                    return false;
                };
                if self
                    .reports
                    .table_catalog
                    .as_ref()
                    .is_none_or(|catalog| index >= catalog.selection.row_count())
                {
                    return false;
                }
                if let Some(at) = draft.selected.iter().position(|value| *value == index) {
                    draft.selected.remove(at);
                } else {
                    draft.selected.push(index);
                }
            }
            "page" => {
                let Ok(page) = value.parse::<usize>() else {
                    return false;
                };
                if self
                    .reports
                    .table_catalog
                    .as_ref()
                    .is_none_or(|catalog| page >= catalog.rows.len().div_ceil(16).max(1))
                {
                    return false;
                }
                draft.page = page;
                return true;
            }
            _ => return false,
        }
        if matches!(field, "level" | "actor" | "category" | "attack") {
            self.reports.table_catalog = None;
            self.reports.table.selected.clear();
            self.reports.table.page = 0;
        }
        self.reports.leave();
        true
    }
    pub(super) fn report_table_copy(&mut self, token: Token) -> bool {
        if token != self.report_token() || !self.report_ready() {
            return false;
        }
        let scope = super::super::plugin_tables::scope(self);
        if scope.level() == TableLevel::Encounter {
            return false;
        }
        let draft = &mut self.reports.table;
        draft.level = scope.level();
        match scope {
            TableScope::Encounters => {
                draft.level = TableLevel::Combatant;
                return false;
            }
            TableScope::Combatants(_) => {}
            TableScope::Categories { combatant, .. } => draft.actor = combatant,
            TableScope::Attacks {
                combatant,
                category,
                ..
            } => {
                draft.actor = combatant;
                draft.category = category;
            }
            TableScope::Actions {
                combatant,
                category,
                attack,
                ..
            } => {
                draft.actor = combatant;
                draft.category = category;
                draft.attack = attack;
            }
        }
        self.reports.table_catalog = None;
        self.reports.table.selected.clear();
        self.reports.table.page = 0;
        self.reports.leave();
        true
    }
    pub(super) fn receive_report_table_catalog(&mut self, catalog: TableReportCatalog) {
        // A new source-index capture retires row selections. Explicit column/sort drafts remain editable.
        if !self.reports.table.columns_edited {
            self.reports.table.columns = catalog
                .columns
                .iter()
                .filter(|column| column.default_visible && column.can_read)
                .take(32)
                .map(|column| column.key.clone())
                .collect();
        }
        self.reports.table.selected.clear();
        self.reports.table.page = 0;
        self.reports.table_catalog = Some(Arc::new(catalog));
    }
    pub(super) fn report_table_begin(
        &mut self,
        token: Token,
        render: bool,
    ) -> Result<Option<(Token, Request)>, &'static str> {
        if token != self.report_token() || !self.report_ready() {
            return Ok(None);
        }
        let scope = self.report_table_scope().ok_or("report-select")?;
        let request = if render {
            let catalog = self
                .report_table_catalog()
                .ok_or("table-report-load-first")?;
            let draft = &self.reports.table;
            if draft.format == TableReportFormat::StaticHtml && draft.columns.is_empty() {
                return Err("table-report-columns-required");
            }
            if draft.primary.is_empty() && !draft.secondary.is_empty() {
                return Err("table-report-sort-invalid");
            }
            let title = self
                .encounters
                .iter()
                .find(|row| Some(row.id) == self.selected)
                .map(|row| format!("{} / {} · {}", row.zone, row.title, context(&scope)))
                .ok_or("report-select")?;
            let options = TableReportOptions {
                format: draft.format,
                name: if draft.name.is_empty() {
                    title.clone()
                } else {
                    draft.name.clone()
                },
                selected_rows: if draft.format == TableReportFormat::TableXml {
                    draft.selected.clone()
                } else {
                    vec![]
                },
                columns: (draft.format == TableReportFormat::StaticHtml)
                    .then(|| draft.columns.clone()),
                sort: (!draft.primary.is_empty()).then(|| TableSort {
                    primary: draft.primary.clone(),
                    secondary: (!draft.secondary.is_empty()).then(|| draft.secondary.clone()),
                    descending: draft.descending,
                }),
            };
            Request::TableRender(catalog.selection.clone(), options, title)
        } else {
            self.reports.table_catalog = None;
            self.reports.table.selected.clear();
            Request::TableCatalog(scope)
        };
        Ok(Some(self.report_admit(request)))
    }
}
