use std::env;

use aftercast::{Error, Host, HostConfig, extract_bridge};
use camino::Utf8Path;
#[path = "support/mod.rs"]
mod support;

fn config(root: &Utf8Path, bridge: &Utf8Path) -> HostConfig {
    HostConfig::builder()
        .application_executable(support::application())
        .dotnet_root(root)
        .data_directory(bridge.join("state"))
        .bridge_directory(bridge)
        .build()
}

#[rstest::rstest]
fn extraction_preserves_existing_directories() {
    let temporary = tempfile::tempdir().expect("create isolated extraction fixture");
    let directory = Utf8Path::from_path(temporary.path())
        .expect("UTF-8 test directory")
        .join("bridge");
    extract_bridge(&directory).expect("extract bundled bridge");
    // C# payloads must stay independent of the removed F# runtime and its deployment assets.
    assert!(!directory.join("FSharp.Core.dll").exists());
    assert!(!directory.join("FSharp.Core.LICENSE").exists());
    assert!(
        !std::fs::read_to_string(directory.join("Aftercast.Bridge.deps.json"))
            .expect("read managed dependencies")
            .contains("FSharp.Core")
    );
    #[cfg(target_os = "linux")]
    assert_eq!(
        std::fs::read(directory.join("System.Windows.Forms.dll"))
            .expect("read compatibility assembly"),
        aftercast_winforms::ASSEMBLY
    );
    #[cfg(target_os = "linux")]
    assert_eq!(
        std::fs::read(directory.join("System.Windows.Forms.DataVisualization.dll"))
            .expect("read chart compatibility assembly"),
        aftercast_winforms::DATAVISUALIZATION_ASSEMBLY
    );
    #[cfg(windows)]
    assert!(!directory.join("System.Windows.Forms.dll").exists());
    #[cfg(windows)]
    assert!(
        !directory
            .join("System.Windows.Forms.DataVisualization.dll")
            .exists()
    );
    let assembly = directory.join("Aftercast.Bridge.dll");
    let expected = std::fs::read(&assembly).expect("read extracted assembly");
    let error = extract_bridge(&directory).expect_err("existing directories must be preserved");
    match error {
        Error::Io { source, .. } => assert_eq!(source.kind(), std::io::ErrorKind::AlreadyExists),
        other => panic!("unexpected extraction error: {other}"),
    }
    assert_eq!(
        std::fs::read(assembly).expect("existing assembly remains"),
        expected
    );
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires a .NET 10 installation; set DOTNET_ROOT and run mise run test:hosting"]
async fn managed_lifecycle_releases_ownership_after_errors_stop_and_drop() {
    let root =
        env::var("DOTNET_ROOT").expect("set a UTF-8 DOTNET_ROOT to a trusted .NET 10 installation");
    let root = Utf8Path::new(&root);
    let temporary = tempfile::tempdir().expect("create isolated bridge fixture");
    let temporary_path = Utf8Path::from_path(temporary.path()).expect("UTF-8 test directory");
    let bridge = temporary_path.join("bridge 한글");
    extract_bridge(&bridge).expect("extract prebuilt managed component");

    let missing_root = temporary_path.join("missing-runtime");
    match Host::start(config(&missing_root, &bridge)).await {
        Err(Error::Io { .. }) => (),
        _ => panic!("missing runtime must fail before acquiring a managed session"),
    }

    let invalid_bridge = temporary_path.join("invalid-bridge");
    std::fs::create_dir(&invalid_bridge).expect("create invalid configuration fixture");
    std::fs::write(
        invalid_bridge.join("Aftercast.Bridge.runtimeconfig.json"),
        "not JSON",
    )
    .expect("write invalid runtime configuration");
    match Host::start(config(root, &invalid_bridge)).await {
        Err(Error::WorkerFailed(_)) => (),
        _ => panic!("invalid runtime configuration must report a hosting failure"),
    }

    let blocked_state = temporary_path.join("blocked-state");
    std::fs::write(&blocked_state, b"preserve existing state path")
        .expect("create blocked state fixture");
    let blocked = HostConfig::builder()
        .application_executable(support::application())
        .dotnet_root(root)
        .bridge_directory(&bridge)
        .data_directory(&blocked_state)
        .build();
    assert!(matches!(Host::start(blocked).await, Err(Error::Io { .. })));
    assert_eq!(
        std::fs::read(&blocked_state).expect("existing state path remains"),
        b"preserve existing state path"
    );

    let host = Host::start(config(root, &bridge))
        .await
        .expect("state setup failure closes the session and releases the host lease");
    let first = host.session_id().await.expect("managed call succeeds");
    assert_ne!(first, 0);
    match Host::start(config(root, &bridge)).await {
        Err(Error::StateInUse(_)) => (),
        _ => panic!("a second worker must not share an active state directory"),
    }
    host.shutdown().await.expect("explicit shutdown succeeds");

    let restarted = Host::start(config(root, &bridge))
        .await
        .expect("restart after shutdown");
    assert_ne!(restarted.session_id().await.expect("fresh session"), first);
    restarted
        .shutdown()
        .await
        .expect("joined cleanup before immediate restart");
    Host::start(config(root, &bridge))
        .await
        .expect("drop also releases the managed session")
        .shutdown()
        .await
        .expect("final shutdown");
}
