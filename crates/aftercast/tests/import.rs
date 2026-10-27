mod common;

use aftercast::{Error, Host, ImportReport, PluginConfig};
use camino::Utf8Path;
use common::{Deployment, deployment};

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn imports_drain_actions_and_recover_after_input_and_plugin_failures(deployment: Deployment) {
    let directory = deployment.directory();
    let host = Host::start(deployment.config())
        .await
        .expect("start import host");
    let log = directory.join("input 한글.log");
    std::fs::write(&log, "\u{feff}0|100\r\n\n2|300").expect("write import records");
    assert!(matches!(
        host.import_log(&log).await,
        Err(Error::Managed { command: 8, .. })
    ));
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.ImportPlugin")
                .build(),
        )
        .await
        .expect("construct import fixture plugin");
    host.initialize_plugin(plugin)
        .await
        .expect("initialize fixture parser");
    let expected = ImportReport {
        lines_read: 3,
        records_dispatched: 2,
        encounters: 1,
        combatants: 2,
        damage: 400,
        healed: 0,
    };
    let mut job = host
        .start_import(&log)
        .await
        .expect("start an observable import");
    let _progress = job.progress();
    assert_eq!(
        (&mut job)
            .await
            .expect("await job without consuming its handle"),
        expected
    );
    assert_eq!(
        host.import_log(&log)
            .await
            .expect("repeat with isolated import state"),
        expected
    );
    std::fs::write(&log, [0xff]).expect("write invalid UTF-8");
    assert!(matches!(
        host.import_log(&log).await,
        Err(Error::Managed { command: 8, .. })
    ));
    std::fs::write(&log, "not-a-timestamp|100\n").expect("write parser failure");
    assert!(matches!(
        host.import_log(&log).await,
        Err(Error::Managed { command: 8, .. })
    ));
    std::fs::write(&log, "0|100\n2|300\n").expect("restore valid input");
    assert_eq!(
        host.import_log(&log).await.expect("recover after failures"),
        ImportReport {
            lines_read: 2,
            ..expected
        }
    );
    host.deinitialize_plugin(plugin)
        .await
        .expect("deinitialize parser");
    assert!(matches!(
        host.import_log(&log).await,
        Err(Error::Managed { command: 8, .. })
    ));
    host.shutdown().await.expect("stop import host");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn source_ranges_include_equal_time_rows_and_keep_partial_results(deployment: Deployment) {
    use aftercast::{ImportRange, SourceLogLink};
    let host = Host::start(deployment.config()).await.expect("range host");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.ImportPlugin")
                .build(),
        )
        .await
        .expect("fixture");
    host.initialize_plugin(plugin).await.expect("initialize");
    let epoch = 621_355_968_000_000_000;
    let range = ImportRange::new(epoch + 10_000_000, epoch + 20_000_000).expect("range");
    let input = deployment.directory().join("range 한글.log");
    std::fs::write(&input, "\u{feff}0|100\r\n1|200\r1|300\n2|400\n3|999\n1|999")
        .expect("synthetic input");
    let report = host
        .import_log_range(&input, range)
        .await
        .expect("range import");
    assert_eq!(
        (report.lines_read, report.records_dispatched, report.damage),
        (5, 3, 900)
    );
    let graph = host.encounters(0, 16).await.expect("graph");
    let id = graph.items.first().expect("range encounter").id;
    let actions = host
        .encounter_actions(id, 0, 16)
        .await
        .expect("actions")
        .expect("encounter");
    let logs = host
        .encounter_logs(id, 0, 16)
        .await
        .expect("logs")
        .expect("encounter");
    assert_eq!(actions.items.len(), 3);
    assert_eq!(logs.page.items.len(), 3);
    for (index, (action, log)) in actions.items.iter().zip(&logs.page.items).enumerate() {
        assert_eq!(action.source, SourceLogLink::Retained(index as u32));
        assert_eq!(action.time_sorter, log.time_sorter);
    }
    assert_eq!(
        actions.items.first().expect("first").time,
        actions.items.get(1).expect("second").time
    );
    assert_ne!(
        actions.items.first().expect("first").time_sorter,
        actions.items.get(1).expect("second").time_sorter
    );
    assert_eq!(
        host.import_log_range(
            &input,
            ImportRange::new(epoch + 10_000_000, epoch + 10_000_000).expect("exact")
        )
        .await
        .expect("exact time")
        .damage,
        500
    );
    let count = host.encounters(0, 16).await.expect("before empty").total;
    assert_eq!(
        host.import_log_range(
            &input,
            ImportRange::new(epoch + 100_000_000, epoch + 110_000_000).expect("empty")
        )
        .await
        .expect("empty range")
        .encounters,
        0
    );
    assert_eq!(
        host.encounters(0, 16).await.expect("after empty").total,
        count
    );
    std::fs::write(&input, [b"1|100\n".as_slice(), &[255, 10]].concat()).expect("malformed prefix");
    host.import_log_range(&input, range)
        .await
        .expect_err("bad encoding in selected range");
    assert!(
        host.encounters(0, 16)
            .await
            .expect("partial graph")
            .items
            .iter()
            .any(
                |row| row.status == aftercast::EncounterStatus::PartialImport && row.damage == 100
            )
    );
    std::fs::write(&input, [b"1|100\n3|999\n".as_slice(), &[255, 10]].concat())
        .expect("malformed suffix");
    assert_eq!(
        host.import_log_range(&input, range)
            .await
            .expect("suffix after cutoff ignored")
            .damage,
        100
    );
    range_cancel_and_shutdown(host, &deployment, &input, range).await;
}

async fn range_cancel_and_shutdown(
    host: Host,
    deployment: &Deployment,
    input: &Utf8Path,
    range: aftercast::ImportRange,
) {
    let handle = host.handle();
    std::fs::write(input, "1|1\n".repeat(100_000)).expect("long synthetic source");
    let job = handle
        .start_import_range(input, range)
        .await
        .expect("range job");
    assert!(matches!(
        handle.start_import_range(input, range).await,
        Err(Error::Busy)
    ));
    wait_range_progress(&handle, &job).await;
    let progress = job.progress();
    assert!(progress.records_dispatched <= progress.lines_read);
    assert!(progress.bytes_read <= progress.total_bytes);
    job.cancel();
    assert!(matches!(job.wait().await, Err(Error::Cancelled)));
    assert!(
        handle
            .live_combat()
            .await
            .expect("live state restored")
            .is_none()
    );
    assert!(
        handle
            .encounters(0, 100)
            .await
            .expect("partial graph")
            .items
            .iter()
            .any(|row| row.status == aftercast::EncounterStatus::PartialImport && row.damage > 0)
    );
    std::fs::write(input, "1|100\n").expect("reuse source");
    assert_eq!(
        handle
            .import_log_range(input, range)
            .await
            .expect("reuse")
            .damage,
        100
    );
    range_shutdown(host, deployment, input, range).await;
}

async fn range_shutdown(
    host: Host,
    deployment: &Deployment,
    input: &Utf8Path,
    range: aftercast::ImportRange,
) {
    use std::time::Duration;
    let handle = host.handle();
    std::fs::write(input, "1|1\n".repeat(100_000)).expect("shutdown source");
    let job = handle
        .start_import_range(input, range)
        .await
        .expect("shutdown job");
    tokio::time::timeout(Duration::from_secs(5), host.shutdown())
        .await
        .expect("shutdown deadline")
        .expect("shutdown");
    assert!(matches!(job.wait().await, Err(Error::Cancelled)));
    assert!(matches!(
        handle.start_import_range(input, range).await,
        Err(Error::Closed)
    ));
    let next = Host::start(deployment.config())
        .await
        .expect("restart releases lease");
    next.shutdown().await.expect("fresh shutdown");
}

async fn wait_range_progress(handle: &aftercast::HostHandle, job: &aftercast::ImportJob) {
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + Duration::from_secs(5);
    while job.progress().records_dispatched == 0 {
        handle
            .diagnostics()
            .await
            .expect("responsive while importing");
        assert!(Instant::now() < deadline, "no range progress");
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}
