use std::time::Duration;

use bon::Builder;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

pub(crate) use crate::local_runtime::canonicalize;

/// Trusted deployment paths and process lifetime limits. No executable is searched on PATH.
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
pub struct HostConfig {
    #[builder(into)]
    pub(crate) dotnet_root: Utf8PathBuf,
    #[builder(into)]
    pub(crate) bridge_directory: Utf8PathBuf,
    #[builder(into)]
    pub(crate) data_directory: Utf8PathBuf,
    /// Retain bounded encounter logs from the first parser callback. Default matches ACT: enabled.
    #[builder(default = true)]
    pub(crate) record_encounter_logs: bool,
    /// Keep original input rather than parser-processed text. Applies when recording is enabled.
    #[builder(default)]
    pub(crate) original_logline_view_logs: bool,
    /// End encounters after the configured interval measured in parsed log time.
    #[builder(default = true)]
    pub(crate) idle_end_enabled: bool,
    /// Enable ACT-style wall-clock idle ticks when idle ending is enabled.
    #[builder(default = true)]
    pub(crate) idle_timer_enabled: bool,
    /// Silence gap in seconds; zero disables segment cutting, otherwise 1–60.
    #[builder(default)]
    pub(crate) silence_cut_seconds: u8,
    /// ACT's idle threshold; valid range is 2–600 seconds.
    #[builder(default = 6)]
    pub(crate) idle_end_seconds: u16,
    /// Defaults to the current application executable. Override for an embedding test harness or launcher.
    #[builder(into)]
    pub(crate) application_executable: Option<Utf8PathBuf>,
    /// Includes worker launch, authentication and CLR initialization.
    #[builder(default = Duration::from_secs(30))]
    pub(crate) startup_timeout: Duration,
    /// Starts when shutdown is requested, even if the owner is inside a plugin callback.
    #[builder(default = Duration::from_secs(5))]
    pub shutdown_timeout: Duration,
}

#[repr(C)]
#[derive(Serialize, Deserialize)]
pub(crate) struct Packet {
    pub(crate) size: u32,
    pub(crate) version: u32,
    pub(crate) command: u32,
    pub(crate) runtime_major: u32,
    pub(crate) session: u64,
}
const _: () = assert!(size_of::<Packet>() == 24 && std::mem::offset_of!(Packet, session) == 16);

#[derive(Clone, Copy, Serialize, Deserialize)]
pub(crate) enum Request {
    ConfigureHost,
    CreatePlugin,
    PluginCommand,
    ImportOperation,
    Logs,
    Snapshot,
    Combat,
    Diagnostics,
    Speech,
    Archive,
    Triggers,
    Notifications,
    Timers,
    History,
    Tables,
    Companions,
    Reports,
    TableReports,
}
