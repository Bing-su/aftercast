mod common;
use aftercast::{Error, Host, PluginConfig, TimerDefinition};
use camino::Utf8Path;
use common::{Deployment, deployment};

async fn fixture(host: &Host) {
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let id = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.TimerMutationPlugin")
                .build(),
        )
        .await
        .expect("synthetic timer plugin");
    host.initialize_plugin(id).await.expect("initialize");
}
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn timer_definitions_persist_without_restoring_or_resetting_countdowns(
    deployment: Deployment,
) {
    let host = Host::start(deployment.config()).await.expect("host");
    normalized_identity(&host).await;
    fixture(&host).await;
    let page = host
        .timer_definitions(0, 16)
        .await
        .expect("definitions and retained frames");
    assert_eq!(page.retained_frames, 1);
    let item = page.items.first().expect("seed timer");
    let definition = TimerDefinition {
        name: "Edited timer".into(),
        category: "Fixture".into(),
        timer_value: 45,
        warning_value: 7,
        remove_value: -10,
        fill_color_argb: -1,
        tooltip: "tooltip & <tag>".into(),
        only_master_ticks: true,
        panel1_display: false,
        panel2_display: true,
        modable: false,
        radial_display: false,
        absolute_timing: true,
        restrict_to_me: true,
        restrict_to_category: true,
        active_in_list: false,
        start_sound_data: "none".into(),
        warning_sound_data: "none".into(),
    };
    let revision = host
        .save_timer_definition(page.revision, Some(&item.key), &definition)
        .await
        .expect("edit and disable")
        .revision;
    let edited = host.timer_definitions(0, 16).await.expect("edited");
    assert_eq!(
        edited.retained_frames, 1,
        "disabling/renaming must not cancel old countdowns"
    );
    assert_eq!(
        edited.items.first().expect("edited row").definition,
        definition
    );
    host.save_timer_definition(page.revision, Some(&item.key), &definition)
        .await
        .expect_err("stale revision");
    host.save_timer_definition(revision, None, &definition)
        .await
        .expect_err("duplicate name/category");
    write_failure(
        &host,
        &deployment,
        &edited.items[0].key,
        revision,
        &definition,
    )
    .await;
    external_change(&host, &deployment, revision).await;
    let fresh = host
        .timer_definitions(0, 16)
        .await
        .expect("external change");
    let revision = host
        .reload_timer_definitions(fresh.revision)
        .await
        .expect("restore saved definition");
    assert_eq!(
        host.timer_definitions(0, 16)
            .await
            .expect("restored")
            .retained_frames,
        1
    );
    let closed = host.handle();
    host.shutdown().await.expect("shutdown");
    closed
        .reload_timer_definitions(revision)
        .await
        .expect_err("closed worker");
    let restarted = Host::start(deployment.config()).await.expect("restart");
    assert!(matches!(
        restarted.reload_timer_definitions(revision).await,
        Err(Error::InvalidTimerRequest(_))
    ));
    let saved = restarted
        .timer_definitions(0, 16)
        .await
        .expect("persisted without frames");
    assert_eq!(saved.retained_frames, 0);
    assert_eq!(
        saved.items.first().expect("saved definition").definition,
        definition
    );
    restarted
        .delete_timer_definition(saved.revision, &saved.items[0].key)
        .await
        .expect("delete");
    restarted.shutdown().await.expect("stop");
    let empty = Host::start(deployment.config())
        .await
        .expect("restart empty");
    assert_eq!(
        empty.timer_definitions(0, 16).await.expect("empty").total,
        0
    );
    empty.shutdown().await.expect("stop empty");
}
async fn write_failure(
    host: &Host,
    deployment: &Deployment,
    key: &str,
    revision: aftercast::TimerRevision,
    definition: &TimerDefinition,
) {
    let path = deployment.state.join("SpellTimers.xml");
    let backup = deployment.state.join("SpellTimers.saved");
    let bytes = std::fs::read(&path).expect("saved settings");
    std::fs::rename(&path, &backup).expect("move owned fixture");
    std::fs::create_dir(&path).expect("block replacement in owned temporary state");
    let result = host
        .save_timer_definition(
            revision,
            Some(key),
            &TimerDefinition {
                name: "Not published".into(),
                ..definition.clone()
            },
        )
        .await;
    std::fs::remove_dir(&path).expect("remove own empty blocker");
    std::fs::rename(&backup, &path).expect("restore fixture");
    result.expect_err("write failure must be explicit");
    assert_eq!(std::fs::read(&path).expect("preserved file"), bytes);
    let page = host
        .timer_definitions(0, 16)
        .await
        .expect("unchanged registry");
    assert_eq!(page.revision, revision);
    assert_eq!(
        page.items.first().expect("preserved definition").definition,
        *definition
    );
}
async fn external_change(host: &Host, deployment: &Deployment, revision: aftercast::TimerRevision) {
    let log = deployment.directory().join("timer-input.log");
    std::fs::write(&log, "0|1|edit-timer\n").expect("synthetic input");
    host.import_log(&log)
        .await
        .expect("external plugin mutation");
    host.reload_timer_definitions(revision)
        .await
        .expect_err("unseen plugin mutation invalidates revision");
}

async fn normalized_identity(host: &Host) {
    let page = host
        .timer_definitions(0, 16)
        .await
        .expect("empty definitions");
    let definition = TimerDefinition {
        name: "İ (I)".into(),
        category: String::new(),
        ..Default::default()
    };
    let saved = host
        .save_timer_definition(page.revision, None, &definition)
        .await
        .expect("normalized save");
    let page = host
        .timer_definitions(0, 16)
        .await
        .expect("authoritative identity");
    assert_eq!(Some(&saved.item), page.items.first());
    assert_eq!(saved.item.definition.category, " General");
    host.delete_timer_definition(saved.revision, &saved.item.key)
        .await
        .expect("delete by managed key");
}
