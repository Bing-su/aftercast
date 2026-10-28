//! Process-isolated ACT-compatible hosting.
//!
//! Provides asynchronous host/plugin lifecycles, cancellable imports, log batches and owned combat results.
//! Call [`extract_bridge`] once to deploy the bundled, prebuilt C# component, then use
//! [`Host::start`] with a trusted .NET 10 installation. Call [`run_child_if_requested`]
//! first in the application's main function to support isolated child execution.
//! No SDK or download is used at runtime.

mod archives;
mod bridge;
pub use archives::{ActExportOptions, ActFormat};
mod avoidance_report;
mod combat;
mod companions;
mod death;
pub use companions::{
    CompanionCandidate, CompanionCatalog, CompanionSource, LiveLogOptions, LiveLogSelection,
    LiveLogSnapshot, LiveLogSourceStatus, LiveLogState, SourceImportOptions,
};
mod diagnostics;
mod history;
mod history_sources;
mod host;
pub use history::{
    HistoryEntry, HistoryId, HistoryPage, HistoryQuery, HistoryRevision, HistorySearchScope,
};
pub use history_sources::{
    CapturedHistoryFile, HistoryCaptureOptions, HistoryCaptureProgress, HistorySourceCandidate,
    HistorySourceCapture, HistorySourceChange, HistorySourceDiagnostic, HistorySourceProposal,
    HistorySourceQuery, HistorySourceSnapshot, HistorySourceState,
};
mod import;
mod local_runtime;
mod logs;
mod notifications;
pub use notifications::{
    Notification, NotificationChange, NotificationCursor, NotificationId, NotificationPage,
    NotificationStatus,
};
mod plugins;
mod process;
mod runtime;
pub use process::{run_child_if_requested, run_child_if_requested_with_audio};
mod hierarchy_html;
pub use hierarchy_html::{
    HierarchyHtmlGraphOptions, HierarchyHtmlLevel, HierarchyHtmlReport, HierarchyHtmlSelection,
};
mod hierarchy_xml;
mod reports;
pub use hierarchy_xml::{HierarchyXmlDepth, HierarchyXmlReport, HierarchyXmlSelection};
mod snapshots;
mod speech;
mod table_reports;
mod tables;
pub use reports::{
    TextReport, TextReportCatalog, TextReportOptions, TextReportPreset, TextReportRevision,
};
pub use table_reports::{
    TableReport, TableReportCatalog, TableReportFormat, TableReportOptions, TableReportSelection,
};
mod timers;
pub use tables::{
    TableCatalog, TableCategory, TableCell, TableCellError, TableColumn, TableLevel, TablePage,
    TableQuery, TableRevision, TableRow, TableScope, TableSort, TableSortError,
};
mod triggers;
pub use speech::AudioSettings;
pub use timers::{SavedTimer, TimerDefinition, TimerItem, TimerPage, TimerRevision};
pub use triggers::TriggerResultCursor;
pub use triggers::{
    TriggerCapture, TriggerDefinition, TriggerItem, TriggerPage, TriggerResult, TriggerResultPage,
    TriggerRevision,
};

pub use avoidance_report::{
    AvoidanceAction, AvoidanceCategory, AvoidanceEvidence, AvoidanceLog, AvoidanceNumber,
    AvoidanceOutcome, AvoidanceReport, AvoidanceSummary,
};
pub use bridge::extract_bridge;
use camino::Utf8PathBuf;
pub use combat::{
    ActionSnapshot, AttackSummary, ClassifiedAction, EncounterAction, EncounterDetail, EncounterId,
    EncounterLog, EncounterLogs, EncounterPage, EncounterStatus, EncounterSummary, SourceLogLink,
};
pub use death::{DeathAnalysis, DeathBucket, DeathReport, death_report, sort_incoming_actions};
pub use diagnostics::HostDiagnostics;
pub use host::{Host, HostHandle};
pub use import::{
    ImportCharacter, ImportEncoding, ImportInvalidBytes, ImportJob, ImportProgress, ImportRange,
    ImportReport, ImportTextOptions, MAX_CLIPBOARD_IMPORT_BYTES,
};
pub use logs::{
    LogBatch, LogLoss, LogRecord, LogSubscription, LogSubscriptionConfig, LogTimeKind, LogTimestamp,
};
pub use plugins::{PluginConfig, PluginId, PluginSettings, PluginState, PluginStatus};
pub use runtime::HostConfig;
pub use snapshots::{CombatRevision, CombatSnapshot, CombatSnapshotStatus, CombatantSnapshot};

/// Failures that callers can distinguish without parsing diagnostic messages.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("invalid text report request or response: {0}")]
    InvalidReportRequest(String),
    #[error("invalid companion request or response: {0}")]
    InvalidCompanionRequest(String),
    #[error("invalid table request or response: {0}")]
    InvalidTableRequest(String),
    #[error("invalid history request or response: {0}")]
    InvalidHistoryRequest(String),
    #[error("invalid timer request or response: {0}")]
    InvalidTimerRequest(String),
    #[error("invalid notification request or response: {0}")]
    InvalidNotificationRequest(String),
    #[error("invalid trigger request or response: {0}")]
    InvalidTriggerRequest(String),
    #[error("the host command queue is full or a conflicting operation is active")]
    Busy,
    #[error("import canceled after retaining partial history and restoring live state")]
    Cancelled,
    #[error("the host is closed")]
    Closed,
    #[error("host worker failed: {0}")]
    WorkerFailed(String),
    #[error(
        "host process exceeded its graceful shutdown deadline of {0:?}; it was killed and reaped"
    )]
    ShutdownTimeout(std::time::Duration),
    #[error("host process exceeded its startup deadline of {0:?}; it was killed and reaped")]
    StartupTimeout(std::time::Duration),
    #[error("invalid archive request: {0}")]
    InvalidArchiveRequest(String),
    #[error("invalid archive response: {0}")]
    InvalidArchiveResponse(String),
    #[error("invalid combat query: {0}")]
    InvalidCombatRequest(String),
    #[error("invalid managed combat query response: {0}")]
    InvalidCombatResponse(String),
    #[error("text-to-speech failed: {0}")]
    Speech(#[from] aftercast_tts::Error),
    #[error("sound playback failed: {0}")]
    Sound(#[source] aftercast_tts::Error),
    #[error("invalid managed speech response: {0}")]
    InvalidSpeechResponse(String),
    #[error("invalid combat snapshot request: {0}")]
    InvalidSnapshotRequest(String),
    #[error("invalid managed combat snapshot: {0}")]
    InvalidSnapshotResponse(String),
    #[error("invalid log subscription request: {0}")]
    InvalidLogRequest(String),
    #[error("invalid managed log batch: {0}")]
    InvalidLogResponse(String),
    #[error("invalid import request: {0}")]
    InvalidImportRequest(String),
    #[error("invalid managed import response: {0}")]
    InvalidImportResponse(String),
    #[error("invalid managed diagnostic response: {0}")]
    InvalidDiagnosticResponse(String),
    #[error("invalid plugin request: {0}")]
    InvalidPluginRequest(String),
    #[error("I/O operation failed for {path}: {source}")]
    Io {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("path contains an embedded NUL: {path}: {source}")]
    InvalidPath {
        path: Utf8PathBuf,
        #[source]
        source: netcorehost::pdcstring::ContainsNul,
    },
    #[error("no stable .NET 10 hostfxr was found under {0}")]
    RuntimeNotFound(Utf8PathBuf),
    #[error("cannot load hostfxr: {0}")]
    Hostfxr(#[from] netcorehost::dlopen2::Error),
    #[error(".NET hosting failed: {0}")]
    Hosting(#[from] netcorehost::error::HostingError),
    #[error("cannot load the managed entry point: {0}")]
    EntryPoint(#[from] netcorehost::hostfxr::GetManagedFunctionError),
    #[error("an ACT host is already active in this process")]
    AlreadyActive,
    #[error("the ACT state directory is already owned by another worker: {0}")]
    StateInUse(Utf8PathBuf),
    #[error("managed bridge command {command} failed with status {status}: {message}")]
    Managed {
        command: u32,
        status: i32,
        message: String,
    },
    #[error("expected .NET 10, but the managed bridge reports major version {0}")]
    RuntimeMismatch(u32),
    #[error(
        "hostfxr is already loaded from {loaded}; cannot switch to {requested} in this process"
    )]
    RuntimeAlreadyLoaded {
        loaded: Utf8PathBuf,
        requested: Utf8PathBuf,
    },
}

fn io_error(path: impl Into<Utf8PathBuf>, source: std::io::Error) -> Error {
    Error::Io {
        path: path.into(),
        source,
    }
}

mod specials_report;
pub use specials_report::{SpecialReport, SpecialRow};

mod timeline;
pub use timeline::{TimelineLog, TimelineOptions, TimelineReport, TimelineRow, TimelineSeries};
