use super::*;
use aftercast::{EncounterSummary, HierarchyHtmlLevel, TableCatalog, TableLevel, TableSort};
pub(super) const LEVELS: [TableLevel; 4] = [
    TableLevel::Combatant,
    TableLevel::Category,
    TableLevel::Attack,
    TableLevel::Action,
];
pub(super) fn label(level: TableLevel) -> &'static str {
    match level {
        TableLevel::Combatant => "html-report-main",
        TableLevel::Category => "html-report-categories",
        TableLevel::Attack => "html-report-attacks",
        TableLevel::Action => "html-report-actions",
        TableLevel::Encounter => "html-report-title",
    }
}
#[derive(Clone, Default)]
pub(super) struct LevelDraft {
    pub columns: Vec<String>,
    edited: bool,
    pub primary: String,
    pub secondary: String,
    pub descending: bool,
    pub page: usize,
}
impl LevelDraft {
    fn settings(
        &self,
        level: TableLevel,
        catalog: &TableCatalog,
    ) -> Result<HierarchyHtmlLevel, &'static str> {
        if self.columns.is_empty() {
            return Err("table-report-columns-required");
        }
        if self.columns.iter().any(|key| {
            !catalog
                .columns
                .iter()
                .any(|column| &column.key == key && column.can_read)
        }) {
            return Err("html-report-missing");
        }
        if self.primary.is_empty() && !self.secondary.is_empty() {
            return Err("table-report-sort-invalid");
        }
        if [&self.primary, &self.secondary].into_iter().any(|key| {
            !key.is_empty()
                && !catalog
                    .columns
                    .iter()
                    .any(|column| &column.key == key && column.can_sort)
        }) {
            return Err("html-report-missing");
        }
        let sort = (!self.primary.is_empty()).then(|| TableSort {
            primary: self.primary.clone(),
            secondary: (!self.secondary.is_empty()).then(|| self.secondary.clone()),
            descending: self.descending,
        });
        Ok(HierarchyHtmlLevel {
            level,
            columns: self.columns.clone(),
            sort,
        })
    }
}
#[derive(Clone)]
pub(super) struct Draft {
    pub last: TableLevel,
    pub levels: [LevelDraft; 4],
    pub selected: Vec<EncounterSummary>,
    pub owner: Option<u64>,
    pub catalogs: Vec<Arc<TableCatalog>>,
    pub page: usize,
}
impl Default for Draft {
    fn default() -> Self {
        Self {
            last: TableLevel::Combatant,
            levels: Default::default(),
            selected: vec![],
            owner: None,
            catalogs: vec![],
            page: 0,
        }
    }
}
impl Draft {
    pub fn stop(&mut self) {
        self.selected.clear();
        self.owner = None;
        self.catalogs.clear();
        self.page = 0;
    }
}
impl AppState {
    pub(super) fn report_html_editable(&self, token: Token) -> bool {
        token == self.report_token()
            && self.screen == "analysis"
            && !self.exit_requested
            && !self.loading
            && self.reports.pending.is_none()
            && matches!(self.phase, Phase::Idle | Phase::Ready)
    }
    pub(super) fn report_html_toggle(
        &mut self,
        token: Token,
        id: EncounterId,
    ) -> Result<bool, &'static str> {
        if !self.report_ready() || token != self.report_token() {
            return Ok(false);
        }
        let same = self.reports.html.owner == Some(self.revision.session);
        if same
            && let Some(index) = self
                .reports
                .html
                .selected
                .iter()
                .position(|row| row.id == id)
        {
            self.reports.html.selected.remove(index);
        } else {
            let Some(row) = self.encounters.iter().find(|row| row.id == id).cloned() else {
                return Ok(false);
            };
            if same && self.reports.html.selected.len() >= 64 {
                return Err("html-report-limit");
            }
            if !same {
                self.reports.html.selected.clear();
            }
            self.reports.html.owner = Some(self.revision.session);
            self.reports.html.selected.push(row);
        }
        self.reports.leave();
        Ok(true)
    }
    pub(super) fn report_html_depth(&mut self, token: Token, level: TableLevel) -> bool {
        if !self.report_html_editable(token) || level == TableLevel::Encounter {
            return false;
        }
        self.reports.html.last = level;
        self.reports.leave();
        true
    }
    pub(super) fn report_html_page(&mut self, token: Token, page: usize) -> bool {
        if !self.report_html_editable(token) || page >= self.encounters.len().div_ceil(16).max(1) {
            return false;
        }
        self.reports.html.page = page;
        true
    }
    pub(super) fn report_html_edit(
        &mut self,
        token: Token,
        level: TableLevel,
        field: &str,
        value: String,
    ) -> bool {
        if !self.report_html_editable(token) || level == TableLevel::Encounter {
            return false;
        }
        let Some(index) = LEVELS.iter().position(|entry| *entry == level) else {
            return false;
        };
        let html = &mut self.reports.html;
        let Some(draft) = html.levels.get_mut(index) else {
            return false;
        };
        let catalog = html.catalogs.iter().find(|catalog| catalog.level == level);
        match field {
            "primary" if level != TableLevel::Category => draft.primary = value,
            "secondary" if level != TableLevel::Category => draft.secondary = value,
            "descending"
                if level != TableLevel::Category && matches!(value.as_str(), "true" | "false") =>
            {
                draft.descending = value == "true"
            }
            "column" => {
                if let Some(index) = draft.columns.iter().position(|key| key == &value) {
                    draft.columns.remove(index);
                } else if draft.columns.len() < 32
                    && catalog.is_some_and(|catalog| {
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
                draft.edited = true;
            }
            "up" | "down" => {
                let Some(at) = draft.columns.iter().position(|key| key == &value) else {
                    return false;
                };
                let other = if field == "up" {
                    at.checked_sub(1)
                } else {
                    at.checked_add(1).filter(|next| *next < draft.columns.len())
                };
                let Some(other) = other else {
                    return false;
                };
                draft.columns.swap(at, other);
                draft.edited = true;
            }
            "page" => {
                let Ok(page) = value.parse::<usize>() else {
                    return false;
                };
                if catalog.is_none_or(|catalog| page >= catalog.columns.len().div_ceil(16).max(1)) {
                    return false;
                }
                draft.page = page;
                return true;
            }
            _ => return false,
        }
        self.reports.leave();
        true
    }
    pub(super) fn receive_report_html_catalogs(&mut self, catalogs: Vec<TableCatalog>) {
        for (level, draft) in LEVELS.iter().zip(self.reports.html.levels.iter_mut()) {
            if !draft.edited
                && let Some(catalog) = catalogs.iter().find(|catalog| catalog.level == *level)
            {
                draft.columns = catalog
                    .columns
                    .iter()
                    .filter(|column| column.default_visible && column.can_read)
                    .take(32)
                    .map(|column| column.key.clone())
                    .collect();
            }
            draft.page = 0;
        }
        self.reports.html.catalogs = catalogs.into_iter().map(Arc::new).collect();
    }
    pub(super) fn report_html_begin(
        &mut self,
        token: Token,
        render: bool,
    ) -> Result<Option<(Token, Request)>, &'static str> {
        if token != self.report_token() || !self.report_ready() {
            return Ok(None);
        }
        let request = if render {
            let html = &self.reports.html;
            if html.owner != Some(self.revision.session) || html.selected.is_empty() {
                return Err("html-report-select");
            }
            if html.selected.len() > 64 {
                return Err("html-report-limit");
            }
            let mut settings = Vec::new();
            for (level, draft) in LEVELS
                .iter()
                .zip(html.levels.iter())
                .take(html.last as usize)
            {
                let catalog = html
                    .catalogs
                    .iter()
                    .find(|catalog| catalog.level == *level)
                    .ok_or("html-report-load-first")?;
                settings.push(draft.settings(*level, catalog)?);
            }
            Request::HtmlRender(html.selected.clone(), html.last, settings)
        } else {
            Request::HtmlCatalog
        };
        Ok(Some(self.report_admit(request)))
    }
}

pub(super) async fn catalogs(handle: &HostHandle) -> Result<Reply, String> {
    let mut catalogs = Vec::new();
    for level in LEVELS {
        catalogs.push(
            handle
                .table_columns(level)
                .await
                .map_err(|error| error.to_string())?,
        );
    }
    Ok(Reply::HtmlCatalog(catalogs))
}
pub(super) async fn render(
    handle: &HostHandle,
    selected: &[EncounterSummary],
    last: TableLevel,
    settings: &[HierarchyHtmlLevel],
) -> Result<Reply, String> {
    let ids: Vec<_> = selected.iter().map(|row| row.id).collect();
    let report = match handle
        .capture_hierarchy_html(&ids, last)
        .await
        .map_err(|error| error.to_string())?
    {
        Some(choice) => Some(
            handle
                .hierarchy_html(&choice, settings)
                .await
                .map_err(|error| error.to_string())?,
        ),
        None => None,
    };
    let ordered = match &report {
        Some(report) => report
            .selection
            .encounters()
            .iter()
            .map(|id| {
                selected
                    .iter()
                    .find(|row| row.id == *id)
                    .cloned()
                    .ok_or_else(|| "HTML snapshot encounter mismatch".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?,
        None => vec![],
    };
    Ok(Reply::HtmlReport(report, settings.to_vec(), ordered))
}
