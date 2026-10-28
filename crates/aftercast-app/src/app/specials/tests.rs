use super::*;
use crate::app::log_window::{WindowAppearance, WindowLifetime};
use aftercast::{EncounterStatus, Host, HostConfig, PluginConfig, extract_bridge};
use camino::{Utf8Path, Utf8PathBuf};
use std::sync::Arc;

async fn fixture() -> (
    tempfile::TempDir,
    Utf8PathBuf,
    Host,
    ContextEndpoint,
    Arc<WindowLifetime>,
) {
    let temp = tempfile::tempdir().expect("temporary directory");
    let root = Utf8Path::from_path(temp.path()).expect("UTF8").to_owned();
    let bridge = root.join("bridge");
    extract_bridge(&bridge).expect("bridge");
    let executable = std::env::current_exe()
        .expect("test path")
        .parent()
        .and_then(std::path::Path::parent)
        .expect("target")
        .join("examples")
        .join(format!("test-application{}", std::env::consts::EXE_SUFFIX));
    let host = Host::start(
        HostConfig::builder()
            .dotnet_root(std::env::var("DOTNET_ROOT").expect("runtime"))
            .bridge_directory(bridge)
            .data_directory(root.join("state"))
            .application_executable(Utf8PathBuf::try_from(executable).expect("UTF8"))
            .build(),
    )
    .await
    .expect("host");
    let plugin = host.create_plugin(PluginConfig::builder()
        .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll"))
        .type_name("Aftercast.PluginFixture.AvoidanceImportPlugin").build()).await.expect("plugin");
    host.initialize_plugin(plugin).await.expect("initialize");
    let input = root.join("synthetic.log");
    std::fs::write(
        &input,
        "0|100|Target||None\n1|100|Target||Crit\n2|100|Target||Crit\n3|100|Target||None\n4|100|Target||Flurry\n",
    )
    .expect("synthetic input");
    host.import_log(&input).await.expect("import");
    let id = host
        .encounters(0, 1)
        .await
        .expect("encounters")
        .items
        .first()
        .expect("encounter")
        .id;
    let lifetime = Arc::new(WindowLifetime::default());
    let endpoint = ContextEndpoint {
        handle: host.handle(),
        id,
        generation: lifetime.generation(),
        lifetime: lifetime.clone(),
        appearance: WindowAppearance {
            title: "test".into(),
            theme: "test".into(),
            language: "en".into(),
        },
    };
    (temp, root, host, endpoint, lifetime)
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn specials_ui_contract_actual_host_snapshot_sources_and_lifecycle() {
    let (_temp, root, host, endpoint, lifetime) = fixture().await;
    let report = load(endpoint.clone(), "Source".into())
        .await
        .expect("load")
        .expect("combatant");
    let row = report
        .rows
        .iter()
        .find(|row| row.key == "Strike(Crit)")
        .expect("pair");
    assert_eq!(
        (row.occurrences, row.swings, row.total_swings),
        (Some(1), 2, Some(5))
    );
    let action = &row.evidence[0];
    let source = Some((
        usize::try_from(action.source_index).expect("source"),
        action.time_sorter,
    ));
    assert_eq!(source_text(&report, source), Some("1|100|Target||Crit"));
    assert_eq!(
        source_text(&report, Some((1, action.time_sorter + 1))),
        None
    );
    let replacement = load(endpoint.clone(), "Source".into())
        .await
        .expect("reload")
        .expect("combatant");
    assert_ne!(report.revision, replacement.revision);
    assert_eq!(source_text(&report, source), Some("1|100|Target||Crit"));
    let mut pending = Box::pin(load(endpoint.clone(), "Source".into()));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(std::future::Future::poll(pending.as_mut(), &mut context).is_pending());
    lifetime.invalidate();
    assert_eq!(pending.await.expect_err("late response"), "Session changed");
    let fresh = ContextEndpoint {
        generation: lifetime.generation(),
        ..endpoint
    };
    let mut cancelled = Box::pin(load(fresh.clone(), "Source".into()));
    assert!(std::future::Future::poll(cancelled.as_mut(), &mut context).is_pending());
    drop(cancelled);
    assert!(
        load(fresh.clone(), "Source".into())
            .await
            .expect("after cancel")
            .is_some()
    );
    let partial = root.join("partial.log");
    std::fs::write(
        &partial,
        "0|100|Target||None\n1|100|Target||Crit\n2|invalid|Target\n",
    )
    .expect("partial input");
    host.import_log(&partial).await.expect_err("invalid input");
    let rows = host.encounters(0, 16).await.expect("encounters");
    let id = rows
        .items
        .iter()
        .find(|item| item.status == EncounterStatus::PartialImport)
        .expect("partial")
        .id;
    let partial_endpoint = ContextEndpoint {
        id,
        ..fresh.clone()
    };
    let partial_report = load(partial_endpoint, "Source".into())
        .await
        .expect("partial load")
        .expect("combatant");
    assert_eq!(partial_report.summary.status, 3);
    lifetime.close();
    assert_eq!(
        load(fresh, "Source".into()).await.expect_err("closed"),
        "Session ended"
    );
    host.shutdown().await.expect("shutdown");
}
