mod common;

use aftercast::{
    CompanionSource, EncounterStatus, Error, HistoryCaptureOptions, HistoryQuery,
    HistorySourceQuery, Host, ImportRange, PluginConfig, SourceImportOptions,
};
use camino::Utf8Path;
use common::{Deployment, deployment};
use std::time::Duration;

async fn setup(deployment: &Deployment) -> Host {
    let host = Host::start(deployment.config())
        .await
        .expect("isolated worker");
    let parser = host.create_plugin(PluginConfig::builder()
        .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll"))
        .type_name("Aftercast.PluginFixture.BoundaryImportPlugin").build()).await.expect("synthetic fixture");
    host.initialize_plugin(parser)
        .await
        .expect("initialize parser");
    host
}
fn range(extend: bool) -> ImportRange {
    let epoch = 621_355_968_000_000_000;
    ImportRange::new(epoch + 10_000_000, epoch + 20_000_000)
        .expect("inclusive range")
        .with_combat_end_extension(extend)
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn admitted_policy_is_owned_and_preserves_act_preparse_boundaries(deployment: Deployment) {
    let main = deployment.directory().join("main.log");
    let host = setup(&deployment).await;
    for (source, strict, extended) in [
        (
            "0|N\n1|A|10\n2|A|20\n2|A|5\n3|A|30\n4|E\n5|A|999\n1|A|999\n",
            (3, 35, 1),
            (5, 65, 1),
        ),
        ("1|A|10\n20|A|100\n21|E\n22|N\n", (1, 10, 1), (3, 110, 2)),
        (
            "1|A|10\n3|E\n1|A|20\n9|E\n10|N\n1|A|999\n",
            (1, 10, 1),
            (4, 30, 2),
        ),
        ("0|N\n3|A|999\n", (0, 0, 0), (0, 0, 0)),
        ("1|A|10\n3|A|20\n", (1, 10, 1), (2, 30, 1)),
        ("", (0, 0, 0), (0, 0, 0)),
    ] {
        std::fs::write(&main, source).expect("synthetic rows");
        let mut caller = range(true);
        let job = host
            .start_import_range(&main, caller)
            .await
            .expect("snapshot admission");
        caller = caller.with_combat_end_extension(false);
        let actual = job.wait().await.expect("extended import");
        assert_eq!(
            (actual.records_dispatched, actual.damage, actual.encounters),
            extended,
            "{source}"
        );
        let actual = host
            .import_log_range(&main, caller)
            .await
            .expect("next strict request");
        assert_eq!(
            (actual.records_dispatched, actual.damage, actual.encounters),
            strict,
            "{source}"
        );
    }
    std::fs::write(&main, "0|A|7\n3|A|11\n").expect("file bounds");
    let report = host
        .import_log_range(
            &main,
            ImportRange::from_bounds(None, None, false).expect("file begin/end"),
        )
        .await
        .expect("whole file effective bounds");
    assert_eq!((report.records_dispatched, report.damage), (2, 18));
    host.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn companion_extension_keeps_accepted_heads_and_next_request_strict(deployment: Deployment) {
    let main = deployment.directory().join("main.log");
    let sides = deployment.directory().join("CompanionLogs");
    std::fs::create_dir(&sides).expect("synthetic companions");
    std::fs::write(&main, "1|A|10\n4|E\n5|N\n").expect("main");
    std::fs::write(sides.join("main.Alpha.log"), "2|A|100\n3|A|200\n5|A|999\n").expect("side");
    let host = setup(&deployment).await;
    for (extend, expected) in [(true, (4, 310)), (false, (2, 110))] {
        let report = host
            .import_sources(
                &main,
                SourceImportOptions {
                    viewpoint: None,
                    range: Some(range(extend)),
                    companions: vec![CompanionSource {
                        name: "Alpha".into(),
                        text: None,
                    }],
                    ..Default::default()
                },
            )
            .await
            .expect("owned source range");
        assert_eq!((report.records_dispatched, report.damage), expected);
    }
    host.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[case(false)]
#[case(true)]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn cancel_and_extended_decoder_failure_retain_partial_graph_and_release_job(
    deployment: Deployment,
    #[case] companion: bool,
) {
    let host = setup(&deployment).await;
    let main = deployment.directory().join("cancel.log");
    // Only synthetic callbacks wait on these markers. Cancellation is requested after a real action.
    std::fs::write(&main, "1|A|1\n".repeat(20_000)).expect("cancel input");
    let job = if companion {
        host.start_import_sources(
            &main,
            SourceImportOptions {
                viewpoint: None,
                range: Some(range(true)),
                ..Default::default()
            },
        )
        .await
    } else {
        host.start_import_range(&main, range(true)).await
    }
    .expect("admit cancel");
    tokio::time::timeout(Duration::from_secs(5), async {
        while !deployment.state.join("boundary.ready").exists() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("callback gate reached");
    job.cancel();
    std::fs::write(deployment.state.join("boundary.release"), "release").expect("release fixture");
    assert!(matches!(job.wait().await, Err(Error::Cancelled)));
    let page = host.encounters(0, 10).await.expect("cancelled graph");
    assert!(
        page.items
            .iter()
            .any(|item| item.status == EncounterStatus::PartialImport)
    );
    let main = deployment.directory().join("broken.log");
    // A malformed row in the extended suffix is reached and reported, after accepted damage.
    let mut bytes = b"1|A|10\n3|A|20\n".to_vec();
    bytes.extend_from_slice(&[0xff, b'\n']);
    std::fs::write(&main, bytes).expect("invalid extended bytes");
    let failed = if companion {
        host.import_sources(
            &main,
            SourceImportOptions {
                viewpoint: None,
                range: Some(range(true)),
                ..Default::default()
            },
        )
        .await
    } else {
        host.import_log_range(&main, range(true)).await
    };
    assert!(matches!(failed, Err(Error::Managed { .. })));
    let page = host.encounters(0, 100).await.expect("failed partial graph");
    assert!(
        page.items
            .iter()
            .any(|item| item.status == EncounterStatus::PartialImport && item.damage == 30)
    );
    std::fs::write(&main, "1|A|7\n3|A|11\n").expect("retry");
    assert_eq!(
        host.import_log_range(&main, range(false))
            .await
            .expect("reusable after failure")
            .damage,
        7
    );
    host.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixtures; run mise run test:plugins"]
async fn one_shot_history_capture_keeps_exact_saved_bounds(deployment: Deployment) {
    let host = setup(&deployment).await;
    let main = deployment.directory().join("main.log");
    std::fs::write(&main, "1|A|10\n2|A|20\n").expect("seed");
    host.import_log(&main).await.expect("seed import");
    assert_eq!(host.save_history().await.expect("save checkpoint"), 1);
    std::fs::write(&main, "1|A|10\n2|A|20\n3|A|30\n4|E\n5|N\n").expect("current selected source");
    assert_eq!(
        host.import_log_range(&main, range(true))
            .await
            .expect("raw extended policy")
            .damage,
        60
    );
    let page = host
        .query_history(&HistoryQuery::default())
        .await
        .expect("history");
    let mut proposal = host
        .propose_history_sources(
            &page.revision,
            &page.items.first().expect("saved").id,
            HistorySourceQuery {
                paths: vec![main.clone()],
                ..Default::default()
            },
        )
        .await
        .expect("explicit candidate");
    tokio::time::timeout(Duration::from_secs(10), async {
        while !host
            .poll_history_sources(&mut proposal)
            .await
            .expect("poll proposal")
        {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("proposal deadline");
    let mut capture = host
        .capture_history_source(&proposal, &main, HistoryCaptureOptions::default())
        .await
        .expect("capture");
    tokio::time::timeout(Duration::from_secs(10), async {
        while host
            .poll_history_capture(&mut capture)
            .await
            .expect("poll capture")
            .is_some()
        {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("capture deadline");
    assert_eq!(capture.range(), Some(range(false)));
    let job = host
        .reparse_history_capture(&mut capture)
        .await
        .expect("one-shot exact replay");
    assert!(matches!(
        host.reparse_history_capture(&mut capture).await,
        Err(Error::InvalidHistoryRequest(_))
    ));
    let report = job.wait().await.expect("replay");
    assert_eq!((report.records_dispatched, report.damage), (2, 30));
    host.release_history_sources(&mut proposal)
        .await
        .expect("release");
    host.shutdown().await.expect("shutdown");
}
