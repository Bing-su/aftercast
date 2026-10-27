mod common;

use aftercast::{Error, Host, PluginConfig, PluginId};
use camino::Utf8Path;
use common::{Deployment, deployment};

async fn construct(host: &mut Host, assembly: &Utf8Path, name: &str) -> PluginId {
    host.create_plugin(
        PluginConfig::builder()
            .path(assembly)
            .type_name(format!("Aftercast.PluginFixture.{name}"))
            .build(),
    )
    .await
    .expect("construct lifecycle fixture")
}

async fn initialization_failure(host: &mut Host, plugin: PluginId, expected: &str) {
    let error = host
        .initialize_plugin(plugin)
        .await
        .expect_err("initialization must fail");
    assert!(
        error.to_string().contains(expected),
        "expected {expected:?}, got {error}"
    );
}

fn lines(directory: &Utf8Path, name: &str) -> usize {
    std::fs::read_to_string(directory.join(name))
        .expect("read callback evidence")
        .lines()
        .count()
}

async fn stop_with_expected_failures(host: Host, state: &Utf8Path) {
    let error = host
        .shutdown()
        .await
        .expect_err("report the failing shutdown callback");
    assert!(error.to_string().contains("fixture shutdown failure"));
    assert!(error.to_string().contains("fixture disposal failure"));
    for mode in ["long", "deinit"] {
        assert_eq!(lines(state, &format!("{mode}.deinit")), 1);
        assert_eq!(lines(state, &format!("{mode}.disposed")), 2);
    }
}

async fn failed_initializations(host: &mut Host, assembly: &Utf8Path, state: &Utf8Path) {
    for (name, mode, expected) in [
        (
            "InitFailurePlugin",
            "throw",
            "fixture initialization failure",
        ),
        ("LoggedFailurePlugin", "logged", "fixture caught failure"),
    ] {
        let failed = construct(host, assembly, name).await;
        initialization_failure(host, failed, expected).await;
        initialization_failure(host, failed, "only be attempted once").await;
        assert!(host.deinitialize_plugin(failed).await.is_err());
        assert_eq!(lines(state, &format!("{mode}.init")), 1);
        assert_eq!(lines(state, &format!("{mode}.disposed")), 2);
        assert!(!state.join(format!("{mode}.deinit")).exists());
    }
    background_diagnostics(host, assembly).await;
}

async fn background_diagnostics(host: &mut Host, assembly: &Utf8Path) {
    let background = construct(host, assembly, "BackgroundLogPlugin").await;
    assert_eq!(
        host.initialize_plugin(background)
            .await
            .expect("unrelated background logs do not fail initialization"),
        "Ready"
    );
    host.deinitialize_plugin(background)
        .await
        .expect("deinitialize background fixture");
    let before = host.diagnostics().await.expect("read worker diagnostics");
    let burst = construct(host, assembly, "DiagnosticBurstPlugin").await;
    assert_eq!(
        host.initialize_plugin(burst)
            .await
            .expect("initialize burst fixture"),
        "Ready"
    );
    let diagnostics = host
        .diagnostics()
        .await
        .expect("read bounded diagnostic snapshot");
    assert_eq!(diagnostics.total, before.total + 40);
    assert_eq!(diagnostics.recent.len(), 32);
    assert_eq!(diagnostics.omitted(), diagnostics.total - 32);
    assert!(
        diagnostics
            .recent
            .first()
            .expect("oldest retained message")
            .starts_with("burst-8:")
    );
    assert!(
        diagnostics
            .recent
            .last()
            .expect("newest retained message")
            .starts_with("burst-39:")
    );
    assert!(
        diagnostics
            .recent
            .iter()
            .all(|text| text.contains("한글\0") && text.len() <= 3 * 8192)
    );
    assert_eq!(
        host.diagnostics().await.expect("reread without consuming"),
        diagnostics
    );
    host.deinitialize_plugin(burst)
        .await
        .expect("deinitialize diagnostic fixture");
}

async fn owner_dispatch(host: &mut Host, assembly: &Utf8Path, state: &Utf8Path) {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::time::Duration;

    let plugin = construct(host, assembly, "PumpPlugin").await;
    assert_eq!(
        host.initialize_plugin(plugin)
            .await
            .expect("initialize pumped plugin"),
        "Ready"
    );
    let append = |line: &str| {
        writeln!(
            OpenOptions::new()
                .append(true)
                .open(state.join("pump.log"))
                .expect("open fixture log"),
            "{line}"
        )
        .expect("append fixture line");
    };
    let wait = |name: &str| {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !state.join(name).exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "missing dispatch evidence: {name}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    append("explicit");
    wait("explicit.queued");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !state.join("explicit.returned").exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "explicit pump did not release worker"
        );
        host.session_id().await.expect("pump owner callback");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(state.join("explicit.pumped").exists());
    append("deinitialize");
    wait("deinitialize.queued");
    host.deinitialize_plugin(plugin)
        .await
        .expect("drain queued callback before deinitialization");
    assert!(state.join("deinitialize.returned").exists());
    assert!(state.join("pump.deinit").exists());
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn initialization_cleanup_and_shutdown_are_session_owned_and_not_retried(
    #[with("state 한글")] deployment: Deployment,
) {
    let state = deployment.directory().join("state 한글");
    let config = || deployment.config();
    let assembly = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll")
        .canonicalize_utf8()
        .expect("build fixtures first");
    let mut host = Host::start(config()).await.expect("start lifecycle host");
    assert!(
        state.join("Config").is_dir(),
        "prepare the ACT settings directory"
    );
    assert_eq!(
        host.diagnostics()
            .await
            .expect("host without plugins")
            .total,
        0
    );
    let ready = construct(&mut host, &assembly, "ReadyPlugin").await;
    assert!(host.deinitialize_plugin(ready).await.is_err());
    assert_eq!(
        host.initialize_plugin(ready)
            .await
            .expect("initialize fixture"),
        "Ready"
    );
    initialization_failure(&mut host, ready, "only be attempted once").await;
    host.deinitialize_plugin(ready)
        .await
        .expect("deinitialize fixture");
    assert!(host.deinitialize_plugin(ready).await.is_err());
    assert_eq!(lines(&state, "ready.init"), 1);
    assert_eq!(lines(&state, "ready.deinit"), 1);
    assert_eq!(lines(&state, "ready.disposed"), 2);
    owner_dispatch(&mut host, &assembly, &state).await;
    failed_initializations(&mut host, &assembly, &state).await;
    let long = construct(&mut host, &assembly, "LongStatusPlugin").await;
    assert_eq!(
        host.initialize_plugin(long)
            .await
            .expect("bounded status")
            .len(),
        2048
    );
    let shutdown = construct(&mut host, &assembly, "ShutdownFailurePlugin").await;
    assert_eq!(
        host.initialize_plugin(shutdown)
            .await
            .expect("initialize shutdown fixture"),
        "Ready"
    );
    stop_with_expected_failures(host, &state).await;
    let mut restarted = Host::start(config())
        .await
        .expect("failed cleanup releases the host session");
    assert_eq!(
        restarted
            .diagnostics()
            .await
            .expect("fresh diagnostic history")
            .total,
        0
    );
    assert!(matches!(
        restarted.initialize_plugin(ready).await,
        Err(Error::InvalidPluginRequest(_))
    ));
    let fresh = construct(&mut restarted, &assembly, "ReadyPlugin").await;
    assert_eq!(
        restarted
            .initialize_plugin(fresh)
            .await
            .expect("old ACT globals were cleared"),
        "Ready"
    );
    restarted
        .shutdown()
        .await
        .expect("wait for cleanup before inspecting files");
    assert_eq!(lines(&state, "ready.deinit"), 2);
    assert_eq!(lines(&state, "ready.disposed"), 4);
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn plugin_status_and_settings_respect_terminal_instances_and_session_restart(
    deployment: Deployment,
) {
    let mut host = Host::start(deployment.config()).await.expect("host");
    let assembly = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let mut previous = None;
    for _ in 0..3 {
        let id = construct(&mut host, &assembly, "ReadyPlugin").await;
        assert_eq!(
            host.plugin_status(id)
                .await
                .expect("constructed status")
                .state,
            aftercast::PluginState::Constructed
        );
        assert!(matches!(
            host.open_plugin_settings(id).await,
            Err(Error::Managed { .. })
        ));
        host.initialize_plugin(id)
            .await
            .expect("initialize fresh instance");
        let status = host.plugin_status(id).await.expect("running status");
        assert_eq!(status.state, aftercast::PluginState::Initialized);
        if cfg!(target_os = "windows") {
            assert_eq!(status.settings, aftercast::PluginSettings::Available);
            host.open_plugin_settings(id).await.expect("show settings");
            host.open_plugin_settings(id)
                .await
                .expect("focus existing window");
        } else {
            assert_eq!(
                status.settings,
                aftercast::PluginSettings::UnsupportedPlatform
            );
            let error = host
                .open_plugin_settings(id)
                .await
                .expect_err("explicit platform limit");
            assert!(error.to_string().contains("Windows"));
        }
        host.deinitialize_plugin(id).await.expect("disable");
        assert_eq!(
            host.plugin_status(id).await.expect("stopped status").state,
            aftercast::PluginState::Stopped
        );
        assert!(matches!(
            host.open_plugin_settings(id).await,
            Err(Error::Managed { .. })
        ));
        assert_ne!(previous, Some(id));
        previous = Some(id);
    }
    let old = previous.expect("old instance");
    host.shutdown().await.expect("stop owner");
    let restarted = Host::start(deployment.config())
        .await
        .expect("restart owner");
    assert!(matches!(
        restarted.plugin_status(old).await,
        Err(Error::InvalidPluginRequest(_))
    ));
    restarted
        .shutdown()
        .await
        .expect("shutdown restarted owner");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn running_toggles_preserve_worker_history_and_stop_old_callbacks(deployment: Deployment) {
    use aftercast::PluginState;

    let mut host = Host::start(deployment.config()).await.expect("host");
    let assembly = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let session = host.session_id().await.expect("original session");
    let parser = construct(&mut host, &assembly, "ImportPlugin").await;
    host.initialize_plugin(parser).await.expect("parser");
    let log = deployment.directory().join("toggle.log");
    let state = deployment.directory().join("state");
    let observed = state.join("toggle.observed");
    let parse = async |host: &mut Host, text: &str| {
        std::fs::write(&log, text).expect("synthetic log");
        host.import_log(&log)
            .await
            .expect("parser remains operational");
    };
    let mut previous = None;
    let mut first_encounter = None;
    for round in 0..3 {
        let observer = construct(&mut host, &assembly, "ToggleObserverPlugin").await;
        assert_ne!(previous, Some(observer), "enable must use a fresh instance");
        host.initialize_plugin(observer).await.expect("enable");
        let before = std::fs::read_to_string(&observed)
            .unwrap_or_default()
            .lines()
            .count();
        parse(&mut host, "0|100\n0|200\n").await;
        assert_eq!(lines(&state, "toggle.observed"), before + 2);
        if round == 0 {
            let encounter = host.encounters(0, 32).await.expect("history").items[0].clone();
            assert_eq!(encounter.damage, 300);
            first_encounter = Some(encounter.id);
        }
        host.deinitialize_plugin(observer).await.expect("disable");
        assert_eq!(
            host.plugin_status(observer).await.expect("status").state,
            PluginState::Stopped
        );
        host.initialize_plugin(observer)
            .await
            .expect_err("terminal instance");
        parse(&mut host, "1|50\n").await;
        assert_eq!(
            lines(&state, "toggle.observed"),
            before + 2,
            "no callbacks while disabled"
        );
        assert_eq!(host.session_id().await.expect("same session"), session);
        let retained = host
            .encounter_actions(first_encounter.expect("first encounter"), 0, 10)
            .await
            .expect("query existing graph")
            .expect("identity retained");
        assert_eq!(retained.total, 2);
        assert_eq!(
            retained.items[0].source,
            aftercast::SourceLogLink::Retained(0)
        );
        assert_eq!(
            retained.items[1].source,
            aftercast::SourceLogLink::Retained(1)
        );
        previous = Some(observer);
    }
    for _ in 0..2 {
        let failed = construct(&mut host, &assembly, "InitFailurePlugin").await;
        initialization_failure(&mut host, failed, "fixture initialization failure").await;
        assert_eq!(
            host.plugin_status(failed)
                .await
                .expect("failed status")
                .state,
            PluginState::Failed
        );
        assert_eq!(
            host.session_id()
                .await
                .expect("failed enable preserves session"),
            session
        );
    }
    let retry = construct(&mut host, &assembly, "ToggleObserverPlugin").await;
    host.initialize_plugin(retry)
        .await
        .expect("fresh retry without restarting parser");
    let throwing = construct(&mut host, &assembly, "ShutdownFailurePlugin").await;
    host.initialize_plugin(throwing)
        .await
        .expect("throwing fixture ready");
    assert!(host.deinitialize_plugin(throwing).await.is_err());
    assert_eq!(
        host.plugin_status(throwing)
            .await
            .expect("cleanup is terminal")
            .state,
        PluginState::Stopped
    );
    parse(&mut host, "2|25\n").await;
    assert_eq!(lines(&state, "toggle.observed"), 7);
    assert_eq!(
        host.session_id().await.expect("still original worker"),
        session
    );
    host.save_history()
        .await
        .expect("checkpoint after lifecycle failures");
    host.shutdown()
        .await
        .expect("no double deinit of failed/stopped instances");
}
