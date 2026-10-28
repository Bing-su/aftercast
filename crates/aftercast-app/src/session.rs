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
        let deploy_root = root.clone();
        let recording = (
            settings.host.record_encounter_logs,
            settings.host.original_logline_view_logs,
            settings.host.idle_end_enabled,
            settings.host.idle_end_seconds,
            settings.host.idle_timer_enabled,
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
                .silence_cut_seconds(recording.5)
                .build(),
        )
        .await
        .into_diagnostic()?;
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
