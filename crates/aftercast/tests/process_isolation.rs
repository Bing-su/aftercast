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

#[test]
fn exiting_parent_entry() {
    let Some(state) = std::env::var_os("AFTERCAST_TEST_EXITING_PARENT") else {
        return;
    };
    // process::exit bypasses Host::drop, exercising the OS binding rather than Rust cleanup.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("parent runtime");
    runtime.block_on(async {
        let config = HostConfig::builder()
            .dotnet_root(std::env::var("DOTNET_ROOT").expect("runtime"))
            .bridge_directory(std::env::var("AFTERCAST_TEST_BRIDGE").expect("bridge"))
            .data_directory(camino::Utf8PathBuf::try_from(state).expect("state path"))
            .worker_executable(env!("CARGO_BIN_EXE_aftercast-worker"))
            .build();
        let host = Host::start(config).await.expect("nested worker");
        let state = camino::Utf8PathBuf::from(
            std::env::var("AFTERCAST_TEST_EXITING_PARENT").expect("state"),
        );
        std::fs::write(state.join("child.pid"), host.process_id().to_string()).expect("worker pid");
        let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll",
        );
        let plugin = host
            .create_plugin(
                PluginConfig::builder()
                    .path(path)
                    .type_name("Aftercast.PluginFixture.StuckInitPlugin")
                    .build(),
            )
            .await
            .expect("stuck plugin");
        let handle = host.handle();
        let _initializer = tokio::spawn(async move { handle.initialize_plugin(plugin).await });
        let deadline = Instant::now() + Duration::from_secs(5);
        while !state.join("stuck.entered").exists() {
            assert!(Instant::now() < deadline, "nested callback never started");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        std::process::exit(0);
    });
}

#[cfg(any(target_os = "linux", windows))]
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn parent_exit_cannot_leave_a_stuck_worker_holding_state(deployment: Deployment) {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: adopt the test's orphan worker so it can be reaped even when container PID 1 does not reap.
        let status =
            unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0_usize, 0_usize, 0_usize) };
        assert_eq!(status, 0);
    }
    let parent = std::process::Command::new(std::env::current_exe().expect("test parent"))
        .args(["--exact", "exiting_parent_entry", "--nocapture"])
        .env("AFTERCAST_TEST_EXITING_PARENT", &deployment.state)
        .env("AFTERCAST_TEST_BRIDGE", &deployment.bridge)
        .spawn()
        .expect("launch parent");
    let mut parent = scopeguard::guard(parent, |mut process| {
        drop(process.kill());
        drop(process.wait());
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = parent.try_wait().expect("wait for parent") {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "parent did not exit");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    #[cfg(target_os = "linux")]
    {
        let pid = std::fs::read_to_string(deployment.state.join("child.pid"))
            .expect("child pid")
            .parse::<libc::pid_t>()
            .expect("numeric pid");
        let cleanup = scopeguard::guard(pid, |pid| {
            // SAFETY: this PID is the unreaped adopted child, so failure cleanup cannot target an unrelated process.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
            // SAFETY: wait only for that recorded child; no status pointer is supplied.
            unsafe {
                libc::waitpid(pid, std::ptr::null_mut(), 0);
            }
        });
        loop {
            // SAFETY: wait only for the recorded child, with no status pointer and no blocking.
            if unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) } == pid {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "parent exit left the plugin worker alive"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        scopeguard::ScopeGuard::into_inner(cleanup);
    }
    loop {
        match Host::start(deployment.config()).await {
            Ok(host) => {
                host.shutdown().await.expect("new worker cleanup");
                break;
            }
            Err(Error::StateInUse(_)) => {
                assert!(
                    Instant::now() < deadline,
                    "parent exit retained the state lock"
                );
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            Err(error) => panic!("state reuse failed after parent exit: {error}"),
        }
    }
}
