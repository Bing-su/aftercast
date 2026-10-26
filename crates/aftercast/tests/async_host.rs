mod common;

use std::time::Duration;

use aftercast::{Error, Host, HostHandle, PluginConfig};
use camino::Utf8Path;
use common::{Deployment, deployment};

#[test]
fn handles_are_send_and_sync() {
    fn check<T: Send + Sync>() {}
    check::<HostHandle>();
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn imports_cancel_restore_and_shutdown_without_caller_pumping(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("start");
    let handle = host.handle();
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = handle
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.ImportPlugin")
                .build(),
        )
        .await
        .expect("fixture");
    handle.initialize_plugin(plugin).await.expect("initialize");
    let input = deployment.directory().join("long.log");
    std::fs::write(&input, "0|1\n".repeat(100_000)).expect("long source");
    let job = handle.start_import(&input).await.expect("admitted import");
    assert!(matches!(
        handle.start_import(&input).await,
        Err(Error::Busy)
    ));
    assert!(matches!(
        handle.deinitialize_plugin(plugin).await,
        Err(Error::Busy)
    ));
    // A long-running import must leave the owner available for diagnostics and progress publication.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while job.progress().records_dispatched == 0 {
        handle
            .diagnostics()
            .await
            .expect("owner responsive during import");
        assert!(std::time::Instant::now() < deadline, "no import progress");
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert!(job.progress().bytes_read <= job.progress().total_bytes);
    job.cancel();
    assert!(matches!(job.wait().await, Err(Error::Cancelled)));
    assert!(
        handle
            .live_combat()
            .await
            .expect("restored live state")
            .is_none()
    );
    std::fs::write(&input, "0|100\n2|300\n").expect("recovery source");
    assert_eq!(
        handle
            .import_log(&input)
            .await
            .expect("reuse after cancellation")
            .damage,
        400
    );
    std::fs::write(&input, "0|1\n".repeat(100_000)).expect("shutdown source");
    let job = handle
        .start_import(&input)
        .await
        .expect("import before shutdown");
    tokio::time::timeout(Duration::from_secs(5), host.shutdown())
        .await
        .expect("shutdown deadline")
        .expect("shutdown");
    assert!(matches!(job.wait().await, Err(Error::Cancelled)));
    assert!(matches!(handle.diagnostics().await, Err(Error::Closed)));
    let restarted = Host::start(deployment.config())
        .await
        .expect("restart after joined cleanup");
    restarted.shutdown().await.expect("finish restarted host");
}
