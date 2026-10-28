mod common;
use aftercast::{Error, HistoryQuery, HistorySearchScope, Host, PluginConfig, SourceLogLink};
use camino::Utf8Path;
use common::{Deployment, deployment};
async fn parser(host: &Host) {
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let id = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.ImportPlugin")
                .build(),
        )
        .await
        .expect("parser");
    host.initialize_plugin(id).await.expect("initialize");
}
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:plugins"]
async fn history_snapshots_select_exact_archives_and_preserve_log_identity(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    assert!(
        !host
            .query_history(&HistoryQuery::default())
            .await
            .expect("cold discovery")
            .index_present
    );
    parser(&host).await;
    let input = deployment.directory().join("history.log");
    for text in ["0|100\n0|300\n", "0|200\n0|500\n"] {
        std::fs::write(&input, text).expect("synthetic input");
        host.import_log(&input).await.expect("import");
    }
    let old = host.encounters(0, 16).await.expect("source graph").items[0].id;
    assert_eq!(host.save_history().await.expect("checkpoint"), 2);
    let saved = host
        .query_history(&HistoryQuery::default())
        .await
        .expect("saved metadata");
    assert_eq!(saved.total, 2);
    assert!(saved.items.iter().all(|item| item.already_restored));
    host.shutdown().await.expect("stop");
    let host = Host::start(deployment.config()).await.expect("restart");
    let page = host
        .query_history(&HistoryQuery::default())
        .await
        .expect("cold index");
    assert!(
        page.items
            .iter()
            .all(|item| !item.already_restored && item.payload_present)
    );
    assert!(matches!(
        host.restore_selected_history(&saved.revision, &[saved.items[0].id.clone()])
            .await,
        Err(Error::InvalidHistoryRequest(_))
    ));
    host.restore_selected_history(
        &page.revision,
        &[page.items.first().expect("first history entry").id.clone()],
    )
    .await
    .expect_err("restore requires initialized parser");
    parser(&host).await;
    let first = host
        .query_history(&HistoryQuery {
            limit: 1,
            ..Default::default()
        })
        .await
        .expect("first page");
    let second = host
        .query_history(&HistoryQuery {
            offset: 1,
            limit: 1,
            ..Default::default()
        })
        .await
        .expect("second page");
    assert_eq!(first.revision, second.revision);
    assert_ne!(first.items[0].id, second.items[0].id);
    assert_eq!(first.items[0].start, second.items[0].start);
    let ticks = first.items[0].start.ticks;
    assert_eq!(
        host.query_history(&HistoryQuery {
            start_ticks: Some(ticks),
            end_ticks: Some(ticks + 1),
            ..Default::default()
        })
        .await
        .expect("date range")
        .total,
        2
    );
    assert_eq!(
        host.query_history(&HistoryQuery {
            end_ticks: Some(ticks),
            ..Default::default()
        })
        .await
        .expect("exclusive end")
        .total,
        0
    );
    assert_eq!(
        host.query_history(&HistoryQuery {
            text: "Import Zone".into(),
            scope: HistorySearchScope::Zone,
            ..Default::default()
        })
        .await
        .expect("zone scope")
        .total,
        2
    );
    assert_eq!(
        host.query_history(&HistoryQuery {
            text: "Import Zone".into(),
            scope: HistorySearchScope::Title,
            ..Default::default()
        })
        .await
        .expect("title scope")
        .total,
        0
    );
    assert!(matches!(
        host.restore_selected_history(
            &page.revision,
            &[
                page.items.first().expect("first history entry").id.clone(),
                page.items.first().expect("first history entry").id.clone()
            ]
        )
        .await,
        Err(Error::InvalidHistoryRequest(_))
    ));
    stale_index(&host, &deployment, &page).await;
    assert_eq!(
        host.restore_selected_history(&first.revision, &[first.items[0].id.clone()])
            .await
            .expect("selected restore"),
        1
    );
    assert_eq!(
        host.restore_selected_history(&first.revision, &[first.items[0].id.clone()])
            .await
            .expect("repeat skip"),
        0
    );
    let rows = host.encounters(0, 16).await.expect("restored graph");
    assert_eq!(rows.total, 1);
    let id = rows.items[0].id;
    let actions = host
        .encounter_actions(id, 1, 1)
        .await
        .expect("actions")
        .expect("encounter");
    assert_eq!(actions.items[0].source, SourceLogLink::Retained(1));
    assert!([300, 500].contains(&actions.items[0].amount));
    assert_eq!(actions.items[0].damage, actions.items[0].amount.to_string());
    assert_eq!(actions.items[0].swing_type, 1);
    assert!(!actions.items[0].critical);
    assert_eq!(actions.items[0].damage_type.as_deref(), Some("Physical"));
    let logs = host
        .encounter_logs(id, 1, 1)
        .await
        .expect("source logs")
        .expect("encounter");
    assert_eq!(logs.page.items[0].time_sorter, actions.items[0].time_sorter);
    host.encounter_actions(old, 0, 1)
        .await
        .expect_err("previous-session encounter rejected");
    drop_then_shutdown(host).await;
    let clean = Host::start(deployment.config())
        .await
        .expect("worker lease released after abandoned query");
    assert_eq!(
        clean
            .query_history(&HistoryQuery::default())
            .await
            .expect("fresh worker query")
            .total,
        2
    );
    clean.shutdown().await.expect("stop clean worker");
}
async fn drop_then_shutdown(host: Host) {
    use std::{future::Future, task::Poll};
    let handle = host.handle();
    let query = HistoryQuery::default();
    let mut pending = Box::pin(handle.query_history(&query));
    std::future::poll_fn(|context| {
        if let Poll::Ready(result) = pending.as_mut().poll(context) {
            result.expect("early query completion");
        }
        Poll::Ready(())
    })
    .await;
    // FIFO diagnostics confirms the earlier query reached the worker before its caller disappears.
    host.diagnostics().await.expect("history admission barrier");
    drop(pending);
    host.shutdown()
        .await
        .expect("accepted history work drains despite dropped future");
}
async fn stale_index(host: &Host, deployment: &Deployment, page: &aftercast::HistoryPage) {
    let index = deployment.state.join("history/index.xml");
    let bytes = std::fs::read(&index).expect("owned index");
    let text = String::from_utf8(bytes.clone())
        .expect("UTF8")
        .replace("Import Zone", "Renamed Zone");
    std::fs::write(&index, text).expect("synthetic concurrent change");
    let newer = host
        .query_history(&HistoryQuery::default())
        .await
        .expect("new snapshot");
    assert_ne!(page.revision, newer.revision);
    assert!(matches!(
        host.restore_selected_history(
            &newer.revision,
            &[page.items.first().expect("first history entry").id.clone()]
        )
        .await,
        Err(Error::InvalidHistoryRequest(_))
    ));
    host.restore_selected_history(
        &page.revision,
        &[page.items.first().expect("first history entry").id.clone()],
    )
    .await
    .expect_err("stale catalog rejected");
    assert_eq!(
        host.encounters(0, 16).await.expect("unchanged graph").total,
        0
    );
    std::fs::write(index, bytes).expect("restore fixture");
}
