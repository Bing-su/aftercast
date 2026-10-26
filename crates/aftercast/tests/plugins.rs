mod common;

use aftercast::{Error, Host, PluginConfig};
use camino::Utf8Path;
use common::{Deployment, deployment};

async fn create(host: &mut Host, path: &Utf8Path, name: &str) -> aftercast::PluginId {
    host.create_plugin(PluginConfig::builder().path(path).type_name(name).build())
        .await
        .expect("construct the selected fixture")
}

async fn failure(host: &mut Host, path: &Utf8Path, name: Option<&str>, expected: &str) {
    let error = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .maybe_type_name(name.map(str::to_owned))
                .build(),
        )
        .await
        .expect_err("invalid fixture must report a managed error");
    assert!(matches!(&error, Error::Managed { command: 4, .. }));
    assert!(
        error.to_string().contains(expected),
        "expected {expected:?}, received {error}"
    );
    assert_ne!(
        host.session_id()
            .await
            .expect("failure preserves the session"),
        0
    );
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and built C# fixtures; run mise run test:plugins"]
async fn constructs_trusted_plugins_without_initialization_and_recovers_from_errors(
    deployment: Deployment,
) {
    let temporary_path = deployment.directory();
    let config = || deployment.config();
    let mut host = Host::start(config())
        .await
        .expect("start managed component");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll")
        .canonicalize_utf8()
        .expect("build the plugin fixture first");
    let good = "Aftercast.PluginFixture.GoodPlugin";
    let first = create(&mut host, &path, good).await;
    let second = create(&mut host, &path, good).await;
    assert_ne!(first, second);
    create(&mut host, &path, "Aftercast.PluginFixture.InternalPlugin").await;
    failure(
        &mut host,
        &path,
        Some("Aftercast.PluginFixture.PrivateConstructorPlugin"),
        "public parameterless constructor",
    )
    .await;
    failure(&mut host, &path, None, "specify type_name").await;
    failure(
        &mut host,
        &path,
        Some("Aftercast.PluginFixture.ThrowingPlugin"),
        "fixture constructor failure",
    )
    .await;
    failure(
        &mut host,
        &path,
        Some("Aftercast.PluginFixture.NeedsArgument"),
        "parameterless constructor",
    )
    .await;
    failure(
        &mut host,
        &path,
        Some("Aftercast.PluginFixture.GenericPlugin`1"),
        "parameterless constructor",
    )
    .await;
    failure(&mut host, &path, Some("Missing.Plugin"), "select type").await;
    #[cfg(target_os = "linux")]
    failure(
        &mut host,
        &path,
        Some("Aftercast.PluginFixture.UiPlugin"),
        "Linux WinForms native handles are not implemented",
    )
    .await;

    let isolated = temporary_path.join("isolated 플러그인");
    std::fs::create_dir(&isolated).expect("create missing dependency fixture");
    let missing = isolated.join("Aftercast.PluginFixture.dll");
    std::fs::copy(&path, &missing).expect("copy without its dependency");
    // An invalid private ACT copy must never replace the bridge's shared contract assembly.
    std::fs::write(
        isolated.join("Advanced Combat Tracker.dll"),
        b"invalid private ACT",
    )
    .expect("create conflicting contract fixture");
    failure(
        &mut host,
        &missing,
        Some(good),
        "Aftercast.PluginDependency",
    )
    .await;
    std::fs::copy(
        path.with_file_name("Aftercast.PluginDependency.dll"),
        isolated.join("Aftercast.PluginDependency.dll"),
    )
    .expect("restore the dependency");
    create(&mut host, &missing, good).await;
    #[cfg(target_os = "linux")]
    {
        // Replace by rename so the CLR's mapped original image is never modified in place.
        let mut changed = std::fs::read(&path).expect("read the fixture image");
        changed.push(0);
        let replacement = isolated.join("replacement.dll");
        std::fs::write(&replacement, changed).expect("prepare a changed main image");
        std::fs::rename(&replacement, &missing).expect("atomically replace the main image");
        failure(&mut host, &missing, Some(good), "restart the process").await;
    }
    assert_ne!(create(&mut host, &path, good).await, first);
    host.shutdown()
        .await
        .expect("never-initialized plugins do not receive DeInitPlugin");
    let mut restarted = Host::start(config())
        .await
        .expect("restart with cached assemblies");
    assert_ne!(create(&mut restarted, &path, good).await, first);
    restarted.shutdown().await.expect("stop restarted session");
}
