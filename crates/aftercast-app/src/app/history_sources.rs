use super::{AppState, Phase, import_text};
use aftercast::{
    CapturedHistoryFile, CompanionCatalog, CompanionSource, HistoryCaptureOptions,
    HistoryCaptureProgress, HistoryEntry, HistoryRevision, HistorySourceCapture,
    HistorySourceProposal, HistorySourceQuery, HistorySourceSnapshot, HistorySourceState,
    HostHandle, ImportJob, ImportRange,
};
use camino::Utf8PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[cfg(test)]
mod tests;
mod view;
pub(super) use view::{Panel, choose, leave};

#[derive(Clone, Debug)]
pub(super) struct Selection {
    pub revision: HistoryRevision,
    pub entry: HistoryEntry,
}
#[derive(Clone, Debug, Default)]
struct Draft {
    path: String,
    root: String,
    filter: String,
    limit: String,
    text: import_text::Draft,
}
impl Draft {
    fn query(&self, root: &camino::Utf8Path) -> Result<HistorySourceQuery, &'static str> {
        if self.path.trim().is_empty() && self.root.trim().is_empty() {
            return Err("history-source-scope-required");
        }
        let path = |value: &str| {
            if value.trim().is_empty() {
                vec![]
            } else {
                vec![crate::config::resolve(root, value.trim())]
            }
        };
        Ok(HistorySourceQuery {
            paths: path(&self.path),
            roots: path(&self.root),
            filter: if self.filter.is_empty() {
                "*.log".into()
            } else {
                self.filter.clone()
            },
        })
    }
    fn limit(&self) -> Result<u64, &'static str> {
        let value = if self.limit.is_empty() {
            4096
        } else {
            self.limit
                .parse::<u64>()
                .map_err(|_error| "history-source-invalid-limit")?
        };
        if !(1..=4096).contains(&value) {
            return Err("history-source-invalid-limit");
        }
        Ok(value * 1024 * 1024)
    }
}
#[derive(Clone, Debug)]
struct Choice {
    name: String,
    text: import_text::Draft,
}
#[derive(Clone, Debug)]
struct CaptureSnapshot {
    bounds: String,
    path: Utf8PathBuf,
    options: HistoryCaptureOptions,
    range: ImportRange,
}
struct Owned {
    host: HostHandle,
    proposal: Option<HistorySourceProposal>,
    capture: Option<HistorySourceCapture>,
}
impl Owned {
    fn new(host: HostHandle) -> Self {
        Self {
            host,
            proposal: None,
            capture: None,
        }
    }
    fn active(&self) -> bool {
        self.proposal.is_some() || self.capture.is_some()
    }
    async fn release_capture(&mut self) -> Result<(), String> {
        let Some(capture) = &mut self.capture else {
            return Ok(());
        };
        let result = self.host.release_history_capture(capture).await;
        // Managed cmd8 retires ownership even when reporting a terminal capture/cleanup fault.
        if result.is_ok()
            || matches!(
                result,
                Err(aftercast::Error::Managed { .. }
                    | aftercast::Error::Closed
                    | aftercast::Error::WorkerFailed(_))
            )
        {
            self.capture = None;
        }
        result.map_err(|error| error.to_string())
    }
    async fn release_proposal(&mut self) -> Result<(), String> {
        let Some(proposal) = &mut self.proposal else {
            return Ok(());
        };
        let result = self.host.release_history_sources(proposal).await;
        if result.is_ok()
            || matches!(
                result,
                Err(aftercast::Error::Managed { .. }
                    | aftercast::Error::Closed
                    | aftercast::Error::WorkerFailed(_))
            )
        {
            self.proposal = None;
        }
        result.map_err(|error| error.to_string())
    }
    async fn release(&mut self) -> Result<(), String> {
        let mut errors = Vec::new();
        if let Err(error) = self.release_capture().await {
            errors.push(error);
        }
        if self.capture.is_none()
            && let Err(error) = self.release_proposal().await
        {
            errors.push(error);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("\n"))
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Token {
    session: u64,
    generation: u64,
}
#[derive(Clone)]
enum SourceAction {
    History(Box<Selection>),
    Discover,
    Path(Utf8PathBuf),
    Capture,
    Replay,
    Release,
}
struct Work {
    token: Token,
    action: SourceAction,
    owned: Owned,
    selection: Option<Selection>,
    query: Option<HistorySourceQuery>,
    capture: Option<CaptureSnapshot>,
    canceled: Arc<AtomicBool>,
}
struct Output {
    owned: Owned,
    snapshot: Option<HistorySourceSnapshot>,
    catalog: Option<CompanionCatalog>,
    job: Option<ImportJob>,
    error: Option<String>,
    canceled: bool,
}
#[derive(Default)]
pub(super) struct State {
    generation: u64,
    pending: Option<Token>,
    cancel: Arc<AtomicBool>,
    owned: Option<Owned>,
    selection: Option<Selection>,
    draft: Draft,
    snapshot: Option<HistorySourceSnapshot>,
    path: Option<Utf8PathBuf>,
    catalog: Option<CompanionCatalog>,
    choices: Vec<Choice>,
    choice_path: Option<Utf8PathBuf>,
    admitted: Option<CaptureSnapshot>,
    files: Vec<CapturedHistoryFile>,
    ready: bool,
    progress: Option<HistoryCaptureProgress>,
    error: Option<String>,
}
impl State {
    fn available(&self, path: &camino::Utf8Path) -> bool {
        self.snapshot.as_ref().is_some_and(|snapshot| {
            snapshot
                .candidates
                .iter()
                .any(|item| item.path == path && item.state == HistorySourceState::Available)
        })
    }
    fn capture_request(&self) -> Result<CaptureSnapshot, &'static str> {
        let selection = self
            .selection
            .as_ref()
            .ok_or("history-source-select-history")?;
        let path = self
            .path
            .clone()
            .filter(|path| self.available(path))
            .ok_or("history-source-select-file")?;
        if self
            .catalog
            .as_ref()
            .is_none_or(|catalog| catalog.path != path)
        {
            return Err("history-source-select-file");
        }
        Ok(CaptureSnapshot {
            bounds: format!(
                "{} — {}",
                super::encounter_context::format_time(selection.entry.start),
                super::encounter_context::format_time(selection.entry.end)
            ),
            path,
            range: ImportRange::new(selection.entry.start.ticks, selection.entry.end.ticks)
                .map_err(|_error| "reparse-invalid-range")?,
            options: HistoryCaptureOptions {
                text: self.draft.text.options(),
                companions: self
                    .choices
                    .iter()
                    .map(|choice| CompanionSource {
                        name: choice.name.clone(),
                        text: choice.text.options(),
                    })
                    .collect(),
                maximum_bytes: self.draft.limit()?,
            },
        })
    }
    fn finish_action(
        &mut self,
        action: &SourceAction,
        selection: Option<Selection>,
        output: &mut Output,
    ) {
        match action {
            SourceAction::History(_) => {
                if output.error.is_none() {
                    self.selection = selection;
                }
                self.clear_image();
                self.snapshot = None;
                self.path = None;
                self.catalog = None;
            }
            SourceAction::Discover => {
                self.clear_image();
                self.snapshot = output.snapshot.take();
                self.path = None;
                self.catalog = None;
            }
            SourceAction::Path(_) => self.accept_catalog(output.catalog.take()),
            SourceAction::Capture => {
                self.files = output
                    .owned
                    .capture
                    .as_ref()
                    .and_then(|capture| capture.files())
                    .map_or_else(Vec::new, |files| files.to_vec());
                self.ready = !self.files.is_empty();
            }
            SourceAction::Replay => {
                self.ready = output
                    .owned
                    .capture
                    .as_ref()
                    .is_some_and(|capture| capture.files().is_some());
                if !self.ready {
                    self.clear_image();
                }
            }
            SourceAction::Release => {
                self.clear_image();
                self.snapshot = None;
                self.path = None;
                self.catalog = None;
            }
        }
    }
    fn accept_catalog(&mut self, catalog: Option<CompanionCatalog>) {
        if let Some(catalog) = &catalog {
            self.choices.retain(|choice| {
                catalog
                    .candidates
                    .iter()
                    .any(|candidate| candidate.name == choice.name)
            });
        }
        self.catalog = catalog;
    }
    fn clear_image(&mut self) {
        self.admitted = None;
        self.ready = false;
        self.files.clear();
        self.progress = None;
    }
    pub(super) fn holds_resources(&self) -> bool {
        self.pending.is_some() || self.owned.as_ref().is_some_and(Owned::active)
    }
    pub(super) fn invalidate(&mut self) {
        self.cancel.store(true, Ordering::Release);
        self.generation += 1;
        self.snapshot = None;
        self.path = None;
        self.catalog = None;
        self.admitted = None;
        self.files.clear();
        self.ready = false;
        self.selection = None;
        self.progress = None;
    }
    pub(super) fn stop(&mut self) {
        self.invalidate();
        self.pending = None;
        self.owned = None;
    }
}
impl AppState {
    pub(in crate::app) fn history_source_view(&self) -> Token {
        Token {
            session: self.revision.session,
            generation: self.history_sources.generation,
        }
    }
    pub(in crate::app) fn source_idle(&self) -> bool {
        self.phase == Phase::Ready
            && !self.loading
            && !self.exit_requested
            && self.history_sources.pending.is_none()
    }
    fn begin_source_work(
        &mut self,
        view: Token,
        action: SourceAction,
        host: HostHandle,
    ) -> Result<Option<Work>, &'static str> {
        if view != self.history_source_view() || !self.source_idle() {
            return Ok(None);
        }
        let state = &mut self.history_sources;
        let query = if matches!(action, SourceAction::Discover) {
            Some(state.draft.query(&self.root)?)
        } else {
            None
        };
        let capture = if matches!(action, SourceAction::Capture) {
            Some(state.capture_request()?)
        } else {
            None
        };
        match &action {
            SourceAction::Discover if state.selection.is_none() => {
                return Err("history-source-select-history");
            }
            SourceAction::Path(path) if !state.available(path) => {
                return Err("history-source-select-file");
            }
            SourceAction::Replay
                if !state.ready
                    || !self
                        .plugins
                        .iter()
                        .any(|plugin| plugin.info.state == aftercast::PluginState::Initialized) =>
            {
                return Err("history-parser-required");
            }
            _ => {}
        }
        state.generation += 1;
        state.cancel = Arc::new(AtomicBool::new(false));
        let token = Token {
            session: self.revision.session,
            generation: state.generation,
        };
        state.pending = Some(token);
        state.error = None;
        let selection = match &action {
            SourceAction::History(selected) => Some((**selected).clone()),
            _ => state.selection.clone(),
        };
        if let SourceAction::Path(path) = &action {
            if state.choice_path.as_ref() != Some(path) {
                state.choices.clear();
                state.choice_path = Some(path.clone());
            }
            state.path = Some(path.clone());
            state.catalog = None;
            state.admitted = None;
            state.files.clear();
            state.progress = None;
            state.ready = false;
        }
        if let Some(snapshot) = &capture {
            state.admitted = Some(snapshot.clone());
            state.files.clear();
            state.ready = false;
            state.progress = None;
        }
        self.phase = if matches!(action, SourceAction::Replay) {
            self.cancel_import = false;
            self.progress = Default::default();
            Phase::Importing
        } else {
            Phase::ChangingSources
        };
        let replay = matches!(action, SourceAction::Replay);
        let work = Work {
            token,
            action,
            owned: state.owned.take().unwrap_or_else(|| Owned::new(host)),
            selection,
            query,
            capture,
            canceled: state.cancel.clone(),
        };
        if replay {
            self.invalidate_restored_context();
        }
        Ok(Some(work))
    }
    fn finish_source_work(
        &mut self,
        token: Token,
        action: &SourceAction,
        selection: Option<Selection>,
        mut output: Output,
    ) -> Option<Owned> {
        if self.revision.session != token.session || self.history_sources.pending != Some(token) {
            return Some(output.owned);
        }
        if matches!(action, SourceAction::Replay) {
            self.invalidate_restored_context();
        }
        let state = &mut self.history_sources;
        state.pending = None;
        self.phase = Phase::Ready;
        if state.generation != token.generation || output.canceled {
            state.error = output.error.clone();
            state.ready = false;
            state.snapshot = None;
            state.path = None;
            state.catalog = None;
            state.files.clear();
            state.progress = None;
        } else {
            state.error = output.error.clone();
            state.finish_action(action, selection, &mut output);
        }
        state.owned = output.owned.active().then_some(output.owned);
        None
    }
    fn leave_source_view(&mut self) -> bool {
        let pending = self.history_sources.pending.is_some();
        self.cancel_source_work();
        self.history_sources.invalidate();
        !pending && self.history_sources.holds_resources()
    }
    fn cancel_source_work(&mut self) {
        self.history_sources.cancel.store(true, Ordering::Release);
        if self.phase == Phase::Importing && self.history_sources.pending.is_some() {
            self.cancel_import = true;
        }
    }
}
async fn discover(
    work: &mut Work,
    owned: &mut Owned,
) -> Result<Option<HistorySourceSnapshot>, String> {
    owned.release().await?;
    let selected = work.selection.as_ref().ok_or("Missing history selection")?;
    owned.proposal = Some(
        owned
            .host
            .propose_history_sources(
                &selected.revision,
                &selected.entry.id,
                work.query.take().ok_or("Missing source scope")?,
            )
            .await
            .map_err(|error| error.to_string())?,
    );
    while !work.canceled.load(Ordering::Acquire) {
        let proposal = owned.proposal.as_mut().ok_or("Missing proposal")?;
        if owned
            .host
            .poll_history_sources(proposal)
            .await
            .map_err(|error| error.to_string())?
        {
            return Ok(proposal.snapshot().cloned());
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    Ok(None)
}
async fn capture_source(
    work: &Work,
    owned: &mut Owned,
    progress: &mut impl FnMut(HistoryCaptureProgress),
) -> Result<(), String> {
    owned.release_capture().await?;
    let request = work.capture.as_ref().ok_or("Missing capture request")?;
    let proposal = owned.proposal.as_ref().ok_or("Missing proposal")?;
    owned.capture = Some(
        owned
            .host
            .capture_history_source(proposal, &request.path, request.options.clone())
            .await
            .map_err(|error| error.to_string())?,
    );
    while !work.canceled.load(Ordering::Acquire) {
        let capture = owned.capture.as_mut().ok_or("Missing capture")?;
        let Some(update) = owned
            .host
            .poll_history_capture(capture)
            .await
            .map_err(|error| error.to_string())?
        else {
            if capture.range() != Some(request.range) {
                return Err("Captured time range changed".into());
            }
            return Ok(());
        };
        progress(update);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    Ok(())
}
async fn perform_action(
    work: &mut Work,
    output: &mut Output,
    progress: &mut impl FnMut(HistoryCaptureProgress),
) -> Result<(), String> {
    match &work.action {
        SourceAction::History(_) | SourceAction::Release => output.owned.release().await?,
        SourceAction::Discover => output.snapshot = discover(work, &mut output.owned).await?,
        SourceAction::Path(path) => {
            output.owned.release_capture().await?;
            output.catalog = Some(
                output
                    .owned
                    .host
                    .companion_candidates(path)
                    .await
                    .map_err(|error| error.to_string())?,
            );
        }
        SourceAction::Capture => capture_source(work, &mut output.owned, progress).await?,
        SourceAction::Replay => {
            let capture = output.owned.capture.as_mut().ok_or("Missing capture")?;
            output.job = Some(
                output
                    .owned
                    .host
                    .reparse_history_capture(capture)
                    .await
                    .map_err(|error| error.to_string())?,
            );
            output.owned.capture = None;
        }
    }
    Ok(())
}
async fn perform(mut work: Work, mut progress: impl FnMut(HistoryCaptureProgress)) -> Output {
    let host = work.owned.host.clone();
    let mut output = Output {
        owned: std::mem::replace(&mut work.owned, Owned::new(host)),
        snapshot: None,
        catalog: None,
        job: None,
        error: None,
        canceled: false,
    };
    output.error = perform_action(&mut work, &mut output, &mut progress)
        .await
        .err();
    if work.canceled.load(Ordering::Acquire) {
        output.canceled = true;
        if let Some(job) = &output.job {
            job.cancel();
        }
        if output.job.is_none()
            && let Err(error) = output.owned.release().await
        {
            output.error = Some(join_error(output.error, error));
        }
    }
    output
}
fn join_error(error: Option<String>, next: String) -> String {
    error.map_or_else(|| next.clone(), |previous| format!("{previous}\n{next}"))
}
