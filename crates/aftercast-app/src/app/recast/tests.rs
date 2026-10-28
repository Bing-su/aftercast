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
        .type_name("Aftercast.PluginFixture.RecastImportPlugin").build()).await.expect("plugin");
    host.initialize_plugin(plugin).await.expect("initialize");
    let input = root.join("synthetic.log");
    std::fs::write(&input, "0|100|Strike|Source|Target\n5|0|Traumatic Swipe|Enemy|Source\n5|-1|Strike|Source|Target\n10|100|Strike|Source|Target\n12|-10|Killing|Source|Enemy\n15|-1|Strike|Source|Target\n20|100|Strike|Source|Target\n").expect("synthetic input");
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
async fn recast_ui_contract_actual_host_selection_evidence_and_lifecycle() {
    let (_temp, root, host, endpoint, lifetime) = fixture().await;
    let mut controller = State::default();
    controller.choose(Choice {
        category: "Damage".into(),
        attack: "Strike".into(),
        outgoing: true,
    });
    controller.restrict = false;
    controller.modifier = true;
    let (token, request, restrict) = controller.begin("Source").expect("begin");
    assert!(controller.begin("Source").is_none());
    controller.restrict = true;
    controller.modifier = false;
    let report = load(endpoint.clone(), request.clone())
        .await
        .expect("load")
        .expect("attack");
    controller.receive(token, restrict, Ok(Some(report.clone())));
    assert!(!controller.active_restrict && report.request.modifier);
    assert!(controller.restrict && !controller.modifier);
    let calculation = controller.calculate().expect("calculate").expect("report");
    assert_eq!(calculation.intervals, [5, 5, 3, 5]);
    assert_eq!(calculation.minimum, Some(3));
    assert_eq!(report.rows[1].action.amount, -1);
    let mut timer = crate::app::timers::EditorState::default();
    timer
        .receive_recast_draft(
            report.attack_name.as_deref().expect("name"),
            report.first_attacker.as_deref().expect("attacker"),
            calculation.minimum.expect("minimum"),
            report.request.modifier,
        )
        .expect("timer draft");
    timer
        .receive_recast_draft("other", "other", 0, false)
        .expect_err("unsaved draft");
    let swipe = report.rows[2].modifier_evidence.as_ref().expect("swipe");
    let source = Some((
        usize::try_from(swipe.source_index).expect("source"),
        swipe.time_sorter,
    ));
    assert_eq!(
        source_text(&report, source),
        Some("5|0|Traumatic Swipe|Enemy|Source")
    );
    assert_eq!(source_text(&report, Some((1, swipe.time_sorter + 1))), None);
    controller.checked[1] = false;
    assert_eq!(
        controller
            .calculate()
            .expect("calculate")
            .expect("report")
            .intervals,
        [10, 3, 5]
    );
    let (cancelled, _, active) = controller.begin("Source").expect("begin cancel");
    controller.cancel();
    controller.receive(cancelled, active, Ok(Some(report.clone())));
    assert_eq!(
        controller.report.as_ref().expect("previous").revision,
        report.revision
    );
    let fresh = crate::app::log_window::verify_delayed_lifecycle(endpoint, &lifetime).await;
    assert!(
        load(fresh.clone(), request.clone())
            .await
            .expect("fresh")
            .is_some()
    );
    let partial = root.join("partial.log");
    std::fs::write(
        &partial,
        "0|100|Strike|Source|Target\n5|-1|Strike|Source|Target\n6|invalid|Strike|Source|Target\n",
    )
    .expect("partial input");
    host.import_log(&partial).await.expect_err("invalid input");
    let id = host
        .encounters(0, 16)
        .await
        .expect("encounters")
        .items
        .iter()
        .find(|item| item.status == EncounterStatus::PartialImport)
        .expect("partial")
        .id;
    let partial_endpoint = ContextEndpoint {
        id,
        ..fresh.clone()
    };
    let partial_report = load(partial_endpoint, request.clone())
        .await
        .expect("partial load")
        .expect("attack");
    assert_eq!(partial_report.summary.status, 3);
    lifetime.close();
    assert_eq!(
        load(fresh, request).await.expect_err("closed"),
        "Session ended"
    );
    host.shutdown().await.expect("shutdown");
}
