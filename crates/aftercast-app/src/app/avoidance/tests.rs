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
        "0|100|Target\n0|-1|Target|Source and YOU\n1|-2|Target|YOU only\n",
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
async fn avoidance_ui_contract_actual_host_snapshot_selection_and_lifecycle() {
    let (_temp, root, host, endpoint, lifetime) = fixture().await;
    host.encounter_detail(endpoint.id)
        .await
        .expect("detail")
        .expect("encounter");
    let report = load(endpoint.clone(), Scope::Combatant("Target".into()))
        .await
        .expect("load")
        .expect("combatant");
    let category = report
        .categories
        .iter()
        .find(|item| item.name == "Damage")
        .expect("category");
    let outcome = category
        .outcomes
        .iter()
        .find(|item| item.label == "Source and YOU")
        .expect("outcome");
    let evidence = &outcome.evidence[0];
    let mut selection = Selection::default();
    selection.select(&evidence.action);
    assert_eq!(
        source_text(&report, selection.source),
        Some("0|-1|Target|Source and YOU")
    );
    selection.select(&evidence.samples[0]);
    assert_eq!(source_text(&report, selection.source), Some("0|100|Target"));
    let (index, sorter) = selection.source.expect("source");
    assert_eq!(source_text(&report, Some((index, sorter + 1))), None);
    selection.clear();
    assert_eq!(selection.source, None);
    let whole = load(endpoint.clone(), Scope::Encounter)
        .await
        .expect("encounter load")
        .expect("encounter");
    for name in ["Source", "YOU"] {
        assert!(
            whole
                .categories
                .iter()
                .find(|item| item.name == name)
                .expect("alias")
                .outcomes
                .iter()
                .any(|item| item.label == "Source and YOU")
        );
    }
    selection.select(&evidence.action);
    assert_eq!(
        source_text(&report, selection.source),
        Some("0|-1|Target|Source and YOU")
    );
    let mut future = Box::pin(load(endpoint.clone(), Scope::Encounter));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(std::future::Future::poll(future.as_mut(), &mut context).is_pending());
    lifetime.invalidate();
    assert_eq!(future.await.expect_err("late response"), "Session changed");
    let fresh = ContextEndpoint {
        generation: lifetime.generation(),
        ..endpoint
    };
    let mut cancelled = Box::pin(load(fresh.clone(), Scope::Combatant("Target".into())));
    assert!(std::future::Future::poll(cancelled.as_mut(), &mut context).is_pending());
    drop(cancelled);
    assert!(
        load(fresh.clone(), Scope::Combatant("Target".into()))
            .await
            .expect("after cancel")
            .is_some()
    );
    let partial = root.join("partial.log");
    std::fs::write(&partial, "0|100|Target\n0|-1|Target\n0|invalid|Target\n")
        .expect("partial input");
    host.import_log(&partial).await.expect_err("invalid input");
    let rows = host.encounters(0, 16).await.expect("encounters");
    let partial_id = rows
        .items
        .iter()
        .find(|item| item.status == EncounterStatus::PartialImport)
        .expect("partial")
        .id;
    let partial_endpoint = ContextEndpoint {
        id: partial_id,
        ..fresh.clone()
    };
    let partial_report = load(partial_endpoint, Scope::Combatant("Target".into()))
        .await
        .expect("partial load")
        .expect("combatant");
    assert_eq!(partial_report.summary.status, 3);
    lifetime.close();
    assert_eq!(
        load(fresh, Scope::Encounter).await.expect_err("closed"),
        "Session ended"
    );
    host.shutdown().await.expect("shutdown");
}
