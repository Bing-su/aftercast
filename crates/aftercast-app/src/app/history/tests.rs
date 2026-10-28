use super::*;
#[test]
fn civil_date_bounds_preserve_whole_days_precision_and_validation() {
    let epoch = 621_355_968_000_000_000;
    assert_eq!(bound("1970-01-01", false).expect("date"), Some(epoch));
    assert_eq!(
        bound("1970-01-01", true).expect("inclusive date"),
        Some(epoch + 864_000_000_000)
    );
    assert_eq!(
        bound("1970-01-01T00:00:00.1234567", false).expect("precision"),
        Some(epoch + 1_234_567)
    );
    assert_eq!(bound("0001-01-01", false).expect("minimum"), Some(0));
    assert_eq!(
        bound("9999-12-31", true).expect("maximum date"),
        Some(MAX_TICKS + 1)
    );
    assert_eq!(
        bound("9999-12-31", false).expect("maximum start day"),
        Some(MAX_TICKS + 1 - 864_000_000_000)
    );
    assert_eq!(
        bound("9999-12-31T23:59:59.9999999", false).expect("maximum exact time"),
        Some(MAX_TICKS)
    );
    for invalid in [
        "2025-02-29",
        "0000-01-01",
        "2024-01-01Z",
        "2024-01-01T00:00:00Z",
        "2024-01-01T00:00:00.12345678",
        "2024-01-01T00:00",
        "2024-01-01T00:00:60",
        "2024-01-01T24:00:00",
    ] {
        bound(invalid, false)
            .expect_err("invalid source date must not infer offsets/round precision");
    }
    SearchDraft {
        start: "2024-02-02".into(),
        end: "2024-02-01".into(),
        ..Default::default()
    }
    .compile()
    .expect_err("reversed range");
}

use aftercast::{Host, HostConfig, PluginConfig, SourceLogLink, extract_bridge};
use camino::Utf8Path;

async fn parser(host: &Host, ui: &mut AppState) {
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let id = host
        .create_plugin(
            PluginConfig::builder()
                .path(path.clone())
                .type_name("Aftercast.PluginFixture.ImportPlugin")
                .build(),
        )
        .await
        .expect("parser");
    let status = host.initialize_plugin(id).await.expect("initialize parser");
    ui.plugins.push(super::super::LoadedPlugin {
        path: path.into_string(),
        id,
        status,
        info: host.plugin_status(id).await.expect("status"),
    });
}
async fn query(host: &Host, ui: &mut AppState, applied: Applied, keep: bool) -> HistoryPage {
    let token = ui.begin_history_query(keep, true).expect("query admitted");
    assert!(ui.begin_history_query(keep, true).is_none());
    assert!(ui.receive_history_query(token, read(host, applied).await));
    ui.history.page.clone().expect("page received")
}
fn choose(ui: &mut AppState, page: &HistoryPage) -> Confirmation {
    let id = &page.items.first().expect("selectable row").id;
    assert!(ui.history.toggle(ui.history.generation, &page.revision, id));
    assert!(ui.history.confirm());
    ui.history.confirmation.clone().expect("confirmation")
}
fn invalidate_probe(ui: &mut AppState, id: aftercast::EncounterId) -> (u64, u64) {
    ui.selected = Some(id);
    ui.detail_loading = true;
    (ui.revision.selection, ui.log_windows.generation())
}
fn assert_invalidated(ui: &AppState, before: (u64, u64)) {
    assert!(ui.selected.is_none());
    assert!(!ui.detail_loading);
    assert!(ui.detail.is_none());
    assert!(ui.revision.selection > before.0);
    assert!(ui.log_windows.generation() > before.1);
    assert!(ui.history.selected.is_empty());
    assert!(ui.history.stale);
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and synthetic plugin fixture; run mise run test:gui-state"]
async fn history_ui_contract_uses_real_snapshots_and_restore_results() {
    let temporary = tempfile::tempdir().expect("isolated history contract");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8 temp root");
    let bridge = root.join("bridge");
    extract_bridge(&bridge).expect("bridge");
    let executable = std::env::current_exe().expect("test executable");
    let application = executable
        .parent()
        .and_then(std::path::Path::parent)
        .expect("target directory")
        .join("examples")
        .join(format!("test-application{}", std::env::consts::EXE_SUFFIX));
    let config = || {
        HostConfig::builder()
            .dotnet_root(std::env::var("DOTNET_ROOT").expect("dotnet"))
            .bridge_directory(bridge.clone())
            .data_directory(root.join("state"))
            .application_executable(
                camino::Utf8PathBuf::try_from(application.clone()).expect("UTF8 executable"),
            )
            .build()
    };
    let mut ui = AppState::new(root.to_owned());
    ui.phase = Phase::Ready;
    ui.revision.session = 1;
    let host = Host::start(config()).await.expect("seed host");
    let default = SearchDraft::default().compile().expect("default");
    assert!(
        !query(&host, &mut ui, default.clone(), false)
            .await
            .index_present
    );
    parser(&host, &mut ui).await;
    let input = root.join("synthetic.log");
    for number in 1..=20 {
        std::fs::write(&input, format!("0|{number}\n0|{}\n", number + 100)).expect("synthetic log");
        host.import_log(&input).await.expect("import");
    }
    let old = host
        .encounters(0, 1)
        .await
        .expect("seed graph")
        .items
        .first()
        .expect("encounter")
        .id;
    assert_eq!(host.save_history().await.expect("save"), 20);
    host.shutdown().await.expect("seed shutdown");
    let host = Host::start(config()).await.expect("empty restarted graph");
    ui = AppState::new(root.to_owned());
    ui.phase = Phase::Ready;
    ui.revision.session = 2;
    parser(&host, &mut ui).await;
    selection_and_queries(&host, &mut ui, default.clone()).await;
    failed_restores(&host, &mut ui, root, old, default.clone()).await;
    successful_restore_and_shutdown(host, &mut ui, old, default).await;
}

async fn selection_and_queries(host: &Host, ui: &mut AppState, default: Applied) {
    let page = query(host, ui, default.clone(), false).await;
    assert_eq!(page.total, 20);
    assert_eq!(page.items.len(), 16);
    let confirmation = choose(ui, &page);
    assert!(ui.history.cancel_confirmation(confirmation.version));
    assert!(ui.selected.is_none());
    assert_eq!(
        host.encounters(0, 1)
            .await
            .expect("cancel leaves graph")
            .total,
        0
    );
    assert!(ui.history.confirm());
    let reopened = ui.history.confirmation.clone().expect("reopened");
    assert!(!ui.history.cancel_confirmation(confirmation.version));
    assert!(ui.begin_history_restore(&confirmation).is_none());
    assert!(ui.history.confirmation_current(&reopened));
    let mut second = default.clone();
    second.query.offset = 16;
    let second_page = query(host, ui, second, true).await;
    assert_eq!(second_page.revision, page.revision);
    assert_eq!(ui.history.selected.len(), 1);
    assert_eq!(
        second_page.items.first().expect("second page").start,
        page.items.first().expect("first page").start
    );
    canceled_queries(host, ui, default).await;
}

async fn canceled_queries(host: &Host, ui: &mut AppState, default: Applied) {
    let page = ui.history.page.clone().expect("current page");
    let selected_generation = ui.history.generation;
    ui.history.draft.text = "unapplied draft".into();
    let cancel = ui
        .begin_history_query(true, true)
        .expect("cancelable query");
    let result = read(host, default.clone()).await;
    assert!(ui.cancel_history_query(cancel, "canceled".into()));
    assert!(!ui.cancel_history_query(cancel, "twice".into()));
    assert!(!ui.receive_history_query(cancel, result));
    assert!(ui.phase == Phase::Ready);
    assert_eq!(
        ui.history.applied.as_ref().expect("applied").query.offset,
        16
    );
    assert_eq!(ui.history.draft.text, "unapplied draft");
    let newer = ui.begin_history_query(false, true).expect("next query");
    assert!(!ui.receive_history_query(cancel, read(host, default.clone()).await));
    assert_eq!(ui.history.pending, Some((newer, Kind::Query)));
    assert!(ui.receive_history_query(newer, read(host, default.clone()).await));
    assert!(!ui.history.toggle(
        selected_generation,
        &page.revision,
        &page.items.first().expect("row").id
    ));
    let empty = SearchDraft {
        text: "no matching encounter".into(),
        ..Default::default()
    }
    .compile()
    .expect("search");
    assert_eq!(query(host, ui, empty, false).await.total, 0);
    assert!(ui.history.selected.is_empty());
    let mut beyond = default.clone();
    beyond.query.offset = 1000;
    assert_eq!(query(host, ui, beyond, false).await.items.len(), 16);
    assert_eq!(
        ui.history.applied.as_ref().expect("fallback").query.offset,
        0
    );
}

async fn failed_restores(
    host: &Host,
    ui: &mut AppState,
    root: &Utf8Path,
    old: aftercast::EncounterId,
    default: Applied,
) {
    let page = ui.history.page.clone().expect("page");
    let frozen = choose(ui, &page);
    let index = root.join("state/history/index.xml");
    let bytes = std::fs::read(&index).expect("owned index");
    std::fs::write(
        &index,
        String::from_utf8(bytes.clone())
            .expect("UTF8")
            .replace("Import Zone", "Changed Zone"),
    )
    .expect("concurrent synthetic write");
    let before = invalidate_probe(ui, old);
    let token = ui.begin_history_restore(&frozen).expect("restore admitted");
    assert_invalidated(ui, before);
    assert!(ui.begin_history_restore(&frozen).is_none());
    let ids = frozen
        .rows
        .iter()
        .map(|row| row.entry.id.clone())
        .collect::<Vec<_>>();
    let result = host.restore_selected_history(&frozen.revision, &ids).await;
    assert!(result.is_err(), "stale disk snapshot rejected");
    assert!(ui.receive_history_restore(token, result, "restored".into()));
    assert!(ui.error.is_some());
    assert_eq!(
        host.encounters(0, 1).await.expect("unchanged graph").total,
        0
    );
    query(host, ui, default.clone(), false).await;
    assert!(ui.history.selected.is_empty());
    std::fs::write(&index, bytes).expect("restore own fixture");
    let page = query(host, ui, default.clone(), false).await;
    corrupt_payloads(host, ui, root, &page, old).await;
}

async fn successful_restore_and_shutdown(
    host: Host,
    ui: &mut AppState,
    old: aftercast::EncounterId,
    default: Applied,
) {
    let page = query(&host, ui, default.clone(), false).await;
    let first = choose(ui, &page);
    let mut second = default.clone();
    second.query.offset = 16;
    let page = query(&host, ui, second, true).await;
    let confirmation = choose(ui, &page);
    assert_eq!(confirmation.rows.len(), 2);
    assert!(!ui.history.confirmation_current(&first));
    let before = invalidate_probe(ui, old);
    let token = ui.begin_history_restore(&confirmation).expect("restore");
    assert_invalidated(ui, before);
    ui.screen = "settings";
    let ids = confirmation
        .rows
        .iter()
        .map(|row| row.entry.id.clone())
        .collect::<Vec<_>>();
    let result = host
        .restore_selected_history(&confirmation.revision, &ids)
        .await;
    assert_eq!(*result.as_ref().expect("actual restore"), 2);
    let before_ack = invalidate_probe(ui, old);
    assert!(ui.receive_history_restore(token, result, "restored".into()));
    assert_invalidated(ui, before_ack);
    assert_eq!(ui.screen, "settings");
    assert!(ui.begin_history_restore(&confirmation).is_none());
    restored_identity_and_shutdown(host, ui, token, default).await;
}

async fn restored_identity_and_shutdown(
    host: Host,
    ui: &mut AppState,
    token: Token,
    default: Applied,
) {
    let rows = host.encounters(0, 1).await.expect("restored graph");
    assert_eq!(rows.total, 2);
    let id = rows.items.first().expect("restored encounter").id;
    let actions = host
        .encounter_actions(id, 1, 1)
        .await
        .expect("actions")
        .expect("encounter");
    let logs = host
        .encounter_logs(id, 1, 1)
        .await
        .expect("logs")
        .expect("encounter");
    assert_eq!(
        actions.items.first().expect("action").source,
        SourceLogLink::Retained(1)
    );
    assert_eq!(
        actions.items.first().expect("action").time_sorter,
        logs.page.items.first().expect("log").time_sorter
    );
    let page = query(&host, ui, default.clone(), false).await;
    let restored = page
        .items
        .iter()
        .find(|row| row.already_restored)
        .expect("marked restored");
    assert!(
        !ui.history
            .toggle(ui.history.generation, &page.revision, &restored.id)
    );
    let pending = ui
        .begin_history_query(false, true)
        .expect("pending during shutdown");
    let result = read(&host, default).await;
    host.shutdown()
        .await
        .expect("shutdown drains accepted work");
    ui.revision.session += 1;
    ui.history = Default::default();
    ui.phase = Phase::Idle;
    assert!(!ui.receive_history_query(pending, result));
    assert!(!ui.receive_history_restore(token, Ok(1), "late".into()));
    assert!(ui.phase == Phase::Idle);
    assert!(ui.history.page.is_none());
}

async fn corrupt_payloads(
    host: &Host,
    ui: &mut AppState,
    root: &Utf8Path,
    page: &HistoryPage,
    old: aftercast::EncounterId,
) {
    let confirmation = choose(ui, page);
    // Damage only this fixture's payloads; opaque IDs remain owned by the real Host.
    let payloads = std::fs::read_dir(root.join("state/history"))
        .expect("own history")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "act"))
        .map(|path| {
            let bytes = std::fs::read(&path).expect("payload");
            (path, bytes)
        })
        .collect::<Vec<_>>();
    assert!(!payloads.is_empty());
    for (path, _) in &payloads {
        std::fs::write(path, b"corrupt synthetic payload").expect("corrupt own payload");
    }
    let token = ui
        .begin_history_restore(&confirmation)
        .expect("corrupt restore");
    let ids = confirmation
        .rows
        .iter()
        .map(|row| row.entry.id.clone())
        .collect::<Vec<_>>();
    let result = host
        .restore_selected_history(&confirmation.revision, &ids)
        .await;
    assert!(result.is_err(), "corrupt payload cannot attach graph");
    let before = invalidate_probe(ui, old);
    assert!(ui.receive_history_restore(token, result, "restored".into()));
    assert_invalidated(ui, before);
    assert_eq!(
        host.encounters(0, 1).await.expect("atomic failure").total,
        0
    );
    for (path, bytes) in payloads {
        std::fs::write(path, bytes).expect("restore own payload");
    }
}

#[test]
fn recording_policy_labels_do_not_infer_old_history() {
    assert_eq!(
        recording_policy_label(Some(true)),
        "history-selective-recording"
    );
    assert_eq!(
        recording_policy_label(Some(false)),
        "history-nonselective-recording"
    );
    assert_eq!(recording_policy_label(None), "history-recording-unknown");
}
