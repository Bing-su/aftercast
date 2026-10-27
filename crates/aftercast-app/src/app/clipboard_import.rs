use super::{AppState, Phase};
use aftercast::{ImportReport, MAX_CLIPBOARD_IMPORT_BYTES};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
mod view;
pub(super) use view::Controls;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    Empty,
    TooLarge,
    Unsupported,
    Unavailable,
    Busy,
    Import,
}
impl Failure {
    fn key(self) -> &'static str {
        match self {
            Self::Empty => "clipboard-empty",
            Self::TooLarge => "clipboard-too-large",
            Self::Unsupported => "clipboard-unsupported",
            Self::Unavailable => "clipboard-unavailable",
            Self::Busy => "clipboard-busy",
            Self::Import => "clipboard-import-failed",
        }
    }
}
// A supplied text provider only. No OS clipboard reader or writer is implemented in this module.
trait Reader: Send + Sync {
    fn read_text(&self) -> Result<Option<String>, ()>;
}
struct UnavailableReader;
impl Reader for UnavailableReader {
    fn read_text(&self) -> Result<Option<String>, ()> {
        Ok(None)
    }
}
struct AdapterInner {
    reader: Box<dyn Reader>,
    busy: AtomicBool,
    available: bool,
}
#[derive(Clone)]
pub(super) struct Adapter(Arc<AdapterInner>);
impl Default for Adapter {
    fn default() -> Self {
        Self::new(UnavailableReader, false)
    }
}
struct ReadLease(Arc<AdapterInner>);
impl Drop for ReadLease {
    fn drop(&mut self) {
        self.0.busy.store(false, Ordering::Release);
    }
}
impl Adapter {
    fn new(reader: impl Reader + 'static, available: bool) -> Self {
        Self(Arc::new(AdapterInner {
            reader: Box::new(reader),
            busy: AtomicBool::new(false),
            available,
        }))
    }
    fn busy(&self) -> bool {
        self.0.busy.load(Ordering::Acquire)
    }
    async fn read(&self) -> Result<Snapshot, Failure> {
        if !self.0.available {
            return Err(Failure::Unavailable);
        }
        self.0
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_busy| Failure::Busy)?;
        let lease = ReadLease(self.0.clone());
        // One supplied read at a time, including across Host restarts; provider errors/content stay private.
        tokio::task::spawn_blocking(move || {
            let result = lease
                .0
                .reader
                .read_text()
                .map_err(|()| Failure::Unavailable)
                .and_then(|text| text.ok_or(Failure::Unsupported))
                .and_then(Snapshot::new);
            drop(lease);
            result
        })
        .await
        .map_err(|_join| Failure::Unavailable)?
    }
}
#[derive(Clone)]
struct Snapshot {
    text: Arc<str>,
    preview: String,
    characters: usize,
    viewpoint: Option<aftercast::ImportCharacter>,
}
impl Snapshot {
    async fn start(
        &self,
        host: &aftercast::HostHandle,
    ) -> Result<aftercast::ImportJob, aftercast::Error> {
        match &self.viewpoint {
            Some(choice) => {
                host.start_clipboard_import_with_viewpoint(self.text.as_ref(), choice.clone())
                    .await
            }
            None => host.start_clipboard_import(self.text.as_ref()).await,
        }
    }
    fn new(text: String) -> Result<Self, Failure> {
        if text.is_empty() {
            return Err(Failure::Empty);
        }
        if text.len() > MAX_CLIPBOARD_IMPORT_BYTES {
            return Err(Failure::TooLarge);
        }
        let preview = text.chars().take(512).collect();
        let characters = text.chars().count();
        Ok(Self {
            text: text.into(),
            preview,
            characters,
            viewpoint: None,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Token {
    session: u64,
    generation: u64,
}
#[derive(Clone)]
enum Pending {
    Read(Token, Phase),
    Import(Token, Snapshot),
}
#[derive(Default)]
pub(super) struct State {
    draft: Option<Snapshot>,
    pending: Option<Pending>,
    generation: u64,
    error: Option<Failure>,
}
impl State {
    pub(super) fn viewpoint_snapshot(&self) -> Option<Option<aftercast::ImportCharacter>> {
        match &self.pending {
            Some(Pending::Import(_, snapshot)) => Some(snapshot.viewpoint.clone()),
            _ => None,
        }
    }
    pub(super) fn active(&self) -> bool {
        self.pending.is_some()
    }
    pub(super) fn importing(&self) -> bool {
        matches!(self.pending, Some(Pending::Import(..)))
    }
    pub(super) fn reset_session(&mut self) {
        self.generation += 1;
        self.pending = None;
        self.error = None;
    }
}
impl AppState {
    fn clipboard_view(&self) -> Token {
        Token {
            session: self.revision.session,
            generation: self.clipboard.generation,
        }
    }
    fn can_edit_clipboard(&self) -> bool {
        matches!(self.phase, Phase::Idle | Phase::Ready)
            && !self.loading
            && !self.exit_requested
            && !self.clipboard.active()
            && !self.import_text.active()
            && !self.reparse.active()
            && !self.history_sources.holds_resources()
    }
    fn begin_clipboard_read(&mut self, view: Token) -> Option<Token> {
        if self.clipboard_view() != view || !self.can_edit_clipboard() {
            return None;
        }
        self.clipboard.generation += 1;
        self.import_text.invalidate_view();
        let token = self.clipboard_view();
        self.clipboard.pending = Some(Pending::Read(token, self.phase));
        self.clipboard.error = None;
        self.phase = Phase::ReadingClipboard;
        Some(token)
    }
    fn receive_clipboard_read(&mut self, token: Token, result: Result<Snapshot, Failure>) -> bool {
        let Some(Pending::Read(current, previous)) = self.clipboard.pending else {
            return false;
        };
        if token != current
            || self.clipboard_view() != token
            || self.phase != Phase::ReadingClipboard
        {
            return false;
        }
        self.clipboard.pending = None;
        self.clipboard.generation += 1;
        self.import_text.invalidate_view();
        self.phase = previous;
        match result {
            Ok(snapshot) => {
                self.clipboard.draft = Some(snapshot);
                self.clipboard.error = None;
            }
            Err(error) => self.clipboard.error = Some(error),
        }
        true
    }
    pub(super) fn abandon_clipboard_read_for_exit(&mut self) -> bool {
        if !self.exit_requested || self.phase != Phase::ReadingClipboard {
            return false;
        }
        let Some(Pending::Read(_, previous)) = self.clipboard.pending else {
            return false;
        };
        // Returning to Ready lets normal Host Stop run; returning to Idle lets the window close.
        self.clipboard.reset_session();
        self.phase = previous;
        true
    }
    fn clear_clipboard_preview(&mut self, view: Token) -> bool {
        if self.clipboard_view() != view || !self.can_edit_clipboard() {
            return false;
        }
        self.clipboard.draft = None;
        self.clipboard.error = None;
        self.clipboard.generation += 1;
        self.import_text.invalidate_view();
        true
    }
    fn can_import_clipboard(&self) -> bool {
        self.can_edit_clipboard()
            && self.phase == Phase::Ready
            && self.clipboard.draft.is_some()
            && self.import_text.viewpoint().is_ok()
            && self
                .plugins
                .iter()
                .any(|plugin| plugin.info.state == aftercast::PluginState::Initialized)
    }
    fn begin_clipboard_import(&mut self, view: Token) -> Option<(Token, Snapshot)> {
        if self.clipboard_view() != view || !self.can_import_clipboard() {
            return None;
        }
        let mut snapshot = self.clipboard.draft.clone()?;
        snapshot.viewpoint = self.import_text.viewpoint().ok()?;
        self.clipboard.generation += 1;
        self.import_text.invalidate_view();
        let token = self.clipboard_view();
        self.clipboard.pending = Some(Pending::Import(token, snapshot.clone()));
        self.clipboard.error = None;
        self.phase = Phase::Importing;
        self.cancel_import = false;
        self.progress = Default::default();
        self.error = None;
        self.message.clear();
        self.invalidate_restored_context();
        Some((token, snapshot))
    }
    fn cancel_clipboard_import(&mut self, token: Token) -> bool {
        if self.clipboard_view() != token
            || !matches!(self.clipboard.pending, Some(Pending::Import(current, _)) if current == token)
            || self.cancel_import
        {
            return false;
        }
        self.cancel_import = true;
        true
    }
    fn receive_clipboard_import(
        &mut self,
        token: Token,
        result: Result<ImportReport, aftercast::Error>,
        complete: &str,
        canceled: &str,
    ) -> bool {
        if self.clipboard_view() != token
            || !matches!(self.clipboard.pending, Some(Pending::Import(current, _)) if current == token)
        {
            return false;
        }
        self.clipboard.pending = None;
        self.clipboard.generation += 1;
        self.import_text.invalidate_view();
        self.phase = Phase::Ready;
        self.invalidate_restored_context();
        match result {
            Ok(report) => {
                self.message = format!(
                    "{complete} · {} / {} · {}",
                    report.records_dispatched, report.lines_read, report.encounters
                )
            }
            Err(aftercast::Error::Cancelled) => self.message = canceled.into(),
            // The general failure logger can include parser input. Keep the clipboard error local/redacted.
            Err(_) => self.clipboard.error = Some(Failure::Import),
        }
        true
    }
}
