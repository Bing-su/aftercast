use super::{AppState, Phase};
use aftercast::{ActExportOptions, ActFormat, EncounterId, HostHandle};
use camino::Utf8PathBuf;
#[cfg(test)]
mod tests;
mod view;
pub(super) use view::Panel;

#[derive(Clone, Debug, Default)]
struct Draft {
    input: String,
    output: String,
    options: ActExportOptions,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Token {
    session: u64,
    generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Import,
    Export,
}
#[derive(Clone, Debug)]
struct Target {
    id: EncounterId,
    title: String,
    selection: u64,
}
#[derive(Clone, Debug)]
struct Export {
    target: Target,
    path: Utf8PathBuf,
    options: ActExportOptions,
}
#[derive(Clone, Debug)]
enum Request {
    Import(Utf8PathBuf),
    Export(Export),
}
#[derive(Clone, Debug)]
struct Picker {
    token: Token,
    direction: Direction,
    selection: u64,
    target: Option<EncounterId>,
}
#[derive(Default)]
pub(super) struct State {
    draft: Draft,
    defaults_applied: bool,
    generation: u64,
    picker: Option<Picker>,
    confirmation: Option<(Token, Export)>,
    pending: Option<(Token, Request)>,
    error: Option<String>,
}
impl State {
    pub(super) fn apply_defaults_once(&mut self, include_logs: bool) {
        if !self.defaults_applied {
            self.draft.options.include_logs = include_logs;
            self.defaults_applied = true;
        }
    }
    pub(super) fn leave(&mut self) {
        self.confirmation = None;
        self.generation += 1;
    }
    pub(super) fn stop(&mut self) {
        self.generation += 1;
        self.picker = None;
        self.confirmation = None;
        self.pending = None;
        self.error = None;
    }
}
fn path(root: &camino::Utf8Path, value: &str) -> Result<Utf8PathBuf, &'static str> {
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err("act-invalid-path");
    }
    Ok(crate::config::resolve(root, value))
}
impl AppState {
    fn interchange_view(&self) -> Token {
        Token {
            session: self.revision.session,
            generation: self.interchange.generation,
        }
    }
    fn interchange_ready(&self) -> bool {
        self.phase == Phase::Ready
            && !self.loading
            && !self.exit_requested
            && self.interchange.pending.is_none()
            && self.interchange.picker.is_none()
            && !self.history_sources.holds_resources()
    }
    fn interchange_edit(&mut self, view: Token, field: &str, value: String) -> bool {
        if view != self.interchange_view()
            || self.interchange.pending.is_some()
            || self.interchange.picker.is_some()
            || !matches!(self.phase, Phase::Idle | Phase::Ready)
        {
            return false;
        }
        let draft = &mut self.interchange.draft;
        match field {
            "input" => draft.input = value,
            "output" => draft.output = value,
            "format" => match value.as_str() {
                "xml" => draft.options.format = ActFormat::Xml,
                "gzip" => draft.options.format = ActFormat::Gzip,
                _ => return false,
            },
            "logs" => match value.as_str() {
                "true" => draft.options.include_logs = true,
                "false" => draft.options.include_logs = false,
                _ => return false,
            },
            _ => return false,
        }
        self.interchange.defaults_applied = true;
        self.interchange.confirmation = None;
        true
    }
    fn interchange_target(&self) -> Option<Target> {
        let id = self.selected?;
        let summary = self
            .detail
            .as_ref()
            .map(|detail| &detail.summary)
            .filter(|summary| summary.id == id)
            .or_else(|| self.encounters.iter().find(|row| row.id == id))?;
        Some(Target {
            id,
            title: format!("{} / {}", summary.zone, summary.title),
            selection: self.revision.selection,
        })
    }
    fn begin_interchange_picker(&mut self, view: Token, direction: Direction) -> Option<Picker> {
        if view != self.interchange_view() || !self.interchange_ready() {
            return None;
        }
        self.interchange.generation += 1;
        let picker = Picker {
            token: self.interchange_view(),
            direction,
            selection: self.revision.selection,
            target: self.selected,
        };
        self.interchange.picker = Some(picker.clone());
        self.interchange.confirmation = None;
        self.interchange.error = None;
        self.phase = Phase::SelectingImport;
        Some(picker)
    }
    fn current_interchange_picker(&self, picker: &Picker) -> bool {
        self.interchange.picker.as_ref().is_some_and(|current| {
            current.token == picker.token && current.direction == picker.direction
        }) && picker.token == self.interchange_view()
            && self.phase == Phase::SelectingImport
            && !self.exit_requested
            && (picker.direction != Direction::Export
                || picker.selection == self.revision.selection && picker.target == self.selected)
    }
    fn receive_interchange_picker(
        &mut self,
        picker: &Picker,
        selected: Option<Utf8PathBuf>,
    ) -> bool {
        if picker.token.session != self.revision.session
            || self
                .interchange
                .picker
                .as_ref()
                .is_none_or(|current| current.token != picker.token)
        {
            return false;
        }
        let current = self.current_interchange_picker(picker);
        self.interchange.picker = None;
        self.phase = Phase::Ready;
        if !current {
            return false;
        }
        if let Some(selected) = selected {
            match picker.direction {
                Direction::Import => self.interchange.draft.input = selected.to_string(),
                Direction::Export => self.interchange.draft.output = selected.to_string(),
            };
        }
        true
    }
    fn review_act_export(&mut self, view: Token) -> Result<bool, &'static str> {
        if view != self.interchange_view() || !self.interchange_ready() {
            return Ok(false);
        }
        let target = self.interchange_target().ok_or("act-select-encounter")?;
        let export = Export {
            target,
            path: path(&self.root, &self.interchange.draft.output)?,
            options: self.interchange.draft.options,
        };
        self.interchange.generation += 1;
        self.interchange.confirmation = Some((self.interchange_view(), export));
        self.interchange.error = None;
        Ok(true)
    }
    fn cancel_act_confirmation(&mut self, view: Token) -> bool {
        if view != self.interchange_view() || !self.interchange_ready() {
            return false;
        }
        self.interchange.confirmation = None;
        true
    }
    fn begin_act_import(&mut self, view: Token) -> Result<Option<(Token, Request)>, &'static str> {
        if view != self.interchange_view() || !self.interchange_ready() {
            return Ok(None);
        }
        if !self
            .plugins
            .iter()
            .any(|plugin| plugin.info.state == aftercast::PluginState::Initialized)
        {
            return Err("history-parser-required");
        }
        let request = Request::Import(path(&self.root, &self.interchange.draft.input)?);
        Ok(Some(self.admit_interchange(request)))
    }
    fn confirm_act_export(&mut self, view: Token) -> Option<(Token, Request)> {
        if view != self.interchange_view() || !self.interchange_ready() {
            return None;
        }
        let (token, export) = self.interchange.confirmation.as_ref()?;
        if *token != view
            || self.selected != Some(export.target.id)
            || self.revision.selection != export.target.selection
        {
            return None;
        }
        let request = Request::Export(export.clone());
        Some(self.admit_interchange(request))
    }
    fn admit_interchange(&mut self, request: Request) -> (Token, Request) {
        self.interchange.generation += 1;
        let token = self.interchange_view();
        self.interchange.confirmation = None;
        self.interchange.pending = Some((token, request.clone()));
        self.interchange.error = None;
        self.phase = Phase::Archiving;
        self.message.clear();
        if matches!(request, Request::Import(_)) {
            self.invalidate_restored_context();
        }
        (token, request)
    }
    fn receive_interchange(
        &mut self,
        token: Token,
        result: Result<u32, aftercast::Error>,
        complete: &str,
    ) -> bool {
        if self.revision.session != token.session
            || self
                .interchange
                .pending
                .as_ref()
                .is_none_or(|(current, _)| *current != token)
        {
            return false;
        }
        let (_, request) = self
            .interchange
            .pending
            .take()
            .expect("checked pending interchange");
        self.phase = Phase::Ready;
        if matches!(request, Request::Import(_)) {
            self.invalidate_restored_context();
        }
        match result {
            Ok(count) => self.message = format!("{complete}: {count}"),
            Err(error) => self.interchange.error = Some(error.to_string()),
        }
        true
    }
}
async fn perform(handle: &HostHandle, request: &Request) -> Result<u32, aftercast::Error> {
    match request {
        Request::Import(path) => handle.import_act(path).await,
        Request::Export(export) => {
            handle
                .export_act_with_options(export.target.id, &export.path, export.options)
                .await?;
            Ok(1)
        }
    }
}
