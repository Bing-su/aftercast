mod app;
mod components;
mod config;
mod runtime_bundle;
mod session;
mod table_settings;

use std::path::PathBuf;

use camino::{Utf8Path, Utf8PathBuf};
use dioxus::desktop::{Config, LogicalSize, WindowBuilder, WindowCloseBehaviour};
use miette::{IntoDiagnostic, Result, miette};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

#[derive(Debug, usage_rs::Cli)]
#[usage(bin = "aftercast", version = "0.1.0")]
struct Cli {
    /// Internal mode used only by the authenticated Host supervisor.
    #[usage(long = "aftercast-child", hide)]
    aftercast_child: bool,
}

#[derive(Clone)]
pub struct ApplicationRoot(pub Utf8PathBuf);

fn open_log(root: &Utf8Path) -> Result<tracing_appender::rolling::RollingFileAppender> {
    let directory = root.join("logs");
    std::fs::create_dir_all(&directory).into_diagnostic()?;
    tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("aftercast")
        .max_log_files(7)
        .build(directory)
        .into_diagnostic()
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.aftercast_child {
        if !aftercast::run_child_if_requested().into_diagnostic()? {
            return Err(miette!(
                "Invalid child invocation: --aftercast-child must be the first argument"
            ));
        }
        return Ok(());
    }
    let executable: PathBuf = std::env::current_exe().into_diagnostic()?;
    let root = Utf8PathBuf::from_path_buf(executable)
        .map_err(|path| miette!("Executable path is not UTF-8: {}", path.display()))?
        .parent()
        .ok_or_else(|| miette!("Executable has no parent directory"))?
        .to_owned();
    let file = open_log(&root)?;
    let (writer, guard) = tracing_appender::non_blocking(file);
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(writer),
        )
        .try_init()
        .into_diagnostic()?;
    tracing::info!(path = %root, "starting desktop application");
    dioxus::LaunchBuilder::desktop()
        .with_context(ApplicationRoot(root))
        .with_cfg(
            Config::new()
                .with_menu(None)
                // Use an empty icon because this shell has no branding, e.g. avoid a missing favicon request.
                .with_custom_head("<link rel=\"icon\" href=\"data:,\">".into())
                .with_close_behaviour(WindowCloseBehaviour::WindowHides)
                .with_window(
                    WindowBuilder::new()
                        .with_title("aftercast")
                        .with_inner_size(LogicalSize::new(1280.0, 820.0))
                        .with_min_inner_size(LogicalSize::new(850.0, 580.0)),
                ),
        )
        .launch(app::App);
    // Dioxus returns after its UI executor has ended, so flushing cannot freeze a live window.
    drop(guard);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_start_prepares_log_directory_and_reports_creation_failure() {
        let root = tempfile::tempdir().expect("temporary root");
        let path = Utf8PathBuf::from_path_buf(root.path().to_owned()).expect("UTF-8 temp path");
        let _writer = open_log(&path).expect("first start");
        assert!(path.join("logs").is_dir());
        let blocked = tempfile::tempdir().expect("blocked root");
        let blocked_path =
            Utf8PathBuf::from_path_buf(blocked.path().to_owned()).expect("UTF-8 temp path");
        std::fs::write(blocked_path.join("logs"), b"fixture").expect("synthetic obstacle");
        open_log(&blocked_path).expect_err("log directory failure");
    }

    #[test]
    fn common_cli_accepts_child_and_rejects_unknown_arguments() {
        let child = [std::ffi::OsStr::new("--aftercast-child")];
        assert!(Cli::parse_from(&child).expect("child flag").aftercast_child);
        assert!(!Cli::parse_from(&[]).expect("GUI default").aftercast_child);
        Cli::parse_from(&[std::ffi::OsStr::new("--unknown")]).expect_err("unknown argument");
    }
}
