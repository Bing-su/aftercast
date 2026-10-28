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
        .type_name("Aftercast.PluginFixture.ImportPlugin").build()).await.expect("plugin");
    host.initialize_plugin(plugin).await.expect("initialize");
    let input = root.join("synthetic.log");
    std::fs::write(&input, "0|100\n0|-10\n1|-10\n").expect("synthetic input");
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
async fn death_ui_contract_actual_host_selection_cancel_late_response_and_source_identity() {
    let (_temp, root, host, endpoint, lifetime) = fixture().await;
    let analysis = load_analysis(endpoint.clone(), "Target".into())
        .await
        .expect("analysis")
        .expect("combatant");
    assert_eq!(analysis.deaths.len(), 2);
    assert_eq!(analysis.dropped_logs, 0);
    let mut selection = DeathSelection::default();
    assert!(selection.select(0, analysis.deaths.len()));
    let death = &analysis
        .actions
        .get(*analysis.deaths.first().expect("death"))
        .expect("action")
        .action;
    let SourceLogLink::Retained(offset) = death.source else {
        panic!("source not retained")
    };
    selection.select_source(Some((offset, death.time_sorter)));
    assert_eq!(
        load_source(
            endpoint.clone(),
            analysis.snapshot,
            offset,
            death.time_sorter
        )
        .await
        .expect("source"),
        "0|-10"
    );
    load_source(
        endpoint.clone(),
        analysis.snapshot,
        offset,
        death.time_sorter + 1,
    )
    .await
    .expect_err("wrong identity");
    selection.cancel_source();
    assert_eq!(selection.source, None);
    selection.select_source(Some((offset, death.time_sorter)));
    assert!(selection.select(1, analysis.deaths.len()));
    assert_eq!(selection.source, None);
    assert!(!selection.select(2, analysis.deaths.len()));
    assert_eq!(selection.selected, Some(1));
    assert!(incomplete(EncounterStatus::PartialImport));
    assert!(incomplete(EncounterStatus::Importing));
    assert!(!incomplete(EncounterStatus::Ended));
    let mut future = Box::pin(load_analysis(endpoint.clone(), "Target".into()));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(std::future::Future::poll(future.as_mut(), &mut context).is_pending());
    let mut late_source = Box::pin(load_source(
        endpoint.clone(),
        analysis.snapshot,
        offset,
        death.time_sorter,
    ));
    assert!(std::future::Future::poll(late_source.as_mut(), &mut context).is_pending());
    lifetime.invalidate();
    assert_eq!(future.await.expect_err("late response"), "Session changed");
    assert_eq!(
        late_source.await.expect_err("late source"),
        "Session changed"
    );
    load_source(
        endpoint.clone(),
        analysis.snapshot,
        offset,
        death.time_sorter,
    )
    .await
    .expect_err("stale session");
    let fresh = ContextEndpoint {
        generation: lifetime.generation(),
        ..endpoint
    };
    let mut cancelled = Box::pin(load_analysis(fresh.clone(), "Target".into()));
    assert!(std::future::Future::poll(cancelled.as_mut(), &mut context).is_pending());
    drop(cancelled);
    load_analysis(fresh.clone(), "Target".into())
        .await
        .expect("after cancel")
        .expect("combatant");
    let partial = root.join("partial.log");
    std::fs::write(&partial, "0|100\n0|-10\n0|invalid\n").expect("partial input");
    host.import_log(&partial).await.expect_err("invalid input");
    let partial_rows = host.encounters(0, 16).await.expect("partial encounters");
    let partial_encounter = partial_rows
        .items
        .iter()
        .find(|item| item.status == EncounterStatus::PartialImport)
        .expect("partial import status");
    assert!(incomplete(partial_encounter.status));
    lifetime.close();
    load_analysis(fresh, "Target".into())
        .await
        .expect_err("closed session");
    host.shutdown().await.expect("shutdown");
}
