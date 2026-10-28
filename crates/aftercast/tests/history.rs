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
    assert!(
        saved
            .items
            .iter()
            .all(|item| item.selective_recording == Some(false))
    );
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
    let snapshot = host
        .capture_action_snapshot(id)
        .await
        .expect("capture")
        .expect("encounter");
    assert_eq!(snapshot.total, 2);
    let frozen = host
        .action_snapshot_page(snapshot, 1, 1)
        .await
        .expect("snapshot page");
    assert_eq!(frozen.items[0].action, actions.items[0]);
    assert!(!frozen.items[0].is_death);
    let frozen_logs = host
        .action_snapshot_logs(snapshot, 1, 1)
        .await
        .expect("snapshot logs");
    assert_eq!(
        frozen_logs.page.items[0].source,
        frozen.items[0].action.source
    );
    assert_eq!(
        frozen_logs.page.items[0].time_sorter,
        frozen.items[0].action.time_sorter
    );
    let replacement = host
        .capture_action_snapshot(id)
        .await
        .expect("replacement")
        .expect("encounter");
    host.action_snapshot_page(snapshot, 0, 1)
        .await
        .expect_err("replaced snapshot rejected");
    host.action_snapshot_logs(snapshot, 0, 1)
        .await
        .expect_err("replaced log snapshot rejected");
    let death_data = host
        .death_analysis(id, &actions.items[0].victim)
        .await
        .expect("death analysis")
        .expect("combatant");
    assert!(death_data.deaths.is_empty());
    assert_eq!(death_data.retained_logs, 2);
    let incoming = host
        .capture_incoming_action_snapshot(id, &actions.items[0].victim)
        .await
        .expect("incoming capture")
        .expect("combatant");
    let incoming_rows = host
        .action_snapshot_page(incoming, 0, 16)
        .await
        .expect("incoming rows");
    assert!(
        incoming_rows
            .items
            .iter()
            .any(|row| row.action.time_sorter == actions.items[0].time_sorter)
    );
    assert_eq!(
        host.action_snapshot_logs(incoming, 1, 1)
            .await
            .expect("incoming snapshot logs")
            .page
            .items[0]
            .time_sorter,
        actions.items[0].time_sorter
    );
    assert!(
        host.capture_incoming_action_snapshot(id, "missing combatant")
            .await
            .expect("missing query")
            .is_none()
    );
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
    clean
        .action_snapshot_logs(replacement, 0, 1)
        .await
        .expect_err("previous session log snapshot rejected");
    clean
        .action_snapshot_page(replacement, 0, 1)
        .await
        .expect_err("previous session snapshot rejected");
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

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:plugins"]
async fn death_analysis_keeps_frozen_action_log_identity(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    parser(&host).await;
    let input = deployment.directory().join("death.log");
    std::fs::write(&input, "0|100\n0|-10\n").expect("synthetic log");
    host.import_log(&input).await.expect("import");
    let id = host.encounters(0, 16).await.unwrap().items[0].id;
    let analysis = host.death_analysis(id, "Target").await.unwrap().unwrap();
    assert_eq!(analysis.deaths.len(), 1);
    let death = &analysis.actions[analysis.deaths[0]].action;
    assert_eq!(death.amount, -10);
    assert_eq!(
        analysis.actions[analysis.deaths[0]].numeric_tooltip,
        "Death"
    );
    let SourceLogLink::Retained(offset) = death.source else {
        panic!("death log missing")
    };
    let log = host
        .action_snapshot_logs(analysis.snapshot, offset, 1)
        .await
        .unwrap();
    assert_eq!(log.page.items[0].time_sorter, death.time_sorter);
    assert_eq!(log.page.items[0].text, "0|-10");
    let _replacement = host
        .capture_incoming_action_snapshot(id, "Target")
        .await
        .unwrap()
        .unwrap();
    host.action_snapshot_logs(analysis.snapshot, offset, 1)
        .await
        .expect_err("replaced death snapshot rejected");
    host.shutdown().await.unwrap();
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:plugins"]
async fn avoidance_report_is_session_owned_and_keeps_sample_log_identity(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.AvoidanceImportPlugin")
                .build(),
        )
        .await
        .expect("plugin");
    host.initialize_plugin(plugin).await.expect("initialize");
    let input = deployment.directory().join("avoidance.log");
    std::fs::write(&input, "0|100|Target\n0|-1|Target\n1|300|Other\n").expect("synthetic");
    host.import_log(&input).await.expect("import");
    let id = host
        .encounters(0, 16)
        .await
        .expect("encounters")
        .items
        .first()
        .expect("encounter")
        .id;
    let report = host
        .combatant_avoidance(id, "Target")
        .await
        .expect("report")
        .expect("combatant");
    assert_eq!(report.combatant.as_deref(), Some("Target"));
    assert_eq!(report.logs.len(), 3);
    let category = report
        .categories
        .iter()
        .find(|row| row.name == "Damage")
        .expect("category");
    let miss = category
        .outcomes
        .iter()
        .find(|row| row.label == "Miss")
        .expect("miss");
    assert_eq!(miss.count, 2);
    let aftercast::AvoidanceNumber::Finite(total) = miss.estimated_damage else {
        panic!("finite estimate")
    };
    assert_eq!(total, 200.0);
    for item in &miss.evidence {
        assert!(!item.used_fallback);
        assert_eq!(item.action.source_index, 1);
        assert!(item.samples.iter().any(|row| row.source_index == 0));
    }
    assert!(
        host.combatant_avoidance(id, "not-present")
            .await
            .expect("missing query")
            .is_none()
    );
    host.shutdown().await.expect("shutdown");
    let fresh = Host::start(deployment.config()).await.expect("restart");
    fresh
        .combatant_avoidance(id, "Target")
        .await
        .expect_err("previous session rejected");
    fresh.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:plugins"]
async fn encounter_avoidance_preserves_alias_overlap_and_scope(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.AvoidanceImportPlugin")
                .build(),
        )
        .await
        .expect("plugin");
    host.initialize_plugin(plugin).await.expect("initialize");
    let input = deployment.directory().join("avoidance-alias.log");
    std::fs::write(&input, "0|100|Target\n0|-1|Target|Source and YOU\n1|-2|Target|YOU only\n1|-1|Target|source lower\n1|-1|Target\n").expect("synthetic");
    host.import_log(&input).await.expect("import");
    let id = host
        .encounters(0, 16)
        .await
        .expect("encounters")
        .items
        .first()
        .expect("encounter")
        .id;
    host.encounter_detail(id)
        .await
        .expect("detail")
        .expect("encounter");
    let report = host
        .encounter_avoidance(id)
        .await
        .expect("encounter report")
        .expect("encounter");
    assert_eq!(report.scope, "encounter");
    assert!(report.combatant.is_none());
    let source = report
        .categories
        .iter()
        .find(|item| item.name == "Source")
        .expect("source alias");
    let you = report
        .categories
        .iter()
        .find(|item| item.name == "YOU")
        .expect("YOU alias");
    assert!(source.swings.is_none() && source.percent_swings.is_none());
    assert!(
        source
            .outcomes
            .iter()
            .any(|item| item.label == "Source and YOU")
    );
    assert!(
        you.outcomes
            .iter()
            .any(|item| item.label == "Source and YOU")
    );
    assert!(you.outcomes.iter().any(|item| item.label == "YOU only"));
    assert!(
        !report
            .categories
            .iter()
            .flat_map(|item| &item.outcomes)
            .any(|item| item.label == "source lower" || item.label == "Miss")
    );
    host.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:plugins"]
async fn special_report_worker_preserves_groups_and_sources(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.AvoidanceImportPlugin")
                .build(),
        )
        .await
        .expect("plugin");
    host.initialize_plugin(plugin).await.expect("initialize");
    let input = deployment.directory().join("specials.log");
    std::fs::write(&input, "0|100|Target||None\n1|100|Target||Crit\n2|100|Target||Crit\n3|100|Target||None\n4|100|Target||Flurry\n").expect("synthetic");
    host.import_log(&input).await.expect("import");
    let id = host.encounters(0, 16).await.expect("encounters").items[0].id;
    let report = host
        .special_report(id, "Source")
        .await
        .expect("report")
        .expect("source");
    let row = report
        .rows
        .iter()
        .find(|row| row.key == "Strike(Crit)")
        .expect("pair");
    assert_eq!(
        (row.occurrences, row.swings, row.total_swings),
        (Some(1), 2, Some(5))
    );
    assert_eq!(row.evidence[0].source_index, 1);
    assert_eq!(report.logs[1].text, "1|100|Target||Crit");
    let skill = report
        .rows
        .iter()
        .find(|row| row.group == "skill")
        .expect("skill");
    assert_eq!((skill.occurrences, skill.swings), (Some(2), 3));
    assert!(
        report
            .rows
            .iter()
            .any(|row| row.group == "special" && row.key == "Flurry")
    );
    assert!(
        host.special_report(id, "missing")
            .await
            .expect("missing")
            .is_none()
    );
    host.shutdown().await.expect("shutdown");
    let fresh = Host::start(deployment.config()).await.expect("restart");
    fresh
        .special_report(id, "Source")
        .await
        .expect_err("stale session");
    fresh.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:plugins"]
async fn timeline_worker_keeps_grouped_rates_and_logs_owned(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.AvoidanceImportPlugin")
                .build(),
        )
        .await
        .expect("plugin");
    host.initialize_plugin(plugin).await.expect("initialize");
    let input = deployment.directory().join("timeline.log");
    std::fs::write(
        &input,
        "0|100|Target\n5|-1|Target\n10|200|Target\n15|-1|Target\n20|-1|Target\n",
    )
    .expect("synthetic");
    host.import_log(&input).await.expect("import");
    let id = host.encounters(0, 16).await.expect("encounters").items[0].id;
    host.encounter_detail(id)
        .await
        .expect("detail")
        .expect("encounter");
    let options = aftercast::TimelineOptions::default();
    let report = host
        .timeline(id, options)
        .await
        .expect("timeline")
        .expect("encounter");
    assert_eq!((report.duration, report.points), (20, 20));
    let source = report
        .rows
        .iter()
        .find(|row| row.name == "Source")
        .expect("source");
    let outgoing = source
        .series
        .iter()
        .find(|series| series.metric == 0)
        .expect("outgoing");
    assert_eq!((outgoing.rates[0], outgoing.rates[9]), (20, 40));
    assert_eq!(report.logs[2].text, "10|200|Target");
    assert_eq!(report.logs[2].seconds, 10.0);
    let grouped = host
        .timeline(
            id,
            aftercast::TimelineOptions {
                grouping: 2,
                ..options
            },
        )
        .await
        .expect("grouped")
        .expect("encounter");
    assert!(grouped.rows.iter().any(|row| row.name == "Us"));
    assert_eq!(report.logs[2].text, "10|200|Target");
    host.shutdown().await.expect("shutdown");
    let fresh = Host::start(deployment.config()).await.expect("restart");
    fresh
        .timeline(id, options)
        .await
        .expect_err("stale session");
    fresh.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:plugins"]
async fn recast_worker_keeps_modifier_evidence_and_frozen_sources(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.RecastImportPlugin")
                .build(),
        )
        .await
        .expect("plugin");
    host.initialize_plugin(plugin).await.expect("initialize");
    let input = deployment.directory().join("recast.log");
    std::fs::write(&input, "0|100|Strike|Source|Target\n5|0|Traumatic Swipe|Enemy|Source\n5|-1|Strike|Source|Target\n10|100|Strike|Source|Target\n12|-10|Killing|Source|Enemy\n15|-1|Strike|Source|Target\n20|100|Strike|Source|Target\n").expect("synthetic");
    host.import_log(&input).await.expect("import");
    let id = host.encounters(0, 16).await.expect("encounters").items[0].id;
    let request = aftercast::RecastRequest {
        combatant: "Source".into(),
        category: "Damage".into(),
        attack: "Strike".into(),
        outgoing: true,
        modifier: true,
    };
    let report = host
        .recast_snapshot(id, &request)
        .await
        .expect("capture")
        .expect("attack");
    assert_eq!(
        report
            .rows
            .iter()
            .map(|row| row.modified)
            .collect::<Vec<_>>(),
        vec![
            Some(false),
            Some(false),
            Some(true),
            Some(false),
            Some(false)
        ]
    );
    assert_eq!(report.rows[1].action.amount, -1);
    assert_eq!(report.rows[2].action.source_index, 3);
    assert_eq!(
        report.rows[2]
            .modifier_evidence
            .as_ref()
            .expect("swipe")
            .source_index,
        1
    );
    assert_eq!(report.logs[1].text, "5|0|Traumatic Swipe|Enemy|Source");
    let times = report.times();
    let selected = aftercast::recast_initial_selection(&times, false).expect("selection");
    let calculation =
        aftercast::recast_calculate(&times, &selected, false, true).expect("calculate");
    assert_eq!(calculation.intervals, [5, 5, 3, 5]);
    assert_eq!(calculation.minimum, Some(3));
    let mut wrong = request.clone();
    wrong.outgoing = false;
    assert!(
        host.recast_snapshot(id, &wrong)
            .await
            .expect("wrong direction")
            .is_none()
    );
    host.shutdown().await.expect("shutdown");
    let fresh = Host::start(deployment.config()).await.expect("restart");
    fresh
        .recast_snapshot(id, &request)
        .await
        .expect_err("stale session");
    fresh.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:plugins"]
async fn vcr_worker_copies_actions_and_logs_without_replaying(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("host");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.VcrImportPlugin")
                .build(),
        )
        .await
        .expect("plugin");
    host.initialize_plugin(plugin).await.expect("initialize");
    let input = deployment.directory().join("vcr.log");
    std::fs::write(
        &input,
        "0|100|Strike|Source|Target\n5|50|Strike|Source|Target\n10|25|Strike|Source|Target\n",
    )
    .expect("synthetic");
    host.import_log(&input).await.expect("import");
    let id = host.encounters(0, 16).await.expect("encounters").items[0].id;
    let frozen = host
        .vcr_snapshot(id)
        .await
        .expect("capture")
        .expect("encounter");
    assert_eq!(frozen.logs.len(), 3);
    let frame = aftercast::vcr_frame(&frozen, 5, 5, &["Damage".into()], true, true).expect("frame");
    assert!(
        frame
            .edges
            .iter()
            .any(|edge| edge.from == "Source" && edge.to == "Target" && edge.amount == 150)
    );
    let replacement = host
        .vcr_snapshot(id)
        .await
        .expect("capture")
        .expect("encounter");
    assert_ne!(frozen.revision, replacement.revision);
    assert_eq!(frozen.logs.len(), 3);
    host.shutdown().await.expect("shutdown");
    let fresh = Host::start(deployment.config()).await.expect("restart");
    fresh.vcr_snapshot(id).await.expect_err("stale session");
    fresh.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:plugins"]
async fn vcr_live_refresh_does_not_change_frozen_import(deployment: Deployment) {
    use std::io::Write;
    let host = Host::start(deployment.config()).await.expect("host");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.VcrLivePlugin")
                .build(),
        )
        .await
        .expect("plugin");
    host.initialize_plugin(plugin).await.expect("initialize");
    let input = deployment.directory().join("import.log");
    std::fs::write(&input, "0|100\n2|200\n").expect("synthetic import");
    host.import_log(&input).await.expect("import");
    let id = host.encounters(0, 16).await.expect("encounters").items[0].id;
    let frozen = host
        .vcr_snapshot(id)
        .await
        .expect("capture")
        .expect("import");
    let live_file = deployment.state.join("live.log");
    std::fs::OpenOptions::new()
        .append(true)
        .open(&live_file)
        .expect("live fixture")
        .write_all(b"0|50\n2|75\n")
        .expect("append");
    let marker = deployment.state.join("live.accepted");
    let started = std::time::Instant::now();
    while !std::fs::read_to_string(&marker).is_ok_and(|s| s == "2|75") {
        host.session_id().await.expect("pump");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "synthetic live fixture timed out"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let live = host
        .live_combat()
        .await
        .expect("live query")
        .expect("active");
    let current = host
        .vcr_snapshot(live.summary.id)
        .await
        .expect("capture")
        .expect("live");
    assert!(!current.summary.is_import);
    assert!(frozen.summary.is_import);
    assert_ne!(current.summary.id, frozen.summary.id);
    assert_eq!(frozen.logs.len(), 2);
    assert_eq!(current.logs.len(), 2);
    assert_eq!(frozen.logs[1].text, "2|200");
    assert_eq!(current.logs[1].text, "2|75");
    host.shutdown().await.expect("shutdown");
}
