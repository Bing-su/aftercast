use super::*;
use aftercast::{Host, HostConfig, PluginConfig};

fn config(root: &Utf8Path) -> HostConfig {
    let bridge = root.join("bridge");
    aftercast::extract_bridge(&bridge).expect("payload");
    let application = std::env::current_exe()
        .expect("test path")
        .parent()
        .and_then(std::path::Path::parent)
        .expect("target")
        .join("examples")
        .join(format!("test-application{}", std::env::consts::EXE_SUFFIX));
    HostConfig::builder()
        .dotnet_root(std::env::var("DOTNET_ROOT").expect("dotnet"))
        .bridge_directory(bridge)
        .data_directory(root.join("state"))
        .application_executable(Utf8PathBuf::try_from(application).expect("UTF8"))
        .build()
}
async fn parser(host: &Host, ui: &mut AppState) {
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let id = host
        .create_plugin(
            PluginConfig::builder()
                .path(path.clone())
                .type_name("Aftercast.PluginFixture.CompanionPlugin")
                .build(),
        )
        .await
        .expect("plugin");
    let status = host.initialize_plugin(id).await.expect("parser");
    ui.plugins.push(super::super::LoadedPlugin {
        path: path.into_string(),
        id,
        status,
        info: host.plugin_status(id).await.expect("status"),
    });
}
fn edit(ui: &mut AppState, name: &str, field: &str, value: &str) {
    assert!(
        ui.edit_source(ui.source_view(), name, field, value)
            .expect("choice")
    );
}
async fn candidates(host: &Host, ui: &mut AppState, main: &Utf8Path) {
    ui.import_path = main.to_string();
    let token = ui.begin_sources(Kind::Catalog).expect("query");
    assert!(ui.begin_sources(Kind::Catalog).is_none(), "duplicate query");
    assert!(ui.receive_catalog(token, host.companion_candidates(main).await));
}
async fn full(host: &Host, ui: &mut AppState, main: &Utf8Path) -> aftercast::ImportReport {
    let picker = ui.select_full_import().expect("select");
    let request = ui
        .admit_full_path(picker.token, main.to_owned())
        .expect("admit");
    let result = import_text::start_selected(
        host,
        main,
        None,
        request.options(),
        request.sources.as_ref(),
    )
    .await
    .expect("start")
    .wait()
    .await;
    let report = result.as_ref().expect("complete").to_owned();
    assert!(ui.receive_full_import(request.token, result, "complete", "cancelled"));
    report
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and synthetic Host; run mise run test:gui-state"]
async fn companion_ui_contract_import_selection_failure_and_stale_responses() {
    let temporary = tempfile::tempdir().expect("private fixtures");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8");
    let host = Host::start(config(root)).await.expect("worker");
    let mut ui = AppState::new(root.to_owned());
    ui.phase = Phase::Ready;
    ui.revision.session = 1;
    let main = root.join("main.log");
    let sides = root.join("CompanionLogs");
    std::fs::create_dir(&sides).expect("sides");
    std::fs::write(&main, "0|10\n3|20\n").expect("main");
    let side = sides.join("main.Alpha.log");
    std::fs::write(&side, "1|100\n3|200\n9|999\n").expect("side");
    candidates(&host, &mut ui, &main).await;
    parser(&host, &mut ui).await;
    edit(&mut ui, "Alpha", "selected", "");
    edit(&mut ui, "Alpha", "encoding", "utf8");
    let snapshot = ui.companions.selection().expect("snapshot");
    let report = full(&host, &mut ui, &main).await;
    assert_eq!((report.damage, ui.companions.excluded), (330, 1));
    // The admitted request owns codec/selection even if a later controller draft changes.
    edit(&mut ui, "Alpha", "encoding", "ascii");
    assert_eq!(
        snapshot.sources[0].text.expect("codec").encoding,
        aftercast::ImportEncoding::Utf8
    );
    let other = root.join("other.log");
    std::fs::write(&other, "0|999\n").expect("other");
    snapshot
        .start(&host, &other, None, None)
        .await
        .err()
        .expect("wrong main rejected");
    std::fs::remove_file(&side).expect("remove synthetic source only");
    let picker = ui.select_full_import().expect("failed selection");
    let request = ui
        .admit_full_path(picker.token, main.clone())
        .expect("admit missing side");
    let result = match import_text::start_selected(
        &host,
        &main,
        None,
        request.options(),
        request.sources.as_ref(),
    )
    .await
    {
        Ok(job) => job.wait().await,
        Err(error) => Err(error),
    };
    assert!(result.is_err());
    assert!(ui.receive_full_import(request.token, result, "complete", "cancelled"));
    assert!(ui.error.is_some());
    assert_eq!(
        ui.companions.choices[0].text.key(),
        "ascii",
        "failure retains draft"
    );
    let token = ui.begin_sources(Kind::Catalog).expect("stale query");
    let result = host.companion_candidates(&main).await;
    ui.revision.session += 1;
    ui.companions.reset_session();
    ui.phase = Phase::Stopping;
    assert!(!ui.receive_catalog(token, result));
    assert!(ui.companions.catalog.is_none() && ui.phase == Phase::Stopping);
    host.shutdown().await.expect("stop worker");
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and synthetic Host; run mise run test:gui-state"]
async fn companion_ui_contract_live_navigation_reopen_failures_and_file_changes() {
    use std::time::{Duration, Instant};
    let temporary = tempfile::tempdir().expect("fixtures");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8");
    let configuration = config(root);
    let host = Host::start(configuration.clone()).await.expect("worker");
    let mut ui = AppState::new(root.to_owned());
    ui.phase = Phase::Ready;
    ui.revision.session = 1;
    parser(&host, &mut ui).await;
    let main = root.join("main.log");
    let sides = root.join("CompanionLogs");
    std::fs::create_dir(&sides).expect("sides");
    std::fs::write(&main, "0|10\n").expect("main");
    let side = sides.join("main.Alpha.log");
    std::fs::write(&side, "").expect("side");
    candidates(&host, &mut ui, &main).await;
    edit(&mut ui, "Alpha", "selected", "");
    let options = ui.companions.live_options(None).expect("options");
    let token = ui.begin_sources(Kind::Open).expect("open");
    assert!(ui.begin_sources(Kind::Open).is_none(), "duplicate open");
    let old_view = ui.source_view();
    assert!(
        !ui.edit_source(old_view, "Alpha", "selected", "")
            .expect("ignored busy edit")
    );
    let result = host.open_live_logs(&main, options).await;
    ui.screen = "analysis";
    assert!(
        ui.receive_live(token, Kind::Open, result, None),
        "tab navigation retains accepted operation"
    );
    let selection = ui.companions.live.as_ref().expect("live").selection;
    edit(&mut ui, "", "wait", "0");
    ui.companions
        .live_options(None)
        .expect_err("invalid clock draft");
    assert_eq!(
        host.live_logs(selection)
            .await
            .expect("preflight preserves live")
            .selection,
        selection
    );
    edit(&mut ui, "", "wait", "30000");
    std::fs::write(&main, "").expect("truncate synthetic live image");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let token = ui.begin_sources(Kind::Status).expect("status");
        let result = host.live_logs(selection).await;
        assert!(ui.receive_live(token, Kind::Status, result, None));
        if ui
            .companions
            .live
            .as_ref()
            .expect("status selection")
            .sources[0]
            .generation
            > 1
        {
            break;
        }
        assert!(Instant::now() < deadline, "file change not exposed");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    std::fs::remove_file(&side).expect("remove synthetic selected source");
    let options = ui.companions.live_options(None).expect("reopen snapshot");
    let token = ui.begin_sources(Kind::Open).expect("replace");
    let result = host.reopen_live_logs(selection, &main, options).await;
    assert!(result.is_err());
    let kept = retained(&host, Some(selection)).await;
    assert!(ui.receive_live(token, Kind::Open, result, kept));
    assert!(ui.companions.live.is_none() && ui.companions.error.is_some());
    assert_eq!(ui.companions.choices.len(), 1);
    std::fs::write(&side, "").expect("retry side");
    let options = ui.companions.live_options(None).expect("retry options");
    let token = ui.begin_sources(Kind::Open).expect("retry");
    assert!(ui.receive_live(
        token,
        Kind::Open,
        host.open_live_logs(&main, options).await,
        None
    ));
    let selection = ui.companions.live.as_ref().expect("retried").selection;
    let token = ui.begin_sources(Kind::Status).expect("inflight status");
    let result = host.live_logs(selection).await;
    // Editing a path during a read-only status must invalidate its response without stranding pending.
    ui.edit_import_path(ui.text_view(), root.join("next.log").to_string());
    assert!(!ui.receive_live(token, Kind::Status, result, None));
    assert!(ui.companions.pending.is_none());
    close_late_and_restart(host, &mut ui, &main, configuration).await;
}

async fn close_late_and_restart(
    host: Host,
    ui: &mut AppState,
    main: &Utf8Path,
    configuration: HostConfig,
) {
    let selection = ui
        .companions
        .live
        .as_ref()
        .expect("live selection before close")
        .selection;
    let token = ui.begin_sources(Kind::Close).expect("close");
    let result = host.close_live_logs(selection).await;
    assert!(ui.receive_close(token, result, None));
    assert!(ui.companions.live.is_none());
    let token = ui.begin_sources(Kind::Open).expect("late open");
    let result = host.open_live_logs(&main, LiveLogOptions::default()).await;
    ui.revision.session += 1;
    ui.companions.reset_session();
    ui.phase = Phase::Stopping;
    assert!(!ui.receive_live(token, Kind::Open, result, None));
    assert!(ui.companions.live.is_none() && ui.phase == Phase::Stopping);
    host.shutdown()
        .await
        .expect("shutdown owns late open cleanup");
    let restarted = Host::start(configuration).await.expect("lease released");
    restarted.shutdown().await.expect("restart stop");
}
