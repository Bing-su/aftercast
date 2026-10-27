use aftercast::{ImportCharacter, ImportProgress, ImportReport};
use std::sync::{Mutex, MutexGuard};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) struct Token {
    pub session: u64,
    pub serial: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Kind {
    File,
    Range,
    Text,
    Saved,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) struct Context {
    pub kind: Kind,
    pub viewpoint: Option<ImportCharacter>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum Outcome {
    Running,
    Cancelling,
    Complete,
    Cancelled,
    Failed,
    Retired,
}
impl Outcome {
    pub fn pending(self) -> bool {
        matches!(self, Self::Running | Self::Cancelling)
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Running => "import-progress-running",
            Self::Cancelling => "import-progress-cancelling",
            Self::Complete => "import-progress-complete",
            Self::Cancelled => "import-progress-cancelled",
            Self::Failed => "import-progress-failed",
            Self::Retired => "import-progress-retired",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) struct Snapshot {
    pub token: Token,
    pub context: Context,
    pub progress: ImportProgress,
    pub outcome: Outcome,
    pub report: Option<ImportReport>,
}
#[derive(Default)]
struct Inner {
    next: u64,
    current: Option<Snapshot>,
    closed: bool,
}
// Plain bounded data only: the desktop progress window has its own VirtualDom, never the main Signal.
#[derive(Default)]
pub(in crate::app) struct Hub(Mutex<Inner>);
impl Hub {
    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    pub fn snapshot(&self) -> Option<Snapshot> {
        self.inner().current.clone()
    }
    pub fn closed(&self) -> bool {
        self.inner().closed
    }
    pub fn begin(&self, session: u64, context: Context, progress: ImportProgress) -> Option<Token> {
        let mut inner = self.inner();
        if inner.closed {
            return None;
        }
        inner.next = inner.next.checked_add(1)?;
        let token = Token {
            session,
            serial: inner.next,
        };
        inner.current = Some(Snapshot {
            token,
            context,
            progress,
            outcome: Outcome::Running,
            report: None,
        });
        Some(token)
    }
    pub fn publish(&self, token: Token, progress: ImportProgress) -> bool {
        let mut inner = self.inner();
        let Some(current) = inner
            .current
            .as_mut()
            .filter(|frame| frame.token == token && frame.outcome.pending())
        else {
            return false;
        };
        current.progress = progress;
        true
    }
    pub fn cancel(&self, token: Token) -> bool {
        let mut inner = self.inner();
        let Some(current) = inner
            .current
            .as_mut()
            .filter(|frame| frame.token == token && frame.outcome == Outcome::Running)
        else {
            return false;
        };
        current.outcome = Outcome::Cancelling;
        true
    }
    pub fn should_cancel(&self, token: Token) -> bool {
        let inner = self.inner();
        inner.closed
            || inner.current.as_ref().is_none_or(|frame| {
                frame.token != token
                    || matches!(frame.outcome, Outcome::Cancelling | Outcome::Retired)
            })
    }
    pub fn finish(&self, token: Token, result: &Result<ImportReport, aftercast::Error>) -> bool {
        let mut inner = self.inner();
        let Some(current) = inner
            .current
            .as_mut()
            .filter(|frame| frame.token == token && frame.outcome.pending())
        else {
            return false;
        };
        current.outcome = match result {
            Ok(_) => Outcome::Complete,
            Err(aftercast::Error::Cancelled) => Outcome::Cancelled,
            Err(_) => Outcome::Failed,
        };
        current.report = result.as_ref().ok().cloned();
        true
    }
    fn close_answer(&self, expected: Token, yes: bool) -> CloseAction {
        let mut inner = self.inner();
        if inner.closed {
            return CloseAction::Ignored;
        }
        let Some(current) = inner
            .current
            .as_mut()
            .filter(|frame| frame.token == expected)
        else {
            return CloseAction::Ignored;
        };
        if !yes {
            return CloseAction::Keep;
        }
        if current.outcome == Outcome::Running {
            current.outcome = Outcome::Cancelling;
        }
        CloseAction::Hide
    }
    pub fn retire(&self) {
        if let Some(frame) = &mut self.inner().current {
            frame.outcome = Outcome::Retired;
            frame.report = None;
        }
    }
    pub fn close(&self) {
        let mut inner = self.inner();
        inner.closed = true;
        if let Some(frame) = &mut inner.current {
            frame.outcome = Outcome::Retired;
            frame.report = None;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CloseAction {
    Ignored,
    Keep,
    Confirm,
    Hide,
}
#[derive(Default)]
pub(super) struct CloseState {
    pub confirmation: Option<Token>,
}
impl CloseState {
    pub fn request(&mut self, frame: Option<&Snapshot>) -> CloseAction {
        self.confirmation = frame
            .filter(|frame| frame.outcome.pending())
            .map(|frame| frame.token);
        if self.confirmation.is_some() {
            CloseAction::Confirm
        } else {
            CloseAction::Hide
        }
    }
    pub fn answer(&mut self, hub: &Hub, expected: Token, yes: bool) -> CloseAction {
        if self.confirmation != Some(expected) {
            return CloseAction::Ignored;
        }
        self.confirmation = None;
        hub.close_answer(expected, yes)
    }
    pub fn sync(&mut self, frame: Option<&Snapshot>) {
        if self
            .confirmation
            .is_some_and(|token| frame.is_none_or(|frame| frame.token != token))
        {
            self.confirmation = None;
        }
    }
}
