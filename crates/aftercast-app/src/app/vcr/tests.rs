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

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn lookup_table_ui_contract_uses_revision_sort_pages_and_session() {
    use aftercast::{TableScope, TableSort};
    let (_temp, _root, host, endpoint, lifetime) = fixture().await;
    let plugin = host.create_plugin(PluginConfig::builder().path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll")).type_name("Aftercast.PluginFixture.ColumnsPlugin").build()).await.expect("columns plugin");
    host.initialize_plugin(plugin)
        .await
        .expect("columns initialized");
    let report = super::super::lookup::load(&endpoint, "Source", "Damage", true)
        .await
        .expect("capture")
        .expect("category");
    let root = TableScope::Categories {
        encounter: endpoint.id,
        combatant: "Source".into(),
    };
    let page = super::super::lookup::load_aggregate(
        &endpoint,
        report.snapshot,
        root.clone(),
        None,
        None,
        0,
        None,
    )
    .await
    .expect("peer rows");
    assert_eq!(page.total, 1);
    assert_eq!(
        super::super::lookup::aggregate_row_label(page.rows.first().expect("peer")),
        "Target"
    );
    assert_eq!(
        page.rows.first().and_then(|row| row.combatant.as_deref()),
        Some("Target")
    );
    verify_lookup_columns(&endpoint, report.snapshot, root.clone()).await;
    let empty = super::super::lookup::load_aggregate(
        &endpoint,
        report.snapshot,
        root.clone(),
        None,
        None,
        16,
        None,
    )
    .await
    .expect("empty next page");
    assert!(empty.rows.is_empty());
    let skills = TableScope::Attacks {
        encounter: endpoint.id,
        combatant: "Source".into(),
        category: "Damage".into(),
    };
    let ordered = super::super::lookup::load_aggregate(
        &endpoint,
        report.snapshot,
        skills,
        Some("Target".into()),
        None,
        0,
        Some(TableSort {
            primary: "fixture".into(),
            secondary: None,
            descending: true,
        }),
    )
    .await
    .expect("sort");
    assert_eq!(ordered.total, 2);
    assert!(ordered.sort_error.is_none());
    endpoint
        .handle
        .capture_category_actions(endpoint.id, "Source", "Damage", true)
        .await
        .expect("replace");
    super::super::lookup::load_aggregate(
        &endpoint,
        report.snapshot,
        root.clone(),
        None,
        None,
        0,
        None,
    )
    .await
    .expect_err("stale");
    lifetime.invalidate();
    super::super::lookup::load_aggregate(&endpoint, report.snapshot, root, None, None, 0, None)
        .await
        .expect_err("session");
    host.shutdown().await.expect("shutdown");
}

#[test]
fn lookup_request_state_rejects_canceled_and_superseded_results() {
    use super::super::lookup::{RequestState, request_is_current};
    let initial = RequestState {
        offset: 0,
        sort: None,
        columns: Some(vec!["fixture".into()]),
        epoch: 1,
    };
    assert!(request_is_current(&initial, &initial));
    let mut newer = initial.clone();
    newer.offset = 16;
    assert!(!request_is_current(&initial, &newer));
    newer = initial.clone();
    newer.sort = Some(aftercast::TableSort {
        primary: "fixture".into(),
        secondary: None,
        descending: true,
    });
    assert!(!request_is_current(&initial, &newer));
    newer = initial.clone();
    newer.columns = Some(vec!["other".into()]);
    assert!(!request_is_current(&initial, &newer));
    newer = initial.clone();
    newer.epoch += 1;
    assert!(!request_is_current(&initial, &newer));
}

async fn verify_lookup_columns(
    endpoint: &ContextEndpoint,
    snapshot: aftercast::ActionSnapshot,
    root: aftercast::TableScope,
) {
    let selected = super::super::lookup::RequestState {
        offset: 0,
        sort: None,
        columns: Some(vec!["fixture".into()]),
        epoch: 0,
    };
    let picked = super::super::lookup::load_aggregate_selected(
        endpoint,
        snapshot,
        root.clone(),
        None,
        None,
        selected.clone(),
    )
    .await
    .expect("selected column");
    assert_eq!(
        picked
            .page
            .columns
            .first()
            .map(|column| column.key.as_str()),
        Some("fixture")
    );
    let mut missing = selected;
    missing.columns = Some(vec!["missing-column".into()]);
    super::super::lookup::load_aggregate_selected(
        endpoint,
        snapshot,
        root.clone(),
        None,
        None,
        missing,
    )
    .await
    .expect_err("missing column");
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn log_search_ui_contract_owns_snapshot_and_rejects_stale() {
    use super::super::log_search_panel::{State, capture};
    let (_temp, _root, host, endpoint, lifetime) = fixture().await;
    let frozen = capture(&endpoint).await.expect("capture").expect("logs");
    assert_eq!(frozen.rows.len(), 3);
    assert_eq!(
        endpoint
            .handle
            .search_snapshot_logs(frozen.snapshot, "STRIKE", false, None)
            .await
            .expect("text"),
        [0, 1, 2]
    );
    assert_eq!(
        endpoint
            .handle
            .search_snapshot_logs(frozen.snapshot, "^5\\|", true, None)
            .await
            .expect("regex"),
        [1]
    );
    assert!(
        endpoint
            .handle
            .search_snapshot_logs(frozen.snapshot, "no such line", false, None)
            .await
            .expect("empty result")
            .is_empty()
    );
    assert!(
        endpoint
            .handle
            .search_snapshot_logs(frozen.snapshot, "", false, Some(i32::MAX))
            .await
            .expect("empty type")
            .is_empty()
    );
    endpoint
        .handle
        .search_snapshot_logs(frozen.snapshot, "(", true, None)
        .await
        .expect_err("invalid regex");
    let mut controller = State::default();
    let first = controller.start();
    controller.cancel();
    assert!(!controller.current(first));
    let older = controller.start();
    let newer = controller.start();
    assert!(!controller.current(older));
    assert!(controller.current(newer));
    endpoint
        .handle
        .capture_log_snapshot(endpoint.id)
        .await
        .expect("replace");
    endpoint
        .handle
        .search_snapshot_logs(frozen.snapshot, "Strike", false, None)
        .await
        .expect_err("stale");
    lifetime.invalidate();
    capture(&endpoint).await.err().expect("closed session");
    host.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires managed fixture; run mise run test:gui-state"]
async fn trigger_benchmark_ui_contract_uses_owned_logs_without_effects() {
    use super::super::trigger_benchmark::{State, run};
    use aftercast::TriggerDefinition;
    let (_temp, root, host, mut endpoint, lifetime) = fixture().await;
    let first = host.triggers(0, 16).await.expect("trigger catalog");
    let definition = TriggerDefinition {
        category: "Synthetic".into(),
        regex: "Strike".into(),
        tabbed: true,
        timer: true,
        timer_name: "Synthetic".into(),
        ..TriggerDefinition::default()
    };
    let revision = host
        .save_trigger(first.revision, None, &definition)
        .await
        .expect("synthetic trigger");
    let key = host
        .triggers(0, 16)
        .await
        .expect("catalog")
        .items
        .first()
        .expect("trigger")
        .key
        .clone();
    run(&endpoint, revision, std::slice::from_ref(&key))
        .await
        .expect_err("short encounter");
    let path = root.join("benchmark.log");
    std::fs::write(&path, "0|100|Strike|Source|Target\n".repeat(1001)).expect("synthetic logs");
    host.import_log(&path).await.expect("import synthetic logs");
    for encounter in host.encounters(0, 16).await.expect("encounters").items {
        if host
            .encounter_logs(encounter.id, 0, 1)
            .await
            .expect("logs")
            .is_some_and(|logs| logs.page.total == 1001)
        {
            endpoint.id = encounter.id;
            break;
        }
    }
    let before = host
        .trigger_results(None, 1)
        .await
        .expect("results")
        .latest_sequence;
    let result = run(&endpoint, revision, std::slice::from_ref(&key))
        .await
        .expect("benchmark");
    assert_eq!(result.lines, 1001);
    assert_eq!(result.rows.first().expect("row").matches, 1001);
    assert_eq!(
        host.trigger_results(None, 1)
            .await
            .expect("results")
            .latest_sequence,
        before
    );
    let mut controller = State::default();
    let first = controller.begin();
    controller.cancel();
    assert!(!controller.current(first));
    let older = controller.begin();
    let newer = controller.begin();
    assert!(!controller.current(older) && controller.current(newer));
    lifetime.invalidate();
    run(&endpoint, revision, &[key])
        .await
        .expect_err("session ended");
    host.shutdown().await.expect("shutdown");
}
