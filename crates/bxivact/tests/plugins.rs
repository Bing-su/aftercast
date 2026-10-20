use std::path::Path;

use bxivact::{Error, Host, HostConfig, PluginConfig, extract_bridge};

fn create(host: &mut Host, path: &Path, name: &str) -> bxivact::PluginId {
    host.create_plugin(PluginConfig::builder().path(path).type_name(name).build())
        .expect("construct the selected fixture")
}

fn failure(host: &mut Host, path: &Path, name: Option<&str>, expected: &str) {
    let error = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .maybe_type_name(name.map(str::to_owned))
                .build(),
        )
        .expect_err("invalid fixture must report a managed error");
    assert!(matches!(&error, Error::Managed { command: 4, .. }));
    assert!(
        error.to_string().contains(expected),
        "expected {expected:?}, received {error}"
    );
    assert_ne!(host.session_id().expect("failure preserves the session"), 0);
}

#[test]
#[ignore = "requires .NET 10 and built F# fixtures; run mise run test:plugins"]
fn constructs_trusted_plugins_without_initialization_and_recovers_from_errors() {
    let root = std::env::var_os("DOTNET_ROOT").expect("set DOTNET_ROOT");
    let temporary = tempfile::tempdir().expect("create isolated plugin test");
    let bridge = temporary.path().join("bridge");
    extract_bridge(&bridge).expect("extract bundled bridge");
    let config = || {
        HostConfig::builder()
            .dotnet_root(&root)
            .bridge_directory(&bridge)
            .build()
    };
    let mut host = Host::start(config()).expect("start managed component");
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Bxivact.PluginFixture.dll")
        .canonicalize()
        .expect("build the plugin fixture first");
    let good = "Bxivact.PluginFixture.GoodPlugin";
    let first = create(&mut host, &path, good);
    let second = create(&mut host, &path, good);
    assert_ne!(first, second);
    failure(&mut host, &path, None, "specify type_name");
    failure(
        &mut host,
        &path,
        Some("Bxivact.PluginFixture.ThrowingPlugin"),
        "fixture constructor failure",
    );
    failure(
        &mut host,
        &path,
        Some("Bxivact.PluginFixture.NeedsArgument"),
        "parameterless constructor",
    );
    failure(
        &mut host,
        &path,
        Some("Bxivact.PluginFixture.GenericPlugin`1"),
        "parameterless constructor",
    );
    failure(&mut host, &path, Some("Missing.Plugin"), "select type");
    #[cfg(target_os = "linux")]
    failure(
        &mut host,
        &path,
        Some("Bxivact.PluginFixture.UiPlugin"),
        "Linux WinForms UI is not implemented",
    );

    let isolated = temporary.path().join("isolated");
    std::fs::create_dir(&isolated).expect("create missing dependency fixture");
    let missing = isolated.join("Bxivact.PluginFixture.dll");
    std::fs::copy(&path, &missing).expect("copy without its dependency");
    // An invalid private ACT copy must never replace the bridge's shared contract assembly.
    std::fs::write(
        isolated.join("Advanced Combat Tracker.dll"),
        b"invalid private ACT",
    )
    .expect("create conflicting contract fixture");
    failure(&mut host, &missing, Some(good), "Bxivact.PluginDependency");
    std::fs::copy(
        path.with_file_name("Bxivact.PluginDependency.dll"),
        isolated.join("Bxivact.PluginDependency.dll"),
    )
    .expect("restore the dependency");
    create(&mut host, &missing, good);
    #[cfg(target_os = "linux")]
    {
        // Replace by rename so the CLR's mapped original image is never modified in place.
        let mut changed = std::fs::read(&path).expect("read the fixture image");
        changed.push(0);
        let replacement = isolated.join("replacement.dll");
        std::fs::write(&replacement, changed).expect("prepare a changed main image");
        std::fs::rename(&replacement, &missing).expect("atomically replace the main image");
        failure(&mut host, &missing, Some(good), "restart the process");
    }
    assert_ne!(create(&mut host, &path, good), first);
    host.stop()
        .expect("never-initialized plugins do not receive DeInitPlugin");
    let mut restarted = Host::start(config()).expect("restart with cached assemblies");
    assert_ne!(create(&mut restarted, &path, good), first);
    restarted.stop().expect("stop restarted session");
}
