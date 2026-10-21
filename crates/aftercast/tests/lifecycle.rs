mod common;

use aftercast::{Error, Host, PluginConfig, PluginId};
use camino::Utf8Path;
use common::{Deployment, deployment};

fn construct(host: &mut Host, assembly: &Utf8Path, name: &str) -> PluginId {
    host.create_plugin(
        PluginConfig::builder()
            .path(assembly)
            .type_name(format!("Aftercast.PluginFixture.{name}"))
            .build(),
    )
    .expect("construct lifecycle fixture")
}

fn initialization_failure(host: &mut Host, plugin: PluginId, expected: &str) {
    let error = host
        .initialize_plugin(plugin)
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

fn stop_with_expected_failures(host: Host, state: &Utf8Path) {
    let error = host
        .stop()
        .expect_err("report the failing shutdown callback");
    assert!(error.to_string().contains("fixture shutdown failure"));
    assert!(error.to_string().contains("fixture disposal failure"));
    for mode in ["long", "deinit"] {
        assert_eq!(lines(state, &format!("{mode}.deinit")), 1);
        assert_eq!(lines(state, &format!("{mode}.disposed")), 2);
    }
}

fn failed_initializations(host: &mut Host, assembly: &Utf8Path, state: &Utf8Path) {
    for (name, mode, expected) in [
        (
            "InitFailurePlugin",
            "throw",
            "fixture initialization failure",
        ),
        ("LoggedFailurePlugin", "logged", "fixture caught failure"),
    ] {
        let failed = construct(host, assembly, name);
        initialization_failure(host, failed, expected);
        initialization_failure(host, failed, "only be attempted once");
        assert!(host.deinitialize_plugin(failed).is_err());
        assert_eq!(lines(state, &format!("{mode}.init")), 1);
        assert_eq!(lines(state, &format!("{mode}.disposed")), 2);
        assert!(!state.join(format!("{mode}.deinit")).exists());
    }
    let background = construct(host, assembly, "BackgroundLogPlugin");
    assert_eq!(
        host.initialize_plugin(background)
            .expect("unrelated background logs do not fail initialization"),
        "Ready"
    );
    host.deinitialize_plugin(background)
        .expect("deinitialize background fixture");
    let before = host.diagnostics().expect("read worker diagnostics");
    let burst = construct(host, assembly, "DiagnosticBurstPlugin");
    assert_eq!(
        host.initialize_plugin(burst)
            .expect("initialize burst fixture"),
        "Ready"
    );
    let diagnostics = host
        .diagnostics()
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
        host.diagnostics().expect("reread without consuming"),
        diagnostics
    );
    host.deinitialize_plugin(burst)
        .expect("deinitialize diagnostic fixture");
}

#[cfg(target_os = "linux")]
fn owner_dispatch(host: &mut Host, assembly: &Utf8Path, state: &Utf8Path) {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::time::Duration;

    let plugin = construct(host, assembly, "PumpPlugin");
    assert_eq!(
        host.initialize_plugin(plugin)
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
        host.pump().expect("pump owner callback");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(state.join("explicit.pumped").exists());
    append("deinitialize");
    wait("deinitialize.queued");
    host.deinitialize_plugin(plugin)
        .expect("drain queued callback before deinitialization");
    assert!(state.join("deinitialize.returned").exists());
    assert!(state.join("pump.deinit").exists());
}

#[rstest::rstest]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
fn initialization_cleanup_and_shutdown_are_session_owned_and_not_retried(
    #[with("state 한글")] deployment: Deployment,
) {
    let state = deployment.directory().join("state 한글");
    let config = || deployment.config();
    let assembly = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll")
        .canonicalize_utf8()
        .expect("build fixtures first");
    let mut host = Host::start(config()).expect("start lifecycle host");
    assert_eq!(host.diagnostics().expect("host without plugins").total, 0);
    let ready = construct(&mut host, &assembly, "ReadyPlugin");
    assert!(host.deinitialize_plugin(ready).is_err());
    assert_eq!(
        host.initialize_plugin(ready).expect("initialize fixture"),
        "Ready"
    );
    initialization_failure(&mut host, ready, "only be attempted once");
    host.deinitialize_plugin(ready)
        .expect("deinitialize fixture");
    assert!(host.deinitialize_plugin(ready).is_err());
    assert_eq!(lines(&state, "ready.init"), 1);
    assert_eq!(lines(&state, "ready.deinit"), 1);
    assert_eq!(lines(&state, "ready.disposed"), 2);
    #[cfg(target_os = "linux")]
    owner_dispatch(&mut host, &assembly, &state);
    failed_initializations(&mut host, &assembly, &state);
    let long = construct(&mut host, &assembly, "LongStatusPlugin");
    assert_eq!(
        host.initialize_plugin(long).expect("bounded status").len(),
        2048
    );
    let shutdown = construct(&mut host, &assembly, "ShutdownFailurePlugin");
    assert_eq!(
        host.initialize_plugin(shutdown)
            .expect("initialize shutdown fixture"),
        "Ready"
    );
    stop_with_expected_failures(host, &state);
    let mut restarted = Host::start(config()).expect("failed cleanup releases the host session");
    assert_eq!(
        restarted
            .diagnostics()
            .expect("fresh diagnostic history")
            .total,
        0
    );
    assert!(matches!(
        restarted.initialize_plugin(ready),
        Err(Error::InvalidPluginRequest(_))
    ));
    let fresh = construct(&mut restarted, &assembly, "ReadyPlugin");
    assert_eq!(
        restarted
            .initialize_plugin(fresh)
            .expect("old ACT globals were cleared"),
        "Ready"
    );
    drop(restarted);
    assert_eq!(lines(&state, "ready.deinit"), 2);
    assert_eq!(lines(&state, "ready.disposed"), 4);
}
