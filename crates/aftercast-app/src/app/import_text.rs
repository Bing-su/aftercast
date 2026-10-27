use super::{AppState, Phase, tr};
use aftercast::{
    HostHandle, ImportEncoding, ImportInvalidBytes, ImportJob, ImportRange, ImportReport,
    ImportTextOptions,
};
use camino::{Utf8Path, Utf8PathBuf};
use dioxus::prelude::*;
pub(super) mod character;
#[cfg(test)]
mod tests;
mod view;

pub(super) const ENCODINGS: [(&str, ImportEncoding, &str); 9] = [
    ("utf8", ImportEncoding::Utf8, "UTF-8"),
    ("utf16-le", ImportEncoding::Utf16Le, "UTF-16 LE"),
    ("utf16-be", ImportEncoding::Utf16Be, "UTF-16 BE"),
    ("utf32-le", ImportEncoding::Utf32Le, "UTF-32 LE"),
    ("utf32-be", ImportEncoding::Utf32Be, "UTF-32 BE"),
    ("cp932", ImportEncoding::ShiftJis, "Windows Shift-JIS (932)"),
    (
        "cp949",
        ImportEncoding::WindowsKorean,
        "Windows Korean (949)",
    ),
    (
        "cp1252",
        ImportEncoding::Windows1252,
        "Windows Western (1252)",
    ),
    ("ascii", ImportEncoding::Ascii, "ASCII"),
];
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Draft {
    pub(super) encoding: Option<ImportEncoding>,
    pub(super) invalid_bytes: ImportInvalidBytes,
    pub(super) detect_bom: bool,
}
impl Default for Draft {
    fn default() -> Self {
        Self {
            encoding: None,
            invalid_bytes: ImportInvalidBytes::Strict,
            detect_bom: true,
        }
    }
}
impl Draft {
    pub(super) fn options(&self) -> Option<ImportTextOptions> {
        self.encoding.map(|encoding| ImportTextOptions {
            encoding,
            invalid_bytes: self.invalid_bytes,
            detect_bom: self.detect_bom,
        })
    }
    pub(super) fn key(&self) -> &'static str {
        ENCODINGS
            .iter()
            .find(|(_, encoding, _)| Some(*encoding) == self.encoding)
            .map_or("plugin", |(key, _, _)| *key)
    }
    pub(super) fn change(&mut self, field: &str, value: &str) -> Result<(), &'static str> {
        match field {
            "encoding" => {
                self.encoding = if value == "plugin" {
                    None
                } else {
                    Some(
                        ENCODINGS
                            .iter()
                            .find(|(key, _, _)| *key == value)
                            .ok_or("import-text-invalid-choice")?
                            .1,
                    )
                };
            }
            "invalid" if self.encoding.is_some() => {
                self.invalid_bytes = match value {
                    "strict" => ImportInvalidBytes::Strict,
                    "replace" => ImportInvalidBytes::Replace,
                    _ => return Err("import-text-invalid-choice"),
                }
            }
            "bom" if self.encoding.is_some() => {
                self.detect_bom = match value {
                    "true" => true,
                    "false" => false,
                    _ => return Err("import-text-invalid-choice"),
                }
            }
            _ => return Err("import-text-invalid-choice"),
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Token {
    pub(super) session: u64,
    generation: u64,
}
#[derive(Clone, Debug)]
pub(super) struct FullRequest {
    pub(super) token: Token,
    pub(super) path: Option<Utf8PathBuf>,
    draft: Draft,
    pub(super) sources: Option<super::companions::Selection>,
    pub(super) viewpoint: Option<aftercast::ImportCharacter>,
}
impl FullRequest {
    pub(super) fn options(&self) -> Option<ImportTextOptions> {
        self.draft.options()
    }
}
#[derive(Default)]
pub(super) struct State {
    draft: Draft,
    character: character::Draft,
    generation: u64,
    full: Option<FullRequest>,
}
impl State {
    pub(super) fn viewpoint(&self) -> Result<Option<aftercast::ImportCharacter>, &'static str> {
        self.character.options()
    }
    pub(super) fn source_snapshot(&self) -> Option<super::companions::ActiveImport> {
        let request = self.full.as_ref()?;
        Some(super::companions::ActiveImport::new(
            request.path.clone()?,
            None,
            request.options(),
            request.sources.as_ref(),
        ))
    }
    pub(super) fn options(&self) -> Option<ImportTextOptions> {
        self.draft.options()
    }
    pub(super) fn active(&self) -> bool {
        self.full.is_some()
    }
    pub(super) fn invalidate_view(&mut self) {
        self.generation += 1;
    }
    pub(super) fn reset_session(&mut self) {
        self.generation += 1;
        self.full = None;
    }
}
impl AppState {
    pub(super) fn text_view(&self) -> Token {
        Token {
            session: self.revision.session,
            generation: self.import_text.generation,
        }
    }
    fn can_edit_import_text(&self) -> bool {
        matches!(self.phase, Phase::Idle | Phase::Ready)
            && !self.loading
            && !self.exit_requested
            && !self.import_text.active()
            && !self.reparse.active()
            && !self.clipboard.active()
    }
    pub(super) fn edit_import_text(
        &mut self,
        expected: Token,
        field: &str,
        value: &str,
    ) -> Result<bool, &'static str> {
        if self.text_view() != expected || !self.can_edit_import_text() {
            return Ok(false);
        }
        self.import_text.draft.change(field, value)?;
        Ok(true)
    }
    pub(super) fn edit_import_path(&mut self, expected: Token, value: String) {
        if self.text_view() == expected && self.can_edit_import_text() {
            if self.import_path != value {
                self.companions.invalidate_catalog();
            }
            self.import_path = value;
        }
    }
    pub(super) fn can_full_import(&self) -> bool {
        self.phase == Phase::Ready
            && self.import_text.viewpoint().is_ok()
            && !self.history_sources.holds_resources()
            && !self.loading
            && !self.exit_requested
            && !self.import_text.active()
            && !self.clipboard.active()
            && self
                .plugins
                .iter()
                .any(|plugin| plugin.info.state == aftercast::PluginState::Initialized)
    }
    pub(super) fn select_full_import(&mut self) -> Option<FullRequest> {
        if !self.can_full_import() {
            return None;
        }
        self.import_text.generation += 1;
        let request = FullRequest {
            token: self.text_view(),
            path: None,
            draft: self.import_text.draft.clone(),
            sources: self.companions.selection(),
            viewpoint: self.import_text.viewpoint().ok()?,
        };
        self.import_text.full = Some(request.clone());
        self.phase = Phase::SelectingImport;
        Some(request)
    }
    pub(super) fn current_full_selection(&self, token: Token) -> bool {
        self.text_view() == token
            && self.phase == Phase::SelectingImport
            && !self.exit_requested
            && self
                .import_text
                .full
                .as_ref()
                .is_some_and(|request| request.token == token && request.path.is_none())
    }
    pub(super) fn abandon_full_selection(&mut self, token: Token) -> bool {
        if self.text_view() != token
            || self
                .import_text
                .full
                .as_ref()
                .is_none_or(|request| request.token != token || request.path.is_some())
        {
            return false;
        }
        self.import_text.full = None;
        self.import_text.invalidate_view();
        if self.phase == Phase::SelectingImport {
            self.phase = Phase::Ready;
        }
        true
    }
    pub(super) fn reject_full_selection(&mut self, token: Token, error: String) {
        if self.abandon_full_selection(token) {
            self.fail(error);
        }
    }
    pub(super) fn admit_full_path(
        &mut self,
        token: Token,
        path: Utf8PathBuf,
    ) -> Option<FullRequest> {
        if self.text_view() != token
            || self.phase != Phase::SelectingImport
            || self.exit_requested
            || self.loading
            || !self
                .plugins
                .iter()
                .any(|plugin| plugin.info.state == aftercast::PluginState::Initialized)
        {
            return None;
        }
        let request = self.import_text.full.as_mut()?;
        if request.token != token || request.path.is_some() {
            return None;
        }
        request.path = Some(path.clone());
        let request = request.clone();
        self.import_path = path.into_string();
        self.phase = Phase::Importing;
        self.cancel_import = false;
        self.progress = Default::default();
        self.message.clear();
        self.error = None;
        self.plugin_tables.invalidate();
        Some(request)
    }
    pub(super) fn receive_full_import(
        &mut self,
        token: Token,
        result: Result<ImportReport, aftercast::Error>,
        complete: &str,
        canceled: &str,
    ) -> bool {
        if self.text_view() != token
            || self
                .import_text
                .full
                .as_ref()
                .is_none_or(|request| request.token != token || request.path.is_none())
        {
            return false;
        }
        self.import_text.full = None;
        self.import_text.invalidate_view();
        self.phase = Phase::Ready;
        self.plugin_tables.invalidate();
        match result {
            Ok(report) => {
                self.message = format!(
                    "{complete} · {} / {} · {}",
                    report.records_dispatched, report.lines_read, report.encounters
                );
                self.companions.excluded = report.companion_heads_excluded
            }
            Err(aftercast::Error::Cancelled) => self.message = canceled.into(),
            Err(error) => self.fail(error),
        }
        true
    }
}
pub(super) async fn start_selected(
    handle: &HostHandle,
    path: &Utf8Path,
    range: Option<ImportRange>,
    text: Option<ImportTextOptions>,
    sources: Option<&super::companions::Selection>,
    viewpoint: Option<&aftercast::ImportCharacter>,
) -> Result<ImportJob, aftercast::Error> {
    if let Some(selection) = sources {
        return selection
            .start_with_viewpoint(handle, path, range, text, viewpoint)
            .await;
    }
    if let Some(viewpoint) = viewpoint {
        return handle
            .start_import_sources(
                path,
                aftercast::SourceImportOptions {
                    range,
                    text,
                    viewpoint: Some(viewpoint.clone()),
                    ..Default::default()
                },
            )
            .await;
    }
    start(handle, path, range, text).await
}
pub(super) async fn start(
    handle: &HostHandle,
    path: &Utf8Path,
    range: Option<ImportRange>,
    text: Option<ImportTextOptions>,
) -> Result<ImportJob, aftercast::Error> {
    match (range, text) {
        (None, None) => handle.start_import(path).await,
        (None, Some(text)) => handle.start_import_with_options(path, text).await,
        (Some(range), None) => handle.start_import_range(path, range).await,
        (Some(range), Some(text)) => {
            handle
                .start_import_range_with_options(path, range, text)
                .await
        }
    }
}
#[component]
pub(super) fn TextControls() -> Element {
    rsx! {
        view::Controls {}
    }
}
