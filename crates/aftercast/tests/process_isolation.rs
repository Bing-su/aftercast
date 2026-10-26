mod common;

use aftercast::{Error, Host, HostConfig, PluginConfig};
use camino::Utf8Path;
use common::{Deployment, deployment};
use std::time::{Duration, Instant};

fn config(deployment: &Deployment) -> HostConfig {
    let mut config = deployment.config();
    config.shutdown_timeout = Duration::from_millis(150);
    config
}
async fn entered(deployment: &Deployment) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !deployment.state.join("stuck.entered").exists() {
        assert!(
            Instant::now() < deadline,
            "fixture never entered its nonreturning callback"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[rstest::rstest]
#[case("StuckInitPlugin")]
#[case("StuckPostedPlugin")]
#[case("StuckImportPlugin")]
#[case("StuckActionPlugin")]
#[case("StuckDeinitPlugin")]
#[case("StuckDisposePlugin")]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn nonreturning_callbacks_are_killed_reaped_and_restartable(
    deployment: Deployment,
    #[case] name: &str,
) {
    let host = Host::start(config(&deployment))
        .await
        .expect("start worker");
    let handle = host.handle();
    let pid = handle.process_id();
    assert_ne!(pid, std::process::id());
    let session = handle.session_id().await.expect("session");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name(format!("Aftercast.PluginFixture.{name}"))
                .build(),
        )
        .await
        .expect("plugin");
    let initializer = tokio::spawn({
        let handle = handle.clone();
        async move { handle.initialize_plugin(plugin).await }
    });
    let mut job = None;
    if name != "StuckInitPlugin" {
        initializer
            .await
            .expect("initializer task")
            .expect("initialized");
        if name == "StuckImportPlugin" || name == "StuckActionPlugin" {
            let input = deployment.directory().join("input.log");
            std::fs::write(&input, "0|1\n").expect("input");
            job = Some(host.start_import(input).await.expect("start import"));
        }
    } else {
        // Keep the blocked response alive and verify termination resolves it below.
        entered(&deployment).await;
        let started = Instant::now();
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(3), host.shutdown())
                .await
                .expect("shutdown bounded"),
            Err(Error::ShutdownTimeout(_))
        ));
        assert!(started.elapsed() >= Duration::from_millis(150));
        tokio::time::timeout(Duration::from_secs(1), initializer)
            .await
            .expect("request released")
            .expect("initializer task")
            .expect_err("blocked initializer must fail after worker termination");
        finish(&deployment, handle, pid, session).await;
        return;
    }
    if name != "StuckDeinitPlugin" && name != "StuckDisposePlugin" {
        entered(&deployment).await;
    }
    let pending = tokio::spawn({
        let handle = handle.clone();
        async move { handle.diagnostics().await }
    });
    let result = tokio::time::timeout(Duration::from_secs(3), host.shutdown())
        .await
        .expect("shutdown bounded");
    assert!(
        matches!(result, Err(Error::ShutdownTimeout(_))),
        "{result:?}"
    );
    if name == "StuckDeinitPlugin" || name == "StuckDisposePlugin" {
        entered(&deployment).await;
    }
    drop(
        tokio::time::timeout(Duration::from_secs(1), pending)
            .await
            .expect("pending request released")
            .expect("query task"),
    );
    if let Some(job) = job {
        tokio::time::timeout(Duration::from_secs(1), job.wait())
            .await
            .expect("import released")
            .expect_err("blocked import must fail after worker termination");
    }
    finish(&deployment, handle, pid, session).await;
}

async fn finish(deployment: &Deployment, handle: aftercast::HostHandle, pid: u32, session: u64) {
    assert!(matches!(
        handle.diagnostics().await,
        Err(Error::Closed | Error::WorkerFailed(_))
    ));
    #[cfg(target_os = "linux")]
    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "worker must be reaped before shutdown returns"
    );
    let restarted = Host::start(config(deployment))
        .await
        .expect("restart using the same state directory");
    assert_ne!(restarted.process_id(), pid);
    assert_ne!(
        restarted.session_id().await.expect("fresh session"),
        session
    );
    restarted.shutdown().await.expect("graceful shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:plugins"]
async fn hosts_are_isolated_but_cannot_share_a_state_directory(deployment: Deployment) {
    let host = Host::start(config(&deployment))
        .await
        .expect("first worker");
    let independent = common::deployment("independent");
    let other = Host::start(independent.config())
        .await
        .expect("independent worker");
    assert_ne!(host.process_id(), other.process_id());
    assert_ne!(
        host.session_id().await.expect("first session"),
        other.session_id().await.expect("independent session")
    );
    assert!(
        Host::start(config(&deployment)).await.is_err(),
        "state ownership must be exclusive across processes"
    );
    host.shutdown().await.expect("first cleanup");
    other
        .diagnostics()
        .await
        .expect("other worker survives first shutdown");
    other.shutdown().await.expect("independent cleanup");
    Host::start(config(&deployment))
        .await
        .expect("state lock released")
        .shutdown()
        .await
        .expect("second cleanup");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn dropping_host_still_kills_a_stuck_worker(deployment: Deployment) {
    let host = Host::start(config(&deployment)).await.expect("worker");
    let handle = host.handle();
    let pid = handle.process_id();
    let session = handle.session_id().await.expect("session");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.StuckInitPlugin")
                .build(),
        )
        .await
        .expect("plugin");
    let pending = tokio::spawn({
        let handle = handle.clone();
        async move { handle.initialize_plugin(plugin).await }
    });
    entered(&deployment).await;
    drop(host);
    tokio::time::timeout(Duration::from_secs(3), pending)
        .await
        .expect("Drop must enforce its deadline")
        .expect("initializer task")
        .expect_err("killed callback must fail");
    // A response can observe socket closure just before wait() reaps the child.
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match Host::start(config(&deployment)).await {
            Ok(restarted) => {
                restarted.shutdown().await.expect("restart after Drop");
                break;
            }
            Err(Error::StateInUse(_)) => {
                assert!(Instant::now() < deadline, "Drop leaked state ownership");
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            Err(error) => panic!("restart failed: {error}"),
        }
    }
    finish(&deployment, handle, pid, session).await;
}

#[cfg(unix)]
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 deployment paths; run mise run test:plugins"]
async fn startup_deadline_covers_a_worker_that_never_authenticates(deployment: Deployment) {
    use std::os::unix::fs::PermissionsExt;
    let worker = deployment.directory().join("stalled-worker");
    // exec keeps the sleeping process as the direct child supervised by aftercast.
    std::fs::write(&worker, "#!/bin/sh\nexec sleep 60\n").expect("stalled executable");
    std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o700))
        .expect("executable mode");
    let config = HostConfig::builder()
        .dotnet_root(std::env::var("DOTNET_ROOT").expect("runtime"))
        .bridge_directory(&deployment.bridge)
        .data_directory(&deployment.state)
        .worker_executable(worker)
        .startup_timeout(Duration::from_millis(100))
        .build();
    let error = tokio::time::timeout(Duration::from_secs(3), Host::start(config))
        .await
        .expect("startup bounded")
        .err()
        .expect("unresponsive startup fails");
    assert!(error.to_string().contains("deadline"), "{error}");
    Host::start(deployment.config())
        .await
        .expect("normal startup still works")
        .shutdown()
        .await
        .expect("cleanup");
}
