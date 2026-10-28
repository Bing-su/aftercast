use super::*;
use aftercast::{
    EncounterSummary, HierarchyHtmlGraphOptions, HierarchyHtmlLevel, TableCatalog, TableLevel,
    TableSort,
};
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
    pub graph_levels: [bool; 4],
    pub graph_window: String,
    pub graph_rolling: bool,
    pub graph_distribution: String,
    pub graph_action_sort: String,
    pub graph_all: String,
    pub graph_ward: String,
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
            graph_levels: [false; 4],
            graph_window: "10".into(),
            graph_rolling: true,
            graph_distribution: String::new(),
            graph_action_sort: String::new(),
            graph_all: "All".into(),
            graph_ward: "Warded/".into(),
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
        for (index, selected) in self.reports.html.graph_levels.iter_mut().enumerate() {
            if index >= level as usize {
                *selected = false;
            }
        }
        self.reports.leave();
        true
    }
    pub(super) fn report_html_graph_edit(
        &mut self,
        token: Token,
        field: &str,
        value: String,
    ) -> bool {
        if !self.report_html_editable(token) {
            return false;
        }
        let draft = &mut self.reports.html;
        match field {
            "toggle" => match value.parse::<usize>() {
                Ok(index) if index < 4 => {
                    draft.graph_levels[index] = !draft.graph_levels[index];
                    if draft.graph_levels[index] && (draft.last as usize) < index + 1 {
                        draft.last = LEVELS[index];
                    }
                }
                _ => return false,
            },
            "clear" => draft.graph_levels = [false; 4],
            "window" => draft.graph_window = value,
            "rolling" if value == "true" || value == "false" => {
                draft.graph_rolling = value == "true"
            }
            "distribution" => draft.graph_distribution = value,
            "action_sort" => draft.graph_action_sort = value,
            "all" => draft.graph_all = value,
            "ward" => draft.graph_ward = value,
            _ => return false,
        }
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
            let graphs = if !html.graph_levels.iter().any(|selected| *selected) {
                None
            } else {
                let window = html
                    .graph_window
                    .parse::<u32>()
                    .ok()
                    .filter(|value| (1..=120).contains(value))
                    .ok_or("html-graph-invalid")?;
                if (html.graph_levels[2] && html.graph_distribution.is_empty())
                    || (html.graph_levels[3] && html.graph_action_sort.is_empty())
                    || html.graph_all.is_empty()
                    || (html.graph_levels[2]
                        && html.graph_distribution == "Resist"
                        && html.graph_ward.is_empty())
                {
                    return Err("html-graph-invalid");
                }
                Some(HierarchyHtmlGraphOptions {
                    window,
                    rolling: html.graph_rolling,
                    distribution_column: if html.graph_levels[2] {
                        html.graph_distribution.clone()
                    } else {
                        "Resist".into()
                    },
                    action_sort_column: if html.graph_levels[3] {
                        html.graph_action_sort.clone()
                    } else {
                        "unused".into()
                    },
                    all_label: html.graph_all.clone(),
                    ward_prefix: html.graph_ward.clone(),
                    selected_combatants: None,
                    solo_categories: None,
                    levels: LEVELS
                        .into_iter()
                        .zip(html.graph_levels)
                        .filter_map(|(level, selected)| selected.then_some(level))
                        .collect(),
                })
            };
            Request::HtmlRender(html.selected.clone(), html.last, settings, graphs)
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
    graphs: Option<&HierarchyHtmlGraphOptions>,
) -> Result<Reply, String> {
    let ids: Vec<_> = selected.iter().map(|row| row.id).collect();
    let report = match handle
        .capture_hierarchy_html(
            &ids,
            if graphs.is_some() {
                TableLevel::Action
            } else {
                last
            },
        )
        .await
        .map_err(|error| error.to_string())?
    {
        Some(choice) => Some(
            handle
                .hierarchy_html_with_graphs(&choice, settings, graphs)
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
    Ok(Reply::HtmlReport(
        report,
        settings.to_vec(),
        ordered,
        graphs.cloned(),
    ))
}
