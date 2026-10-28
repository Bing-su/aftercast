use super::{AppState, Phase};
use aftercast::{EncounterId, HostHandle, TextReport, TextReportCatalog, TextReportOptions};
use camino::Utf8PathBuf;
use std::sync::Arc;
mod file;
mod hierarchy;
mod html;
mod html_view;
mod table_view;
mod tables;
#[cfg(test)]
mod tests;
mod view;
use file::{SaveControl, Saved};
pub(super) use view::Panel;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Token {
    session: u64,
    epoch: u64,
    selection: u64,
    target: Option<EncounterId>,
}
#[derive(Clone, Debug)]
struct Preview {
    token: Token,
    report: Arc<Content>,
    title: String,
}
#[derive(Clone, Debug)]
enum Content {
    Text(TextReport, TextReportOptions),
    Table(aftercast::TableReport, aftercast::TableReportOptions),
    Hierarchy(aftercast::HierarchyXmlReport),
    Html(
        aftercast::HierarchyHtmlReport,
        Vec<aftercast::HierarchyHtmlLevel>,
        Vec<aftercast::EncounterSummary>,
        Option<aftercast::HierarchyHtmlGraphOptions>,
    ),
}
impl Content {
    fn text(&self) -> &str {
        match self {
            Self::Text(report, _) => &report.text,
            Self::Table(report, _) => &report.content,
            Self::Hierarchy(report) => &report.content,
            Self::Html(report, ..) => &report.content,
        }
    }
    fn preview_text(&self) -> &str {
        let text = self.text();
        let mut end = text.len().min(64 * 1024);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.get(..end).unwrap_or_default()
    }
    fn description(&self) -> String {
        match self {
            Self::Text(_, options) => format!("Text · {}", options.sorting),
            Self::Html(report, levels, _, graphs) => {
                let label = super::tr(html::label(
                    levels
                        .last()
                        .map_or(aftercast::TableLevel::Combatant, |level| level.level),
                ));
                if graphs.is_some() {
                    format!(
                        "{} · {} · {} encounters",
                        super::tr("html-report-title"),
                        label,
                        report.selection.encounters().len()
                    )
                } else {
                    format!(
                        "{} · {} · {} encounters · {} tables · {} rows",
                        super::tr("html-report-title"),
                        label,
                        report.selection.encounters().len(),
                        report.selection.table_count(),
                        report.selection.row_count()
                    )
                }
            }
            Self::Hierarchy(report) => format!(
                "{} · {} · {} tables · {} rows",
                super::tr("hierarchy-report-title"),
                super::tr(hierarchy::label(report.selection.depth())),
                report.selection.table_count(),
                report.selection.row_count()
            ),
            Self::Table(report, options) => format!(
                "{:?} · {} · {} · {}",
                report.format,
                tables::context(report.selection.scope()),
                options.name,
                options.sort.as_ref().map_or_else(
                    || "—".into(),
                    |sort| format!(
                        "{} / {} · {}",
                        sort.primary,
                        sort.secondary.as_deref().unwrap_or("—"),
                        if sort.descending { "↓" } else { "↑" }
                    )
                )
            ),
        }
    }
}
#[derive(Clone, Debug)]
enum Request {
    HtmlCatalog,
    HtmlGraphCandidates(Vec<aftercast::EncounterSummary>),
    HtmlRender(
        Vec<aftercast::EncounterSummary>,
        aftercast::TableLevel,
        Vec<aftercast::HierarchyHtmlLevel>,
        Option<aftercast::HierarchyHtmlGraphOptions>,
        Option<html::GraphCandidates>,
    ),
    Hierarchy(EncounterId, aftercast::HierarchyXmlDepth, String),
    Catalog,
    TableCatalog(aftercast::TableScope),
    TableRender(
        aftercast::TableReportSelection,
        aftercast::TableReportOptions,
        String,
    ),
    Render {
        catalog: TextReportCatalog,
        options: TextReportOptions,
        target: EncounterId,
        title: String,
    },
    Save {
        preview: Preview,
        path: Utf8PathBuf,
        control: Arc<SaveControl>,
    },
}
#[derive(Debug)]
enum Reply {
    HtmlCatalog(Vec<aftercast::TableCatalog>),
    HtmlGraphCandidates(html::GraphCandidates),
    HtmlReport(
        Option<aftercast::HierarchyHtmlReport>,
        Vec<aftercast::HierarchyHtmlLevel>,
        Vec<aftercast::EncounterSummary>,
        Option<aftercast::HierarchyHtmlGraphOptions>,
    ),
    Hierarchy(Option<aftercast::HierarchyXmlReport>, String),
    Catalog(TextReportCatalog),
    TableCatalog(Option<aftercast::TableReportCatalog>),
    TableReport(
        aftercast::TableReport,
        aftercast::TableReportOptions,
        String,
    ),
    Report {
        report: Option<TextReport>,
        options: TextReportOptions,
        title: String,
    },
    Save(Saved),
}
#[derive(Default)]
pub(super) struct State {
    epoch: u64,
    hierarchy: hierarchy::Draft,
    html: html::Draft,
    table: tables::Draft,
    table_catalog: Option<Arc<aftercast::TableReportCatalog>>,
    catalog: Option<TextReportCatalog>,
    draft: TextReportOptions,
    edited: bool,
    output: String,
    preview: Option<Preview>,
    pending: Option<(Token, Request)>,
    confirmation: Option<(Token, Preview, Utf8PathBuf)>,
    error: Option<String>,
    preset_page: usize,
}
impl State {
    pub(super) fn leave(&mut self) {
        if let Some((_, Request::Save { control, .. })) = &self.pending {
            control.cancel();
        }
        self.epoch += 1;
        self.preview = None;
        self.confirmation = None;
    }
    pub(super) fn stop(&mut self) {
        self.leave();
        self.pending = None;
        self.catalog = None;
        self.table_catalog = None;
        self.html.stop();
        self.error = None;
    }
}
impl AppState {
    fn report_token(&self) -> Token {
        Token {
            session: self.revision.session,
            epoch: self.reports.epoch,
            selection: self.revision.selection,
            target: self.selected,
        }
    }
    fn report_ready(&self) -> bool {
        self.screen == "analysis"
            && self.phase == Phase::Ready
            && !self.loading
            && !self.exit_requested
            && self.reports.pending.is_none()
            && !self.history_sources.holds_resources()
    }
    fn report_preview(&self) -> Option<&Preview> {
        self.reports
            .preview
            .as_ref()
            .filter(|preview| preview.token == self.report_token())
    }
    fn report_edit(&mut self, token: Token, field: &str, value: String) -> bool {
        if token != self.report_token()
            || self.screen != "analysis"
            || self.reports.pending.is_some()
            || !matches!(self.phase, Phase::Idle | Phase::Ready)
        {
            return false;
        }
        match field {
            "player" => self.reports.draft.player_format = value,
            "allied" => self.reports.draft.allies_format = value,
            "sort" => self.reports.draft.sorting = value,
            "only" if value == "true" || value == "false" => {
                self.reports.draft.show_only_allies = value == "true"
            }
            "group" if value == "true" || value == "false" => {
                self.reports.draft.show_allied_info = value == "true"
            }
            "path" => {
                let current = self.report_preview().is_some();
                self.reports.output = value;
                self.reports.confirmation = None;
                self.reports.epoch += 1;
                let token = self.report_token();
                if current && let Some(preview) = &mut self.reports.preview {
                    preview.token = token;
                }
                return true;
            }
            _ => return false,
        }
        self.reports.edited = true;
        self.reports.leave();
        true
    }
    fn report_preset(&mut self, token: Token, index: Option<usize>) -> bool {
        if token != self.report_token() || !self.report_ready() {
            return false;
        }
        let Some(catalog) = &self.reports.catalog else {
            return false;
        };
        let options = match index {
            Some(index) => catalog
                .presets
                .get(index)
                .map(|preset| preset.options.clone()),
            None => Some(catalog.default_options.clone()),
        };
        let Some(options) = options else {
            return false;
        };
        self.reports.draft = options;
        self.reports.edited = true;
        self.reports.leave();
        true
    }
    fn report_begin(
        &mut self,
        token: Token,
        render: bool,
    ) -> Result<Option<(Token, Request)>, &'static str> {
        if token != self.report_token() || !self.report_ready() {
            return Ok(None);
        }
        let request = if render {
            let target = self.selected.ok_or("report-select")?;
            let catalog = self.reports.catalog.clone().ok_or("report-load-first")?;
            let title = self
                .encounters
                .iter()
                .find(|row| row.id == target)
                .map(|row| format!("{} / {}", row.zone, row.title))
                .ok_or("report-select")?;
            Request::Render {
                catalog,
                options: self.reports.draft.clone(),
                target,
                title,
            }
        } else {
            self.reports.catalog = None;
            self.reports.preset_page = 0;
            Request::Catalog
        };
        Ok(Some(self.report_admit(request)))
    }
    fn report_admit(&mut self, request: Request) -> (Token, Request) {
        if matches!(&request, Request::HtmlRender(_, _, _, _, None)) {
            self.reports.html.graph_candidates = None;
        }
        self.reports.epoch += 1;
        let token = self.report_token();
        if matches!(request, Request::Save { .. }) {
            if let Some(preview) = &mut self.reports.preview {
                preview.token = token;
            }
        } else {
            self.reports.preview = None;
        }
        self.reports.confirmation = None;
        self.reports.error = None;
        self.reports.pending = Some((token, request.clone()));
        self.phase = Phase::MakingReport;
        (token, request)
    }
    fn report_receive(
        &mut self,
        token: Token,
        result: Result<Reply, String>,
        complete: &str,
    ) -> bool {
        if token.session != self.revision.session
            || self
                .reports
                .pending
                .as_ref()
                .is_none_or(|(current, _)| *current != token)
        {
            return false;
        }
        self.reports.pending = None;
        self.phase = Phase::Ready;
        if token != self.report_token() || self.exit_requested || self.screen != "analysis" {
            return false;
        }
        match result {
            Ok(Reply::HtmlCatalog(catalogs)) => self.receive_report_html_catalogs(catalogs),
            Ok(Reply::HtmlGraphCandidates(candidates)) => {
                self.reports.html.graph_candidates = Some(candidates)
            }
            Ok(Reply::HtmlReport(Some(report), levels, selected, graphs)) => {
                self.reports.preview = Some(Preview {
                    token,
                    title: "HTML".to_owned(),
                    report: Arc::new(Content::Html(report, levels, selected, graphs)),
                });
            }
            Ok(Reply::HtmlReport(None, ..)) => {
                self.reports.error = Some("report-unavailable".into())
            }
            Ok(Reply::Hierarchy(Some(report), title)) => {
                self.reports.preview = Some(Preview {
                    token,
                    report: Arc::new(Content::Hierarchy(report)),
                    title,
                });
            }
            Ok(Reply::Hierarchy(None, _)) => self.reports.error = Some("report-unavailable".into()),
            Ok(Reply::Catalog(catalog)) => {
                if !self.reports.edited {
                    self.reports.draft = catalog.default_options.clone();
                }
                self.reports.catalog = Some(catalog);
            }
            Ok(Reply::TableCatalog(Some(catalog))) => self.receive_report_table_catalog(catalog),
            Ok(Reply::TableCatalog(None)) => self.reports.error = Some("report-unavailable".into()),
            Ok(Reply::TableReport(report, options, title)) => {
                self.reports.preview = Some(Preview {
                    token,
                    report: Arc::new(Content::Table(report, options)),
                    title,
                });
            }
            Ok(Reply::Report {
                report: Some(report),
                options,
                title,
            }) => {
                self.reports.preview = Some(Preview {
                    token,
                    report: Arc::new(Content::Text(report, options)),
                    title,
                })
            }
            Ok(Reply::Report { report: None, .. }) => {
                self.reports.error = Some("report-unavailable".into())
            }
            Ok(Reply::Save(_)) => self.message = complete.to_owned(),
            Err(error) => self.reports.error = Some(error),
        }
        true
    }
    fn report_cancel(&mut self, token: Token) -> bool {
        if token != self.report_token() {
            return false;
        }
        self.reports.leave();
        true
    }
    fn report_review_save(&mut self, token: Token) -> Result<bool, &'static str> {
        if token != self.report_token() || !self.report_ready() {
            return Ok(false);
        }
        let mut preview = self
            .report_preview()
            .ok_or("report-generate-first")?
            .clone();
        let raw = self.reports.output.trim();
        if raw.is_empty() || raw.chars().any(char::is_control) {
            return Err("act-invalid-path");
        }
        let path = crate::config::resolve(&self.root, raw);
        self.reports.epoch += 1;
        let token = self.report_token();
        preview.token = token;
        self.reports.preview = Some(preview.clone());
        self.reports.confirmation = Some((token, preview, path));
        Ok(true)
    }
    fn report_cancel_review(&mut self, token: Token) -> bool {
        if token != self.report_token() || !self.report_ready() {
            return false;
        }
        self.reports.confirmation = None;
        self.reports.epoch += 1;
        let token = self.report_token();
        if let Some(preview) = &mut self.reports.preview {
            preview.token = token;
        }
        true
    }
    fn report_confirm_save(&mut self, token: Token) -> Option<(Token, Request)> {
        if token != self.report_token() || !self.report_ready() {
            return None;
        }
        let (current, preview, path) = self.reports.confirmation.as_ref()?;
        if *current != token || preview.token != token {
            return None;
        }
        Some(self.report_admit(Request::Save {
            preview: preview.clone(),
            path: path.clone(),
            control: Arc::default(),
        }))
    }
}
async fn perform(handle: &HostHandle, request: &Request) -> Result<Reply, String> {
    match request {
        Request::HtmlCatalog => html::catalogs(handle).await,
        Request::HtmlGraphCandidates(selected) => html::candidates(handle, selected)
            .await
            .map(Reply::HtmlGraphCandidates),
        Request::HtmlRender(selected, last, settings, graphs, candidates) => {
            html::render(
                handle,
                selected,
                *last,
                settings,
                graphs.as_ref(),
                candidates.as_ref(),
            )
            .await
        }
        Request::Hierarchy(target, depth, title) => {
            let report = match handle
                .capture_hierarchy_xml(*target, *depth)
                .await
                .map_err(|error| error.to_string())?
            {
                Some(selection) => Some(
                    handle
                        .hierarchy_xml(&selection)
                        .await
                        .map_err(|error| error.to_string())?,
                ),
                None => None,
            };
            Ok(Reply::Hierarchy(report, title.clone()))
        }
        Request::TableCatalog(scope) => handle
            .table_report_catalog(scope)
            .await
            .map(Reply::TableCatalog)
            .map_err(|error| error.to_string()),
        Request::TableRender(selection, options, title) => handle
            .table_report(selection, options)
            .await
            .map(|report| Reply::TableReport(report, options.clone(), title.clone()))
            .map_err(|error| error.to_string()),
        Request::Catalog => handle
            .text_report_catalog()
            .await
            .map(Reply::Catalog)
            .map_err(|error| error.to_string()),
        Request::Render {
            catalog,
            options,
            target,
            title,
        } => handle
            .text_report(*target, catalog.revision, options)
            .await
            .map(|report| Reply::Report {
                report,
                options: options.clone(),
                title: title.clone(),
            })
            .map_err(|error| error.to_string()),
        Request::Save {
            preview,
            path,
            control,
        } => {
            let (path, text, control) = (
                path.clone(),
                preview.report.text().to_owned(),
                control.clone(),
            );
            tokio::task::spawn_blocking(move || file::save_new(&path, &text, &control))
                .await
                .map_err(|error| error.to_string())?
                .map(Reply::Save)
        }
    }
}
