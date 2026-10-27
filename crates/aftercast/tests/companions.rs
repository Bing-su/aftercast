mod common;

use aftercast::{CompanionSource, Error, Host, LiveLogOptions, PluginConfig, SourceImportOptions};
use camino::Utf8Path;
use common::{Deployment, deployment};

async fn parser(host: &Host, live: bool) {
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name(if live {
                    "Aftercast.PluginFixture.LiveCombatPlugin"
                } else {
                    "Aftercast.PluginFixture.ImportPlugin"
                })
                .build(),
        )
        .await
        .expect("fixture plugin");
    host.initialize_plugin(plugin)
        .await
        .expect("fixture parser");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn explicit_import_merges_sources_and_preserves_single_source_defaults(
    deployment: Deployment,
) {
    let main = deployment.directory().join("main.log");
    let companion = deployment.directory().join("CompanionLogs");
    std::fs::create_dir(&companion).expect("synthetic companions");
    std::fs::write(&main, "0|10\n3|20\n").expect("main");
    std::fs::write(companion.join("main.Alpha.log"), "1|100\n3|200\n9|999\n").expect("companion");
    let host = Host::start(deployment.config())
        .await
        .expect("isolated worker");
    let catalog = host
        .companion_candidates(&main)
        .await
        .expect("bounded catalog before parser");
    assert_eq!(
        catalog
            .candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>(),
        ["Alpha"]
    );
    parser(&host, false).await;
    let report = host
        .import_sources(
            &main,
            SourceImportOptions {
                viewpoint: None,
                companions: vec![CompanionSource {
                    name: "Alpha".into(),
                    text: None,
                }],
                ..Default::default()
            },
        )
        .await
        .expect("merge");
    assert_eq!(
        (
            report.records_dispatched,
            report.damage,
            report.companion_heads_excluded
        ),
        (4, 330, 1)
    );
    let explicit = host
        .import_sources(&main, SourceImportOptions::default())
        .await
        .expect("explicit single main");
    let legacy = host.import_log(&main).await.expect("legacy single main");
    assert_eq!(explicit, legacy);
    assert_eq!((legacy.records_dispatched, legacy.damage), (2, 30));
    host.shutdown().await.expect("join worker");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn live_selection_reopen_close_and_session_ownership(deployment: Deployment) {
    let main = deployment.directory().join("tail.log");
    std::fs::write(&main, "").expect("synthetic tail");
    let host = Host::start(deployment.config())
        .await
        .expect("isolated live worker");
    parser(&host, true).await;
    let first = host
        .open_live_logs(&main, LiveLogOptions::default())
        .await
        .expect("open single main");
    assert_eq!(first.sources.len(), 1);
    assert_eq!(
        host.live_logs(first.selection).await.expect("status").path,
        main
    );
    let second = host
        .reopen_live_logs(first.selection, &main, LiveLogOptions::default())
        .await
        .expect("replace capture");
    assert_ne!(first.selection, second.selection);
    host.live_logs(first.selection)
        .await
        .expect_err("stale selection");
    host.close_live_logs(second.selection)
        .await
        .expect("close readers");
    host.live_logs(second.selection)
        .await
        .expect_err("closed selection");
    let third = host
        .open_live_logs(&main, LiveLogOptions::default())
        .await
        .expect("open again");
    host.shutdown().await.expect("stop worker");
    let restarted = Host::start(deployment.config())
        .await
        .expect("new worker session");
    assert!(matches!(
        restarted.live_logs(third.selection).await,
        Err(Error::InvalidCompanionRequest(_))
    ));
    restarted.shutdown().await.expect("stop restarted worker");
}

async fn progress(host: &aftercast::HostHandle, job: &aftercast::ImportJob) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while job.progress().records_dispatched == 0 {
        host.diagnostics().await.expect("responsive import");
        assert!(std::time::Instant::now() < deadline, "no import progress");
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn source_import_cancel_drop_shutdown_and_failure_release_owned_inputs(
    deployment: Deployment,
) {
    use std::time::Duration;
    let main = deployment.directory().join("large.log");
    std::fs::write(&main, "1|1\n".repeat(100_000)).expect("long synthetic input");
    let host = Host::start(deployment.config()).await.expect("worker");
    parser(&host, false).await;
    let handle = host.handle();
    let job = host
        .start_import_sources(&main, SourceImportOptions::default())
        .await
        .expect("admit");
    progress(&handle, &job).await;
    job.cancel();
    assert!(matches!(job.wait().await, Err(Error::Cancelled)));
    assert!(
        !host
            .encounters(0, 100)
            .await
            .expect("partial graph")
            .items
            .is_empty()
    );
    std::fs::write(&main, "0|7\n").expect("retry");
    let job = host
        .start_import_sources(&main, SourceImportOptions::default())
        .await
        .expect("drop job");
    drop(job); // An observer's drop must not cancel an admitted import.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match host
            .import_sources(&main, SourceImportOptions::default())
            .await
        {
            Err(Error::Busy) => {
                assert!(std::time::Instant::now() < deadline, "dropped job stuck");
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            result => {
                assert_eq!(result.expect("reuse after drop").damage, 7);
                break;
            }
        }
    }
    std::fs::write(&main, [0xff]).expect("invalid bytes");
    host.import_sources(&main, SourceImportOptions::default())
        .await
        .expect_err("visible decoding failure");
    std::fs::write(&main, "1|1\n".repeat(100_000)).expect("shutdown input");
    let job = host
        .start_import_sources(&main, SourceImportOptions::default())
        .await
        .expect("active shutdown");
    progress(&handle, &job).await;
    tokio::time::timeout(Duration::from_secs(5), host.shutdown())
        .await
        .expect("shutdown deadline")
        .expect("shutdown");
    assert!(matches!(job.wait().await, Err(Error::Cancelled)));
    assert!(matches!(
        handle
            .import_sources(&main, SourceImportOptions::default())
            .await,
        Err(Error::Closed)
    ));
    let restarted = Host::start(deployment.config())
        .await
        .expect("lease released");
    restarted.shutdown().await.expect("restart shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn live_clock_failure_and_external_generation_are_observable(deployment: Deployment) {
    use std::{
        io::Write,
        time::{Duration, Instant},
    };
    let main = deployment.directory().join("clock.log");
    let sides = deployment.directory().join("CompanionLogs");
    std::fs::create_dir(&sides).expect("sides");
    std::fs::write(&main, "").expect("main");
    let side = sides.join("clock.Alpha.log");
    std::fs::write(&side, "").expect("side");
    let host = Host::start(deployment.config()).await.expect("worker");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.CompanionPlugin")
                .build(),
        )
        .await
        .expect("synthetic plugin");
    host.initialize_plugin(plugin).await.expect("initialize");
    let selection = host
        .open_live_logs(
            &main,
            LiveLogOptions {
                companions: vec![CompanionSource {
                    name: "Alpha".into(),
                    text: None,
                }],
                max_clock_wait: Duration::from_millis(50),
                ..Default::default()
            },
        )
        .await
        .expect("clock selection");
    let mut append = std::fs::OpenOptions::new()
        .append(true)
        .open(&side)
        .expect("append side");
    append.write_all(b"3600|100\n").expect("future head");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let state = host
            .live_logs(selection.selection)
            .await
            .expect("live diagnostics");
        let row = state.sources.get(1).expect("companion status");
        if row.state == aftercast::LiveLogState::Failed {
            assert!(!row.last_error.is_empty());
            break;
        }
        assert!(Instant::now() < deadline, "clock timeout absent: {state:?}");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let reopened = host
        .reopen_live_logs(selection.selection, &main, LiveLogOptions::default())
        .await
        .expect("recover single");
    let mut append = std::fs::OpenOptions::new()
        .append(true)
        .open(&main)
        .expect("append main");
    append
        .write_all(b"3|1|external\n")
        .expect("external root reopen");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !deployment.state.join("companion.external").exists() {
        host.diagnostics().await.expect("pump external callback");
        assert!(Instant::now() < deadline, "external callback absent");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    host.live_logs(reopened.selection)
        .await
        .expect_err("external generation invalidates owner");
    let fresh = host
        .open_live_logs(&main, LiveLogOptions::default())
        .await
        .expect("fresh after external open");
    host.close_live_logs(fresh.selection)
        .await
        .expect("close fresh");
    host.end_combat().await.expect("close leaves host usable");
    host.shutdown().await.expect("shutdown");
}

#[cfg(target_os = "linux")]
fn exclusive(file: &std::fs::File) {
    use std::os::fd::AsRawFd;
    assert_eq!(
        // SAFETY: flock borrows this live fixture fd without transferring ownership.
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0,
        "reader handle leaked: {}",
        std::io::Error::last_os_error()
    );
}

#[cfg(target_os = "linux")]
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn partial_live_open_failure_cleans_readers_and_drop_stops_worker(deployment: Deployment) {
    use std::time::{Duration, Instant};
    let main = deployment.directory().join("partial.log");
    let sides = deployment.directory().join("CompanionLogs");
    std::fs::create_dir(&sides).expect("sides");
    let alpha = sides.join("partial.Alpha.log");
    let locked = sides.join("partial.Zlocked.log");
    for path in [&main, &alpha, &locked] {
        std::fs::write(path, "").expect("source");
    }
    let host = Host::start(deployment.config()).await.expect("worker");
    parser(&host, true).await;
    let first = host
        .open_live_logs(&main, LiveLogOptions::default())
        .await
        .expect("first selection");
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&locked)
        .expect("exclusive synthetic source");
    exclusive(&lock);
    let options = LiveLogOptions {
        companions: vec![
            CompanionSource {
                name: "Alpha".into(),
                text: None,
            },
            CompanionSource {
                name: "Zlocked".into(),
                text: None,
            },
        ],
        ..Default::default()
    };
    host.reopen_live_logs(first.selection, &main, options)
        .await
        .expect_err("later source open fails");
    host.live_logs(first.selection)
        .await
        .expect_err("failed reopen retires old owner");
    // Every earlier opened source must have been joined/disposed before the failure returns.
    for path in [&main, &alpha] {
        let file = std::fs::File::open(path).expect("probe source");
        exclusive(&file);
    }
    drop(lock);
    let reopened = host
        .open_live_logs(
            &main,
            LiveLogOptions {
                companions: vec![CompanionSource {
                    name: "Alpha".into(),
                    text: None,
                }],
                ..Default::default()
            },
        )
        .await
        .expect("retry after cleanup");
    let handle = host.handle();
    drop(host);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if matches!(
            handle.live_logs(reopened.selection).await,
            Err(Error::Closed)
        ) {
            break;
        }
        assert!(Instant::now() < deadline, "drop did not stop worker");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    // The data lease is released only after worker exit, not when a cloned handle observes Closed.
    loop {
        match Host::start(deployment.config()).await {
            Ok(next) => {
                next.shutdown().await.expect("restart shutdown");
                break;
            }
            Err(error) => {
                assert!(Instant::now() < deadline, "worker retained lease: {error}");
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
    }
    for path in [&main, &alpha] {
        let file = std::fs::File::open(path).expect("probe stopped source");
        exclusive(&file);
    }
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn case_preserving_live_mapping_and_extensionless_range_import(deployment: Deployment) {
    let main = deployment.directory().join("extensionless");
    let sides = deployment.directory().join("CompanionLogs");
    std::fs::create_dir(&sides).expect("sides");
    std::fs::write(&main, "").expect("main");
    std::fs::write(sides.join("extensionless.Älpha"), "").expect("unicode side");
    std::fs::write(sides.join("extensionless.Zulu"), "").expect("Zulu side");
    let host = Host::start(deployment.config()).await.expect("worker");
    parser(&host, true).await;
    let companions = vec![
        CompanionSource {
            name: "Zulu".into(),
            text: None,
        },
        CompanionSource {
            name: "älpha".into(),
            text: None,
        },
    ];
    let snapshot = host
        .open_live_logs(
            &main,
            LiveLogOptions {
                companions: companions.clone(),
                ..Default::default()
            },
        )
        .await
        .expect("extensionless Unicode catalog");
    assert!(
        snapshot
            .sources
            .iter()
            .any(|source| source.selected_name == "älpha" && source.name == "Älpha")
    );
    assert!(
        snapshot
            .sources
            .iter()
            .any(|source| source.selected_name == "Zulu" && source.name == "Zulu")
    );
    host.close_live_logs(snapshot.selection)
        .await
        .expect("close selection");
    let trailing_dot = deployment.directory().join("extensionless.");
    std::fs::write(&trailing_dot, "").expect("empty-extension main");
    let empty_extension = host
        .open_live_logs(
            &trailing_dot,
            LiveLogOptions {
                companions: companions.clone(),
                ..Default::default()
            },
        )
        .await
        .expect(".NET empty-extension companion paths");
    host.close_live_logs(empty_extension.selection)
        .await
        .expect("close empty extension");
    std::fs::write(&main, "0|10\n3|20\n").expect("main range");
    std::fs::write(sides.join("extensionless.Älpha"), "1|100\n3|200\n9|999\n").expect("side range");
    // BOM decoding belongs to this companion only; the main remains UTF-8.
    let utf16 = std::iter::once(0xfeff_u16)
        .chain("3|300|日本語\n".encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    std::fs::write(sides.join("extensionless.Zulu"), utf16).expect("wide companion");
    let ticks = 621_355_968_000_000_000 + 30_000_000;
    let range = aftercast::ImportRange::new(ticks, ticks).expect("inclusive same-time boundary");
    let options = SourceImportOptions {
        viewpoint: None,
        range: Some(range),
        companions,
        ..Default::default()
    };
    let report = host
        .import_sources(&main, options)
        .await
        .expect("range per-source BOM");
    assert_eq!((report.records_dispatched, report.damage), (3, 520));
    host.shutdown().await.expect("shutdown");
}
