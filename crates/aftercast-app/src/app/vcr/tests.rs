use super::*;
use crate::app::log_window::{WindowAppearance, WindowLifetime};
use aftercast::{Host, HostConfig, PluginConfig, extract_bridge};
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
        .type_name("Aftercast.PluginFixture.VcrImportPlugin").build()).await.expect("plugin");
    host.initialize_plugin(plugin).await.expect("initialize");
    let input = root.join("synthetic.log");
    std::fs::write(
        &input,
        "0|100|Strike|Source|Target\n5|50|Strike|Source|Target\n10|25|Strike|Source|Target\n",
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
async fn vcr_ui_contract_owns_capture_and_rejects_late_results() {
    let (_temp, root, host, endpoint, lifetime) = fixture().await;
    let mut controller = State::default();
    let token = controller.begin().expect("capture");
    assert!(controller.begin().is_none());
    let snapshot = load(&endpoint, false)
        .await
        .expect("load")
        .expect("encounter");
    controller.receive(token, Ok(Some(snapshot.clone())), false);
    assert_eq!(controller.clock.position, 0);
    assert_eq!(controller.selected, snapshot.categories);
    let frame = vcr_frame(&snapshot, 5, 5, &["Damage".into()], true, true).expect("frame");
    assert!(
        frame
            .edges
            .iter()
            .any(|edge| edge.from == "Source" && edge.to == "Target" && edge.amount == 150)
    );
    controller.lookback = "0".into();
    controller.clock.linked = true;
    let token = controller.begin().expect("follow refresh");
    controller.receive(token, Ok(Some(snapshot.clone())), true);
    assert_eq!(controller.lookback, "1");
    assert_eq!(controller.clock.position, snapshot.duration);
    let token = controller.begin().expect("refresh");
    controller.cancel();
    controller.receive(token, Err("late".into()), true);
    assert!(controller.error.is_none());
    assert_eq!(
        controller.snapshot.as_ref().expect("original").revision,
        snapshot.revision
    );
    let token = controller.begin().expect("failure");
    controller.receive(token, Err("synthetic failure".into()), false);
    assert_eq!(controller.error.as_deref(), Some("synthetic failure"));
    assert_eq!(
        controller.snapshot.as_ref().expect("preserved").revision,
        snapshot.revision
    );
    assert!(
        load(&endpoint, true)
            .await
            .expect("no live encounter")
            .is_none()
    );
    let token = controller.begin().expect("follow unavailable");
    controller.clock.linked = true;
    controller.receive(token, Ok(None), true);
    assert!(!controller.clock.linked);
    assert_eq!(
        controller
            .snapshot
            .as_ref()
            .expect("previous snapshot")
            .revision,
        snapshot.revision
    );
    let token = controller.begin().expect("before close");
    controller.close();
    controller.receive(token, Ok(Some(snapshot.clone())), false);
    assert!(controller.snapshot.is_none());
    let token = controller.begin().expect("reopen");
    controller.receive(token, Ok(Some(snapshot.clone())), false);
    assert_eq!(controller.clock.position, 0);
    let fresh = crate::app::log_window::verify_delayed_lifecycle(endpoint, &lifetime).await;
    assert!(
        load(&fresh, false)
            .await
            .expect("after invalidate")
            .is_some()
    );
    check_partial(&root, &host, fresh.clone()).await;
    lifetime.close();
    assert_eq!(
        load(&fresh, false).await.expect_err("closed"),
        "Session ended"
    );
    host.shutdown().await.expect("shutdown");
}

async fn check_partial(root: &Utf8Path, host: &Host, fresh: ContextEndpoint) {
    let partial = root.join("partial.log");
    std::fs::write(
        &partial,
        "0|100|Strike|Source|Target\n5|broken|Strike|Source|Target\n",
    )
    .expect("partial fixture");
    host.import_log(&partial)
        .await
        .expect_err("partial failure");
    let id = host
        .encounters(0, 16)
        .await
        .expect("encounters")
        .items
        .into_iter()
        .find(|entry| entry.status == aftercast::EncounterStatus::PartialImport)
        .expect("partial encounter")
        .id;
    let partial_endpoint = ContextEndpoint {
        id,
        ..fresh.clone()
    };
    let snapshot = load(&partial_endpoint, false)
        .await
        .expect("partial capture")
        .expect("partial encounter");
    assert_eq!(snapshot.summary.status, 3);
    assert_eq!(snapshot.logs.len(), 2);
    assert!(
        snapshot
            .logs
            .iter()
            .any(|line| line.text.contains("broken"))
    );
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn lookup_ui_contract_uses_category_snapshot_and_session_guard() {
    let (_temp, _root, host, endpoint, lifetime) = fixture().await;
    let outgoing = super::super::lookup::load(&endpoint, "Source", "Damage", true)
        .await
        .expect("outgoing")
        .expect("category");
    assert_eq!(outgoing.actions.len(), 3);
    let mut collision = outgoing.actions.clone();
    collision[0].action.attack = "All".into();
    let aggregate = super::super::lookup::matched_indices(&collision, "Target", true, None);
    assert_eq!(aggregate.len(), 4);
    assert_eq!(aggregate.iter().filter(|&&index| index == 0).count(), 2);
    assert_eq!(
        aggregate,
        super::super::lookup::matched_indices(&collision, "Target", true, Some("All"))
    );
    assert!(super::super::lookup::matched_indices(&collision, "Source", true, None).is_empty());
    assert!(
        outgoing
            .actions
            .iter()
            .all(|row| super::super::lookup::peer(row, true) == "Target")
    );
    let first = &outgoing.actions[0].action;
    let aftercast::SourceLogLink::Retained(offset) = first.source else {
        panic!("source missing")
    };
    let logs = endpoint
        .handle
        .action_snapshot_logs(outgoing.snapshot, offset, 1)
        .await
        .expect("log");
    assert_eq!(logs.page.items[0].time_sorter, first.time_sorter);
    let incoming = super::super::lookup::load(&endpoint, "Target", "Taken", false)
        .await
        .expect("incoming")
        .expect("category");
    assert_eq!(incoming.actions.len(), 3);
    assert!(
        incoming
            .actions
            .iter()
            .all(|row| super::super::lookup::peer(row, false) == "Source")
    );
    endpoint
        .handle
        .action_snapshot_page(outgoing.snapshot, 0, 1)
        .await
        .expect_err("stale snapshot");
    assert!(
        super::super::lookup::load(&endpoint, "Source", "Damage", false)
            .await
            .expect("wrong direction")
            .is_none()
    );
    lifetime.invalidate();
    assert!(
        super::super::lookup::load(&endpoint, "Source", "Damage", true)
            .await
            .is_err()
    );
    host.shutdown().await.expect("shutdown");
}
