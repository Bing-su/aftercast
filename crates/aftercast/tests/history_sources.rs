mod common;
use aftercast::{
    CompanionSource, Error, HistoryCaptureOptions, HistoryQuery, HistorySourceCapture,
    HistorySourceProposal, HistorySourceQuery, Host, PluginConfig,
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
        .type_name("Aftercast.PluginFixture.ImportPlugin").build()).await.expect("fixture");
    host.initialize_plugin(parser).await.expect("parser");
    host
}
async fn proposed(host: &Host, path: &Utf8Path) -> HistorySourceProposal {
    let page = host
        .query_history(&HistoryQuery::default())
        .await
        .expect("history page");
    let mut proposal = host
        .propose_history_sources(
            &page.revision,
            &page.items.first().expect("saved history").id,
            HistorySourceQuery {
                paths: vec![
                    path.parent()
                        .expect("parent")
                        .join("missing/../")
                        .join(path.file_name().expect("filename")),
                ],
                ..Default::default()
            },
        )
        .await
        .expect("explicit proposal");
    tokio::time::timeout(Duration::from_secs(10), async {
        while !host
            .poll_history_sources(&mut proposal)
            .await
            .expect("source poll")
        {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("bounded source discovery");
    proposal
}
async fn ready(host: &Host, capture: &mut HistorySourceCapture) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while host
            .poll_history_capture(capture)
            .await
            .expect("capture poll")
            .is_some()
        {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("bounded capture");
}
async fn seed(host: &Host, path: &Utf8Path) {
    std::fs::write(path, "1|10\n3|20\n").expect("synthetic main");
    host.import_log(path).await.expect("seed encounter");
    assert_eq!(host.save_history().await.expect("checkpoint"), 1);
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:plugins"]
async fn frozen_capture_replays_once_after_original_replacement(deployment: Deployment) {
    let host = setup(&deployment).await;
    let main = deployment.directory().join("main.log");
    seed(&host, &main).await;
    let sides = deployment.directory().join("CompanionLogs");
    std::fs::create_dir(&sides).expect("companions");
    let side = sides.join("MAIN.Alpha.LOG");
    std::fs::write(&side, "1|100\n3|200\n9|999\n").expect("side");
    let mut proposal = proposed(&host, &main).await;
    let mut capture = host
        .capture_history_source(
            &proposal,
            &main,
            HistoryCaptureOptions {
                companions: vec![CompanionSource {
                    name: "ALPHA".into(),
                    text: None,
                }],
                ..Default::default()
            },
        )
        .await
        .expect("capture admission");
    ready(&host, &mut capture).await;
    let foreign = Host::start(
        aftercast::HostConfig::builder()
            .application_executable(common::application())
            .dotnet_root(std::env::var("DOTNET_ROOT").expect("trusted runtime"))
            .bridge_directory(&deployment.bridge)
            .data_directory(deployment.directory().join("foreign"))
            .build(),
    )
    .await
    .expect("foreign session");
    assert!(matches!(
        foreign.release_history_capture(&mut capture).await,
        Err(Error::InvalidHistoryRequest(_))
    ));
    foreign.shutdown().await.expect("foreign worker stop");
    assert!(matches!(
        host.release_history_sources(&mut proposal).await,
        Err(Error::Managed { .. })
    ));
    assert_eq!(capture.files().expect("ready files").len(), 2);
    assert!(
        capture
            .files()
            .expect("files")
            .iter()
            .all(|file| file.sha256.len() == 64)
    );
    std::fs::write(&main, "9|9999\n").expect("replace original");
    std::fs::remove_file(&side).expect("delete synthetic side");
    let job = host
        .reparse_history_capture(&mut capture)
        .await
        .expect("one shot transfer");
    assert!(matches!(
        host.reparse_history_capture(&mut capture).await,
        Err(Error::InvalidHistoryRequest(_))
    ));
    let report = job.wait().await.expect("drained replay");
    assert_eq!(
        (
            report.records_dispatched,
            report.damage,
            report.companion_heads_excluded
        ),
        (4, 330, 1)
    );
    host.release_history_sources(&mut proposal)
        .await
        .expect("release proposal");
    // Existing source import remains usable after captured import relinquishes ownership.
    assert_eq!(
        host.import_log(&main)
            .await
            .expect("legacy single source")
            .damage,
        9999
    );
    host.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:plugins"]
async fn failures_releases_stale_history_and_restart_remain_explicit(deployment: Deployment) {
    let host = setup(&deployment).await;
    let main = deployment.directory().join("main.log");
    seed(&host, &main).await;
    let mut proposal = proposed(&host, &main).await;
    let mut failed = host
        .capture_history_source(
            &proposal,
            &main,
            HistoryCaptureOptions {
                maximum_bytes: 1,
                ..Default::default()
            },
        )
        .await
        .expect("budget request");
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match host.poll_history_capture(&mut failed).await {
                Err(Error::Managed { .. }) => break,
                Ok(Some(_)) => tokio::time::sleep(Duration::from_millis(1)).await,
                other => panic!("unexpected budget result: {other:?}"),
            }
        }
    })
    .await
    .expect("budget failure");
    assert!(matches!(
        host.release_history_capture(&mut failed).await,
        Err(Error::Managed { .. })
    ));
    let mut capture = host
        .capture_history_source(&proposal, &main, HistoryCaptureOptions::default())
        .await
        .expect("retry after release");
    ready(&host, &mut capture).await;
    let index = deployment.state.join("history/index.xml");
    let original = std::fs::read(&index).expect("synthetic index");
    let mut changed = original.clone();
    changed.push(b' ');
    std::fs::write(&index, changed).expect("change revision");
    assert!(matches!(
        host.reparse_history_capture(&mut capture).await,
        Err(Error::Managed { .. })
    ));
    // Failed preflight preserves a ready handle for explicit disposal.
    host.release_history_capture(&mut capture)
        .await
        .expect("dispose preserved image");
    std::fs::write(&index, original).expect("restore fixture index");
    host.shutdown().await.expect("first worker shutdown");
    let fresh = setup(&deployment).await;
    assert!(matches!(
        fresh.poll_history_sources(&mut { proposal }).await,
        Err(Error::InvalidHistoryRequest(_))
    ));
    let mut current = proposed(&fresh, &main).await;
    fresh
        .release_history_sources(&mut current)
        .await
        .expect("fresh proposal release");
    fresh.shutdown().await.expect("fresh worker shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:plugins"]
async fn pending_cancel_drop_and_active_replay_shutdown_close_worker(deployment: Deployment) {
    let host = setup(&deployment).await;
    let main = deployment.directory().join("main.log");
    seed(&host, &main).await;
    let mut proposal = proposed(&host, &main).await;
    let mut capture = host
        .capture_history_source(&proposal, &main, HistoryCaptureOptions::default())
        .await
        .expect("pending capture");
    host.release_history_capture(&mut capture)
        .await
        .expect("cancel or release before next request");
    let mut capture = host
        .capture_history_source(&proposal, &main, HistoryCaptureOptions::default())
        .await
        .expect("reopen");
    ready(&host, &mut capture).await;
    let job = host
        .reparse_history_capture(&mut capture)
        .await
        .expect("admitted replay");
    job.cancel();
    assert!(matches!(job.wait().await, Err(Error::Cancelled) | Ok(_)));
    host.release_history_sources(&mut proposal)
        .await
        .expect("release");
    let proposal = proposed(&host, &main).await;
    let capture = host
        .capture_history_source(&proposal, &main, HistoryCaptureOptions::default())
        .await
        .expect("retained capture");
    drop(capture);
    drop(proposal); // Drop retains bounded managed ownership; shutdown joins it.
    host.shutdown().await.expect("drain dropped capture");
    let fresh = setup(&deployment).await;
    let proposal = proposed(&fresh, &main).await;
    let mut capture = fresh
        .capture_history_source(&proposal, &main, HistoryCaptureOptions::default())
        .await
        .expect("new worker capture");
    ready(&fresh, &mut capture).await;
    let job = fresh
        .reparse_history_capture(&mut capture)
        .await
        .expect("active import");
    fresh
        .shutdown()
        .await
        .expect("active captured import shutdown");
    assert!(matches!(job.wait().await, Ok(_) | Err(Error::Cancelled)));
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:plugins"]
async fn replay_input_failure_retains_partial_graph_and_dropped_admission_drains(
    deployment: Deployment,
) {
    use std::future::Future;
    use std::task::Poll;
    let host = setup(&deployment).await;
    let main = deployment.directory().join("main.log");
    seed(&host, &main).await;
    std::fs::write(&main, "1|100\n2|bad\n3|200\n").expect("synthetic bad parser row");
    let mut proposal = proposed(&host, &main).await;
    let mut capture = host
        .capture_history_source(&proposal, &main, HistoryCaptureOptions::default())
        .await
        .expect("bad input image");
    ready(&host, &mut capture).await;
    let job = host
        .reparse_history_capture(&mut capture)
        .await
        .expect("admission");
    assert!(matches!(job.wait().await, Err(Error::Managed { .. })));
    assert!(host.encounters(0, 16).await.expect("partial graph").total >= 2);
    host.release_history_sources(&mut proposal)
        .await
        .expect("release after failed parser");
    std::fs::write(&main, "1|10\n3|20\n").expect("restore synthetic source");
    assert_eq!(
        host.import_log(&main)
            .await
            .expect("import after failure")
            .damage,
        30
    );
    let proposal = proposed(&host, &main).await;
    let mut future =
        Box::pin(host.capture_history_source(&proposal, &main, HistoryCaptureOptions::default()));
    // Poll once so admission enters the bounded Host queue, then drop only the response future.
    std::future::poll_fn(|cx| {
        drop(future.as_mut().poll(cx));
        Poll::Ready(())
    })
    .await;
    drop(future);
    host.diagnostics().await.expect("admission barrier");
    host.shutdown()
        .await
        .expect("drain response-less source request");
}
