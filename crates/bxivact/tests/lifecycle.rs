use bxivact::{Error, Host, HostConfig, PluginConfig, PluginId, extract_bridge};
use camino::{Utf8Path, Utf8PathBuf};

fn construct(host: &mut Host, assembly: &Utf8Path, name: &str) -> PluginId {
    host.create_plugin(
        PluginConfig::builder()
            .path(assembly)
            .type_name(format!("Bxivact.PluginFixture.{name}"))
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
}

#[test]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
fn initialization_cleanup_and_shutdown_are_session_owned_and_not_retried() {
    let root = std::env::var("DOTNET_ROOT").expect("set DOTNET_ROOT");
    let temporary = tempfile::tempdir().expect("create lifecycle fixture");
    let directory = Utf8PathBuf::try_from(temporary.path().to_owned()).expect("UTF-8 fixture path");
    let bridge = directory.join("bridge");
    let state = directory.join("state 한글");
    extract_bridge(&bridge).expect("extract bridge");
    let config = || {
        HostConfig::builder()
            .dotnet_root(&root)
            .bridge_directory(&bridge)
            .data_directory(&state)
            .build()
    };
    let assembly = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Bxivact.PluginFixture.dll")
        .canonicalize_utf8()
        .expect("build fixtures first");
    let mut host = Host::start(config()).expect("start lifecycle host");
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
    let error = host
        .stop()
        .expect_err("report the failing shutdown callback");
    assert!(error.to_string().contains("fixture shutdown failure"));
    assert!(error.to_string().contains("fixture disposal failure"));
    for mode in ["long", "deinit"] {
        assert_eq!(lines(&state, &format!("{mode}.deinit")), 1);
        assert_eq!(lines(&state, &format!("{mode}.disposed")), 2);
    }
    let mut restarted = Host::start(config()).expect("failed cleanup releases the host session");
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
