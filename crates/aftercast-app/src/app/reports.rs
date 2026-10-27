use super::{AppState, Phase};
use aftercast::{EncounterId, HostHandle, TextReport, TextReportCatalog, TextReportOptions};
use camino::Utf8PathBuf;
use std::sync::Arc;
mod file;
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
    report: TextReport,
    options: TextReportOptions,
    title: String,
}
#[derive(Clone, Debug)]
enum Request {
    Catalog,
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
    Catalog(TextReportCatalog),
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
            Ok(Reply::Catalog(catalog)) => {
                if !self.reports.edited {
                    self.reports.draft = catalog.default_options.clone();
                }
                self.reports.catalog = Some(catalog);
            }
            Ok(Reply::Report {
                report: Some(report),
                options,
                title,
            }) => {
                self.reports.preview = Some(Preview {
                    token,
                    report,
                    options,
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
            let (path, text, control) =
                (path.clone(), preview.report.text.clone(), control.clone());
            tokio::task::spawn_blocking(move || file::save_new(&path, &text, &control))
                .await
                .map_err(|error| error.to_string())?
                .map(Reply::Save)
        }
    }
}
