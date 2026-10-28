use super::*;
use aftercast::{
    EncounterSummary, HierarchyHtmlGraphOptions, HierarchyHtmlLevel, HierarchyHtmlSelection,
    TableCatalog, TableLevel, TableScope, TableSort,
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct GraphCandidates {
    pub encounters: Vec<EncounterId>,
    pub actors: Vec<String>,
    pub categories: Vec<String>,
    pub selection: HierarchyHtmlSelection,
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
    pub graph_simple: bool,
    pub graph_simple_column: String,
    pub graph_only_allies: bool,
    pub graph_totals: bool,
    pub graph_distribution: String,
    pub graph_action_sort: String,
    pub graph_all: String,
    pub graph_ward: String,
    pub graph_candidates: Option<GraphCandidates>,
    pub graph_actors: Option<Vec<String>>,
    pub graph_categories: Option<Vec<String>>,
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
            graph_simple: true,
            graph_simple_column: String::new(),
            graph_only_allies: true,
            graph_totals: true,
            graph_distribution: String::new(),
            graph_action_sort: String::new(),
            graph_all: "All".into(),
            graph_ward: "Warded/".into(),
            graph_candidates: None,
            graph_actors: None,
            graph_categories: None,
        }
    }
}
fn toggle_graph_choice(available: &[String], choice: &mut Option<Vec<String>>, key: &str) -> bool {
    let Some(selected) = choice.as_mut() else {
        return false;
    };
    if let Some(index) = selected.iter().position(|item| item == key) {
        selected.remove(index);
        return true;
    }
    if !available.iter().any(|item| item == key) {
        return false;
    }
    selected.push(key.to_owned());
    selected.sort_by_key(|item| {
        available
            .iter()
            .position(|value| value == item)
            .unwrap_or(usize::MAX)
    });
    true
}
fn choice_missing(values: Option<&Vec<String>>, available: &[String]) -> bool {
    values.is_some_and(|values| values.iter().any(|value| !available.contains(value)))
}
impl Draft {
    pub(super) fn restore_graphs(&mut self, graph: &crate::config::GraphSettings) {
        self.graph_levels = graph.levels;
        self.graph_window = graph.window.to_string();
        self.graph_rolling = graph.rolling;
        self.graph_simple = graph.simple;
        self.graph_simple_column = graph.simple_column.clone();
        self.graph_only_allies = graph.only_allies;
        self.graph_totals = graph.totals;
        self.graph_distribution = graph.distribution.clone();
        self.graph_action_sort = graph.action_sort.clone();
        self.graph_all = graph.all_label.clone();
        self.graph_ward = graph.ward_prefix.clone();
        self.graph_categories = graph.solo_categories.clone();
        self.graph_candidates = None;
        if let Some((index, _)) = self
            .graph_levels
            .iter()
            .enumerate()
            .rev()
            .find(|(_, enabled)| **enabled)
            && let Some(level) = LEVELS.get(index)
        {
            self.last = *level;
        }
    }
    pub(super) fn graph_settings(&self) -> Result<crate::config::GraphSettings, &'static str> {
        let window = self
            .graph_window
            .parse::<u32>()
            .ok()
            .filter(|v| (1..=120).contains(v))
            .ok_or("html-graph-invalid")?;
        let graph = crate::config::GraphSettings {
            levels: self.graph_levels,
            window,
            rolling: self.graph_rolling,
            simple: self.graph_simple,
            simple_column: self.graph_simple_column.clone(),
            only_allies: self.graph_only_allies,
            totals: self.graph_totals,
            distribution: self.graph_distribution.clone(),
            action_sort: self.graph_action_sort.clone(),
            all_label: self.graph_all.clone(),
            ward_prefix: self.graph_ward.clone(),
            solo_categories: self.graph_categories.clone(),
        };
        graph.validate().map_err(|_error| "html-graph-invalid")?;
        Ok(graph)
    }
    fn graph_capture(
        &self,
        graph: Option<&HierarchyHtmlGraphOptions>,
    ) -> Result<Option<GraphCandidates>, &'static str> {
        let Some(graph) = graph else {
            return Ok(None);
        };
        if graph.selected_combatants.is_none() && graph.solo_categories.is_none() {
            return Ok(None);
        }
        let candidates = self
            .graph_candidates
            .as_ref()
            .ok_or("html-graph-load-candidates")?;
        if candidates.encounters != self.selected.iter().map(|row| row.id).collect::<Vec<_>>() {
            return Err("html-graph-load-candidates");
        }
        if choice_missing(graph.selected_combatants.as_ref(), &candidates.actors)
            || choice_missing(graph.solo_categories.as_ref(), &candidates.categories)
        {
            return Err("html-graph-missing-choices");
        }
        Ok(Some(candidates.clone()))
    }
    fn graphs(&self) -> Result<Option<HierarchyHtmlGraphOptions>, &'static str> {
        if !self.graph_levels.iter().any(|selected| *selected) {
            Ok(None)
        } else {
            let needs_window = self.graph_levels[1] || (self.graph_levels[0] && !self.graph_simple);
            let window = if needs_window {
                self.graph_window
                    .parse::<u32>()
                    .ok()
                    .filter(|value| (1..=120).contains(value))
                    .ok_or("html-graph-invalid")?
            } else {
                10
            };
            if (self.graph_levels[0] && self.graph_simple && self.graph_simple_column.is_empty())
                || (self.graph_levels[2] && self.graph_distribution.is_empty())
                || (self.graph_levels[3] && self.graph_action_sort.is_empty())
                || self.graph_all.is_empty()
                || (self.graph_levels[2]
                    && self.graph_distribution == "Resist"
                    && self.graph_ward.is_empty())
            {
                return Err("html-graph-invalid");
            }
            Ok(Some(HierarchyHtmlGraphOptions {
                window,
                rolling: self.graph_rolling,
                simple: self.graph_simple,
                simple_column: self.graph_simple_column.clone(),
                only_allies: self.graph_only_allies,
                totals: self.graph_totals,
                distribution_column: if self.graph_levels[2] {
                    self.graph_distribution.clone()
                } else {
                    "Resist".into()
                },
                action_sort_column: if self.graph_levels[3] {
                    self.graph_action_sort.clone()
                } else {
                    "unused".into()
                },
                all_label: self.graph_all.clone(),
                ward_prefix: self.graph_ward.clone(),
                selected_combatants: self.graph_levels[0]
                    .then(|| self.graph_actors.clone())
                    .flatten(),
                solo_categories: self.graph_levels[1]
                    .then(|| self.graph_categories.clone())
                    .flatten(),
                levels: LEVELS
                    .into_iter()
                    .zip(self.graph_levels)
                    .filter_map(|(level, selected)| selected.then_some(level))
                    .collect(),
            }))
        }
    }
    pub fn stop(&mut self) {
        self.selected.clear();
        self.owner = None;
        self.catalogs.clear();
        self.graph_candidates = None;
        self.graph_actors = None;
        self.page = 0;
    }
}
impl AppState {
    pub(super) fn report_html_editable(&self, token: Token) -> bool {
        token == self.report_token()
            && self.screen == "analysis"
            && !self.exit_requested
            && !self.loading
            && !self.saving
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
        self.reports.html.graph_candidates = None;
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
    pub(super) fn report_html_prepare_graph_save(
        &mut self,
        token: Token,
    ) -> Result<bool, &'static str> {
        if !self.report_html_editable(token) || !self.can_save_settings() {
            return Ok(false);
        }
        self.settings.graphs = self.reports.html.graph_settings()?;
        Ok(true)
    }
    pub(super) fn report_html_graph_candidates_begin(
        &mut self,
        token: Token,
    ) -> Result<Option<(Token, Request)>, &'static str> {
        if !self.report_html_editable(token) {
            return Ok(None);
        }
        if self.reports.html.owner != Some(self.revision.session)
            || self.reports.html.selected.is_empty()
        {
            return Err("html-report-select");
        }
        self.reports.html.graph_candidates = None;
        self.reports.table_catalog = None;
        self.reports.table.selected.clear();
        self.reports.table.page = 0;
        Ok(Some(self.report_admit(Request::HtmlGraphCandidates(
            self.reports.html.selected.clone(),
        ))))
    }
    pub(super) fn report_html_graph_scope(&mut self, token: Token, field: &str, key: &str) -> bool {
        if !self.report_html_editable(token) {
            return false;
        }
        let draft = &mut self.reports.html;
        let Some(candidates) = &draft.graph_candidates else {
            return false;
        };
        let (available, choice, mode) = match field {
            "actors" => (&candidates.actors, &mut draft.graph_actors, false),
            "categories" => (&candidates.categories, &mut draft.graph_categories, false),
            "actors_mode" => (&candidates.actors, &mut draft.graph_actors, true),
            "categories_mode" => (&candidates.categories, &mut draft.graph_categories, true),
            _ => return false,
        };
        if mode {
            match key {
                "all" => *choice = None,
                "custom" => *choice = Some(Vec::new()),
                _ => return false,
            }
        } else if !toggle_graph_choice(available, choice, key) {
            return false;
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
                Ok(index) => {
                    let (Some(selected), Some(level)) =
                        (draft.graph_levels.get_mut(index), LEVELS.get(index))
                    else {
                        return false;
                    };
                    *selected = !*selected;
                    if *selected && (draft.last as usize) < index + 1 {
                        draft.last = *level;
                    }
                }
                _ => return false,
            },
            "clear" => draft.graph_levels = [false; 4],
            "simple" if value == "true" || value == "false" => draft.graph_simple = value == "true",
            "simple_column" => draft.graph_simple_column = value,
            "only_allies" if value == "true" || value == "false" => {
                draft.graph_only_allies = value == "true"
            }
            "totals" if value == "true" || value == "false" => draft.graph_totals = value == "true",
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
            let graphs = html.graphs()?;
            let candidates = html.graph_capture(graphs.as_ref())?;
            Request::HtmlRender(
                html.selected.clone(),
                html.last,
                settings,
                graphs,
                candidates,
            )
        } else {
            Request::HtmlCatalog
        };
        Ok(Some(self.report_admit(request)))
    }
}

pub(super) async fn candidates(
    handle: &HostHandle,
    selected: &[EncounterSummary],
) -> Result<GraphCandidates, String> {
    let ids: Vec<_> = selected.iter().map(|row| row.id).collect();
    let selection = handle
        .capture_hierarchy_html(&ids, TableLevel::Action)
        .await
        .map_err(|error| error.to_string())?
        .ok_or("Encounter no longer exists")?;
    let category_catalog = handle
        .table_columns(TableLevel::Category)
        .await
        .map_err(|error| error.to_string())?;
    let mut categories = Vec::new();
    for category in category_catalog.categories {
        if !categories.contains(&category.key) {
            categories.push(category.key);
        }
    }
    if categories.len() > 256 {
        return Err("Graph category candidates exceed 256".into());
    }
    let mut actors = Vec::new();
    for row in selected {
        let catalog = handle
            .table_report_catalog(&TableScope::Combatants(row.id))
            .await
            .map_err(|error| error.to_string())?
            .ok_or("Encounter no longer exists")?;
        for actor in catalog.rows.into_iter().filter_map(|entry| entry.combatant) {
            if !actors.contains(&actor) {
                actors.push(actor);
            }
            if actors.len() > 256 {
                return Err("Graph combatant candidates exceed 256".into());
            }
        }
    }
    Ok(GraphCandidates {
        encounters: selected.iter().map(|row| row.id).collect(),
        actors,
        categories,
        selection,
    })
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
    expected: Option<&GraphCandidates>,
) -> Result<Reply, String> {
    let ids: Vec<_> = selected.iter().map(|row| row.id).collect();
    let choice = match expected {
        Some(expected) if expected.encounters == ids => Some(expected.selection.clone()),
        Some(_) => return Err("Graph choices changed; reload candidates".into()),
        None => handle
            .capture_hierarchy_html(
                &ids,
                if graphs.is_some() {
                    TableLevel::Action
                } else {
                    last
                },
            )
            .await
            .map_err(|error| error.to_string())?,
    };
    let report = match choice {
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
