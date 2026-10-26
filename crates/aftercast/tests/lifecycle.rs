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
