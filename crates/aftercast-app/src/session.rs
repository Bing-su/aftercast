use aftercast::{Host, HostConfig};
use camino::{Utf8Path, Utf8PathBuf};
use miette::{Context, IntoDiagnostic, Result};

use crate::config::{Settings, resolve};

pub struct RunningHost {
    pub host: Host,
    deployment: tempfile::TempDir,
}

impl RunningHost {
    pub async fn start(root: Utf8PathBuf, settings: Settings) -> Result<Self> {
        settings.validate()?;
        let audio = settings.audio.settings();
        let colors = settings.appearance.color_scalars();
        let color_controls = settings.appearance.color_controls();
        let deploy_root = root.clone();
        let selective = settings.host.selective_options();
        let recording = (
            settings.host.record_encounter_logs,
            settings.host.original_logline_view_logs,
            settings.host.idle_end_enabled,
            settings.host.idle_end_seconds,
            settings.host.idle_timer_enabled,
            settings.host.main_table_commas,
            settings.host.count_zero_damage_as_hit,
            if settings.host.silence_cut_enabled {
                settings.host.silence_cut_seconds
            } else {
                0
            },
        );
        let (deployment, dotnet_root) = tokio::task::spawn_blocking(move || {
            let runtime = if settings.host.dotnet_root == "dotnet" {
                crate::runtime_bundle::ensure(&deploy_root)?
            } else {
                resolve(&deploy_root, &settings.host.dotnet_root)
            };
            Ok::<_, miette::Report>((prepare(&deploy_root)?, runtime))
        })
        .await
        .into_diagnostic()??;
        let bridge = Utf8Path::from_path(deployment.path())
            .ok_or_else(|| miette::miette!("Bridge path is not UTF-8"))?
            .join("bridge");
        let host = Host::start(
            HostConfig::builder()
                .dotnet_root(dotnet_root)
                .bridge_directory(bridge)
                .data_directory(root.join("state"))
                .record_encounter_logs(recording.0)
                .original_logline_view_logs(recording.1)
                .idle_end_enabled(recording.2)
                .idle_end_seconds(recording.3)
                .idle_timer_enabled(recording.4)
                .main_table_commas(recording.5)
                .count_zero_damage_as_hit(recording.6)
                .selective(selective)
                .silence_cut_seconds(recording.7)
                .build(),
        )
        .await
        .into_diagnostic()?;
        if let Err(error) = host.set_audio_settings(&audio).await {
            if let Err(shutdown_error) = host.shutdown().await {
                tracing::warn!(error = %shutdown_error, "Failed to stop host after audio setup error");
            }
            return Err(error).into_diagnostic();
        }
        if let Err(error) = host.set_color_scalars(colors).await {
            if let Err(shutdown_error) = host.shutdown().await {
                tracing::warn!(error = %shutdown_error, "Failed to stop host after color setup error");
            }
            return Err(error).into_diagnostic();
        }
        if let Err(error) = host.set_color_controls(color_controls).await {
            if let Err(shutdown_error) = host.shutdown().await {
                tracing::warn!(error = %shutdown_error, "Failed to stop host after color control setup error");
            }
            return Err(error).into_diagnostic();
        }
        Ok(Self { host, deployment })
    }

    pub async fn shutdown(self) -> Result<()> {
        let Self {
            host, deployment, ..
        } = self;
        let result = host.shutdown().await.into_diagnostic();
        let cleanup = tokio::task::spawn_blocking(move || deployment.close())
            .await
            .into_diagnostic()?
            .into_diagnostic();
        result.and(cleanup)
    }
}

fn prepare(root: &Utf8Path) -> Result<tempfile::TempDir> {
    // A fresh private bridge prevents stale payload reuse; persistent ACT state stays beside the executable.
    let deployment = tempfile::Builder::new()
        .prefix(".aftercast-bridge-")
        .tempdir_in(root)
        .into_diagnostic()?;
    let bridge = Utf8Path::from_path(deployment.path())
        .ok_or_else(|| miette::miette!("Bridge path is not UTF-8"))?
        .join("bridge");
    aftercast::extract_bridge(&bridge)
        .into_diagnostic()
        .wrap_err("Deploy embedded managed bridge")?;
    Ok(deployment)
}
