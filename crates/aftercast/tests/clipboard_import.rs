mod common;

use aftercast::{
    ActFormat, EncounterStatus, Error, HistoryQuery, Host, ImportJob, LogSubscriptionConfig,
    MAX_CLIPBOARD_IMPORT_BYTES, PluginConfig, SourceLogLink,
};
use camino::Utf8Path;
use common::{Deployment, deployment};
use std::{collections::BTreeMap, time::Duration};

async fn initialize(host: &Host) {
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.ClipboardImportPlugin")
                .build(),
        )
        .await
        .expect("synthetic parser");
    host.initialize_plugin(plugin).await.expect("initialize");
}
async fn setup(deployment: &Deployment) -> Host {
    let host = Host::start(deployment.config())
        .await
        .expect("isolated worker");
    initialize(&host).await;
    host
}
fn archives(deployment: &Deployment) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(deployment.state.join("history"))
        .expect("private history")
        .map(|entry| entry.expect("archive entry"))
        .filter(|entry| entry.path().extension() == Some(std::ffi::OsStr::new("act")))
        .map(|entry| {
            (
                entry.file_name().into_string().expect("owned hash name"),
                std::fs::read(entry.path()).expect("owned archive"),
            )
        })
        .collect()
}
async fn gate(deployment: &Deployment) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !deployment.state.join("clipboard.ready").exists() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("synthetic callback gate");
}
fn release(deployment: &Deployment) {
    std::fs::write(deployment.state.join("clipboard.release"), "release")
        .expect("synthetic gate release");
}
async fn assert_links(host: &Host, expected: &[(i32, String)]) {
    let page = host.encounters(0, 10).await.expect("graph");
    let encounter = page.items.first().expect("one encounter");
    assert_eq!(encounter.damage, 65);
    let logs = host
        .encounter_logs(encounter.id, 0, 10)
        .await
        .expect("logs")
        .expect("scope");
    let actions = host
        .encounter_actions(encounter.id, 0, 10)
        .await
        .expect("actions")
        .expect("scope");
    assert_eq!(logs.dropped, 0);
    assert_eq!(
        logs.page
            .items
            .iter()
            .map(|line| (line.time_sorter, line.text.clone()))
            .collect::<Vec<_>>(),
        expected
    );
    for (index, action) in actions.items.iter().enumerate() {
        assert_eq!(action.source, SourceLogLink::Retained(index as u32));
        assert_eq!(
            action.time_sorter,
            expected.get(index).expect("retained source").0
        );
    }
}

async fn public_export(host: &Host, deployment: &Deployment) {
    let id = host
        .encounters(0, 1)
        .await
        .expect("export scope")
        .items
        .first()
        .expect("clipboard encounter")
        .id;
    let path = deployment.directory().join("clipboard.xml");
    host.export_act(id, &path, ActFormat::Xml)
        .await
        .expect("ACT XML export");
    let xml = std::fs::read_to_string(path).expect("owned export");
    assert!(xml.contains("한글") && xml.contains("日本語") && xml.contains('😀'));
    assert!(
        !xml.contains("AftercastClipboardImport"),
        "private origin does not leak into ACT interchange"
    );
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn unicode_snapshot_is_pathless_and_private_archive_roundtrip_preserves_sources(
    deployment: Deployment,
) {
    let host = Host::start(deployment.config()).await.expect("worker");
    assert!(matches!(
        host.import_clipboard_text("1|1").await,
        Err(Error::Managed { .. })
    ));
    initialize(&host).await;
    let subscription = host
        .subscribe_logs(LogSubscriptionConfig::builder().build())
        .await
        .expect("original logs");
    let mut text = "\u{feff}1|10|한글\r\n\rU|20|日本語\n1|5|😀\r2|30".to_owned();
    let job = host
        .start_clipboard_import(&text)
        .await
        .expect("owned Unicode Begin");
    text.clear();
    text.push_str("bad replacement");
    let report = job.wait().await.expect("snapshot completed");
    assert_eq!(
        (report.lines_read, report.records_dispatched, report.damage),
        (5, 4, 65)
    );
    let batch = host
        .drain_logs(subscription)
        .await
        .expect("pathless records");
    assert_eq!(batch.records.len(), 4);
    assert!(
        batch
            .records
            .iter()
            .all(|record| record.is_import && record.source_path.as_str().is_empty())
    );
    let expected = batch
        .records
        .iter()
        .map(|record| {
            (
                record.time_sorter,
                std::str::from_utf8(&record.original_line)
                    .expect("Unicode")
                    .to_owned(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        expected
            .iter()
            .map(|(_, line)| line.as_str())
            .collect::<Vec<_>>(),
        ["\u{feff}1|10|한글", "U|20|日本語", "1|5|😀", "2|30"]
    );
    assert_links(&host, &expected).await;
    public_export(&host, &deployment).await;
    assert_eq!(host.save_history().await.expect("save private graph"), 1);
    let page = host
        .query_history(&HistoryQuery::default())
        .await
        .expect("history metadata");
    assert!(page.items.first().expect("entry").folder_hint.is_empty());
    let saved = archives(&deployment);
    let stale = host.handle();
    host.shutdown().await.expect("join first worker");
    assert!(matches!(
        stale.import_clipboard_text("1|9").await,
        Err(Error::Closed)
    ));
    let restarted = setup(&deployment).await;
    assert_eq!(
        restarted
            .restore_history("")
            .await
            .expect("restore clipboard checkpoint"),
        1
    );
    assert_links(&restarted, &expected).await;
    assert_eq!(
        restarted
            .save_history()
            .await
            .expect("resave provenance/log identity"),
        1
    );
    // Re-saving can normalize baseline import-zone metadata; old immutable payloads must remain intact.
    let after = archives(&deployment);
    for (name, bytes) in saved {
        assert_eq!(
            after.get(&name),
            Some(&bytes),
            "saved payload was modified or removed"
        );
    }
    assert!(
        restarted
            .query_history(&HistoryQuery::default())
            .await
            .expect("restored metadata")
            .items
            .first()
            .expect("entry")
            .folder_hint
            .is_empty()
    );
    restarted.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn unicode_budget_empty_threshold_and_initial_undated_clock_are_explicit(
    deployment: Deployment,
) {
    let host = setup(&deployment).await;
    for invalid in [
        String::new(),
        "x".repeat(MAX_CLIPBOARD_IMPORT_BYTES + 1),
        "한".repeat(MAX_CLIPBOARD_IMPORT_BYTES / 3 + 1),
    ] {
        assert!(matches!(
            host.start_clipboard_import(invalid).await,
            Err(Error::InvalidImportRequest(_))
        ));
    }
    let ascii = "1|0|".to_owned() + &"x".repeat(MAX_CLIPBOARD_IMPORT_BYTES - 4);
    let mut korean = "1|0|".to_owned() + &"한".repeat((MAX_CLIPBOARD_IMPORT_BYTES - 4) / 3);
    korean.push_str(&"x".repeat(MAX_CLIPBOARD_IMPORT_BYTES - korean.len()));
    for maximum in [ascii, korean] {
        assert_eq!(maximum.len(), MAX_CLIPBOARD_IMPORT_BYTES);
        let report = host
            .import_clipboard_text(maximum)
            .await
            .expect("exact 8 MiB through binary IPC");
        assert_eq!((report.records_dispatched, report.damage), (1, 0));
    }
    assert_eq!(
        host.import_clipboard_text("\n".repeat(1001) + "1|999")
            .await
            .expect("ACT blank stop")
            .records_dispatched,
        0
    );
    assert_eq!(
        host.import_clipboard_text("\n".repeat(1000) + "1|7")
            .await
            .expect("before blank stop")
            .damage,
        7
    );
    assert_eq!(
        host.import_clipboard_text("\r\n\r\n")
            .await
            .expect("only blank physical rows")
            .records_dispatched,
        0
    );
    let subscription = host
        .subscribe_logs(LogSubscriptionConfig::builder().build())
        .await
        .expect("clock observation");
    host.import_clipboard_text("U|11")
        .await
        .expect("first undated row");
    let batch = host.drain_logs(subscription).await.expect("undated time");
    assert_eq!(
        batch.records.first().expect("row").timestamp.ticks,
        621_355_968_000_000_000 + 864_000_000_000
    );
    host.import_clipboard_text("U|12\n1|1")
        .await
        .expect("leading undated then date");
    let batch = host.drain_logs(subscription).await.expect("leading clock");
    assert_eq!(
        batch.records.first().expect("first").timestamp.ticks,
        621_355_968_000_000_000 + 864_000_000_000
    );
    host.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn clipboard_cancel_failure_and_observer_drop_release_admission(deployment: Deployment) {
    let host = setup(&deployment).await;
    let source = "1|1|gate\n".to_owned() + &"1|1\n".repeat(20_000);
    let expected_bytes = source.len() as u64;
    let mut job = host
        .start_clipboard_import(source)
        .await
        .expect("cancel admission");
    gate(&deployment).await;
    job.cancel();
    release(&deployment);
    assert!(matches!((&mut job).await, Err(Error::Cancelled)));
    let progress = job.progress();
    assert_eq!(progress.total_bytes, expected_bytes);
    assert!(progress.bytes_read > 0 && progress.bytes_read <= progress.total_bytes);
    let graph = host.encounters(0, 100).await.expect("partial cancel graph");
    assert!(
        graph
            .items
            .iter()
            .any(|row| row.status == EncounterStatus::PartialImport && row.damage > 0)
    );
    assert!(matches!(
        host.import_clipboard_text("1|10\nbad timestamp").await,
        Err(Error::Managed { .. })
    ));
    assert!(
        host.encounters(0, 100)
            .await
            .expect("partial error")
            .items
            .iter()
            .any(|row| row.status == EncounterStatus::PartialImport && row.damage == 10)
    );
    drop(
        host.start_clipboard_import("2|7")
            .await
            .expect("observer drop"),
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match host.import_clipboard_text("3|9").await {
            Err(Error::Busy) => {
                assert!(std::time::Instant::now() < deadline, "observer drop stuck");
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            result => {
                assert_eq!(result.expect("reused after failure/drop").damage, 9);
                break;
            }
        }
    }
    // Dropping the admission future after enqueue also leaves cleanup with Host, not the caller.
    let mut admission = Box::pin(host.start_clipboard_import("4|5"));
    std::future::poll_fn(|cx| {
        drop(std::future::Future::poll(admission.as_mut(), cx));
        std::task::Poll::Ready(())
    })
    .await;
    drop(admission);
    host.diagnostics().await.expect("queued admission barrier");
    host.shutdown()
        .await
        .expect("shutdown drains response-less admission");
}

async fn gated_job(host: &Host, deployment: &Deployment) -> ImportJob {
    let job = host
        .start_clipboard_import("1|1|gate\n".to_owned() + &"1|1\n".repeat(20_000))
        .await
        .expect("gated import");
    gate(deployment).await;
    job
}
#[rstest::rstest]
#[case(false)]
#[case(true)]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn active_clipboard_shutdown_or_host_drop_drains_before_restart(
    deployment: Deployment,
    #[case] drop_host: bool,
) {
    let host = setup(&deployment).await;
    let stale = host.handle();
    let job = gated_job(&host, &deployment).await;
    if drop_host {
        drop(host);
        release(&deployment);
    } else {
        let release = async {
            tokio::time::sleep(Duration::from_millis(5)).await;
            release(&deployment);
        };
        let (result, ()) = tokio::join!(host.shutdown(), release);
        result.expect("shutdown drains admitted clipboard input");
    }
    assert!(matches!(job.wait().await, Err(Error::Cancelled)));
    assert!(matches!(
        stale.import_clipboard_text("1|99").await,
        Err(Error::Closed)
    ));
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match Host::start(deployment.config()).await {
            Ok(fresh) => {
                initialize(&fresh).await;
                assert_eq!(
                    fresh
                        .import_clipboard_text("1|9")
                        .await
                        .expect("fresh session import")
                        .damage,
                    9
                );
                fresh.shutdown().await.expect("fresh shutdown");
                break;
            }
            Err(error) => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "worker lease retained: {error}"
                );
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
    }
}
