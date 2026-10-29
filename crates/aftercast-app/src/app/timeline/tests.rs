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
        "0|100|Target\n5|-1|Target\n10|200|Target\n15|-1|Target\n20|-1|Target\n",
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
async fn timeline_ui_contract_actual_host_snapshot_range_and_lifecycle() {
    let (_temp, root, host, endpoint, lifetime) = fixture().await;
    host.encounter_detail(endpoint.id)
        .await
        .expect("detail")
        .expect("encounter");
    let mut controller = State {
        window_text: "bad".into(),
        ..State::default()
    };
    assert!(controller.begin().is_none());
    assert_eq!(controller.window_text, "bad");
    controller.window_text = "10".into();
    let (token, options) = controller.begin().expect("begin");
    assert!(controller.begin().is_none());
    controller.draft.grouping = 2;
    let loaded = load(endpoint.clone(), options)
        .await
        .expect("load")
        .expect("encounter");
    assert_eq!(loaded.options.grouping, 0);
    controller.receive(token, Ok(Some(loaded.clone())));
    assert_eq!(controller.draft.grouping, 2);
    assert_eq!(point_time(&loaded, 9), Some((10.0, 1.0)));
    let mut segments = (*loaded).clone();
    segments.options.rolling = false;
    segments.points = 2;
    assert_eq!(point_time(&segments, 0), Some((10.0, 10.0)));
    assert_eq!(logs_at(&loaded, 5.0, 5.0), vec![1]);
    assert_eq!(logs_at(&loaded, 10.0, 1.0), vec![2]);
    assert_eq!(loaded.logs[2].text, "10|200|Target");
    super::log_window::verify_retained_view(&loaded);
    assert_eq!(point_time(&loaded, loaded.points as usize), None);
    assert!(!graph_points(&[0, 10, 20], 0, 3, 20.0).is_empty());
    let (cancelled, _) = controller.begin().expect("begin cancel");
    controller.cancel();
    controller.receive(cancelled, Ok(Some(loaded.clone())));
    assert_eq!(
        controller
            .report
            .as_ref()
            .expect("previous report")
            .revision,
        loaded.revision
    );
    let (failed, _) = controller.begin().expect("begin failed");
    controller.receive(failed, Err("synthetic failure".into()));
    assert_eq!(controller.error.as_deref(), Some("synthetic failure"));
    assert_eq!(
        controller
            .report
            .as_ref()
            .expect("preserved report")
            .revision,
        loaded.revision
    );
    let fresh = crate::app::log_window::verify_delayed_lifecycle(endpoint, &lifetime).await;
    assert!(
        load(fresh.clone(), options)
            .await
            .expect("after cancel")
            .is_some()
    );
    check_partial(&root, &host, fresh.clone(), options).await;
    lifetime.close();
    assert_eq!(
        load(fresh, options).await.expect_err("closed"),
        "Session ended"
    );
    host.shutdown().await.expect("shutdown");
}

async fn check_partial(
    root: &Utf8Path,
    host: &Host,
    fresh: ContextEndpoint,
    options: TimelineOptions,
) {
    let partial = root.join("partial.log");
    std::fs::write(&partial, "0|100|Target\n5|100|Target\n10|invalid|Target\n").expect("partial");
    host.import_log(&partial).await.expect_err("invalid input");
    let rows = host.encounters(0, 16).await.expect("encounters");
    let id = rows
        .items
        .iter()
        .find(|item| item.status == EncounterStatus::PartialImport)
        .expect("partial")
        .id;
    host.encounter_detail(id)
        .await
        .expect("detail")
        .expect("encounter");
    let partial_endpoint = ContextEndpoint {
        id,
        ..fresh.clone()
    };
    let partial_report = load(partial_endpoint, options)
        .await
        .expect("partial load")
        .expect("encounter");
    assert_eq!(partial_report.summary.status, 3);
}

#[test]
fn graph_coordinates_and_scale_are_deterministic() {
    assert_eq!(graph_axis([100].into_iter()), 125.0);
    assert_eq!(
        graph_points(&[0, 10, 20], 0, 3, 20.0),
        "0.00,50.00 300.00,27.50 600.00,5.00"
    );
}
