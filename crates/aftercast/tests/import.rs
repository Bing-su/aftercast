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
        companion_heads_excluded: 0,
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
    let before = handle
        .encounters(0, 100)
        .await
        .expect("before cancellation")
        .items
        .into_iter()
        .map(|row| row.id)
        .collect::<Vec<_>>();
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
    assert_canceled_range_partial(&handle, &before).await;
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

async fn assert_canceled_range_partial(
    handle: &aftercast::HostHandle,
    before: &[aftercast::EncounterId],
) {
    let graph = handle.encounters(0, 100).await.expect("after cancellation");
    let added = graph
        .items
        .iter()
        .filter(|row| !before.contains(&row.id))
        .collect::<Vec<_>>();
    assert_eq!(
        added.len(),
        1,
        "this canceled range must retain its own partial encounter"
    );
    let partial = added.first().expect("new partial encounter");
    assert_eq!(partial.status, aftercast::EncounterStatus::PartialImport);
    assert!(partial.damage > 0);
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn silence_cut_is_session_owned_and_applies_to_imported_rates(deployment: Deployment) {
    use aftercast::HostConfig;
    let input = deployment.directory().join("silence.log");
    std::fs::write(&input, "0|100|Strike|Source|Target\n6|100|Strike|Source|Target\n13|100|Strike|Source|Target\n14|100|Strike|Source|Target\n").expect("synthetic input");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let mut old_id = None;
    for (cut, expected) in [(6, 7), (0, 14)] {
        let config = HostConfig::builder()
            .application_executable(common::application())
            .dotnet_root(std::env::var("DOTNET_ROOT").expect("dotnet"))
            .bridge_directory(&deployment.bridge)
            .data_directory(&deployment.state)
            .idle_end_enabled(false)
            .silence_cut_seconds(cut)
            .build();
        let host = Host::start(config).await.expect("host");
        if let Some(id) = old_id {
            host.encounter_detail(id).await.expect_err("old session");
        }
        let plugin = host
            .create_plugin(
                PluginConfig::builder()
                    .path(path.clone())
                    .type_name("Aftercast.PluginFixture.VcrImportPlugin")
                    .build(),
            )
            .await
            .expect("plugin");
        host.initialize_plugin(plugin).await.expect("initialize");
        host.import_log(&input).await.expect("import");
        let page = host.encounters(0, 16).await.expect("encounters");
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].duration_ticks, expected * 10_000_000);
        old_id = Some(page.items[0].id);
        host.shutdown().await.expect("shutdown");
    }
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn selective_options_reach_worker_before_plugin_and_import(deployment: Deployment) {
    use aftercast::{HostConfig, SelectiveMode, SelectiveOptions};
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let log = deployment.directory().join("synthetic-selective.log");
    std::fs::write(&log, "0|Other|Target\n1|Other|Source\n2|Source|Target\n")
        .expect("synthetic log");
    let duplicate = HostConfig::builder()
        .application_executable(common::application())
        .dotnet_root(std::env::var("DOTNET_ROOT").expect("dotnet"))
        .bridge_directory(&deployment.bridge)
        .data_directory(&deployment.state)
        .selective(SelectiveOptions {
            mode: SelectiveMode::Encounters,
            selected: vec!["Source".into(), "source".into()],
        })
        .build();
    if let Ok(host) = Host::start(duplicate).await {
        host.shutdown().await.expect("stop");
        panic!("duplicate selection was admitted");
    }
    for (mode, status, count) in [
        (SelectiveMode::Encounters, "SelectiveEncounters", 2),
        (
            SelectiveMode::EncountersStrict,
            "SelectiveEncountersStrict",
            1,
        ),
        (SelectiveMode::None, "None", 3),
        (SelectiveMode::GraphsExports, "SelectiveGraphsExports", 3),
        (
            SelectiveMode::GraphsExportsAndAlliedTotals,
            "SelectiveGraphsExportsAndAlliedTotals",
            3,
        ),
    ] {
        let host = Host::start(
            HostConfig::builder()
                .application_executable(common::application())
                .dotnet_root(std::env::var("DOTNET_ROOT").expect("dotnet"))
                .bridge_directory(&deployment.bridge)
                .data_directory(&deployment.state)
                .selective(SelectiveOptions {
                    mode,
                    selected: vec!["Source".into()],
                })
                .build(),
        )
        .await
        .expect("host");
        let plugin = host
            .create_plugin(
                PluginConfig::builder()
                    .path(path.clone())
                    .type_name("Aftercast.PluginFixture.SelectiveFixturePlugin")
                    .build(),
            )
            .await
            .expect("plugin");
        assert_eq!(
            host.initialize_plugin(plugin).await.expect("initialize"),
            status
        );
        host.import_log(&log).await.expect("import");
        let encounter = host.encounters(0, 16).await.expect("encounters").items[0].id;
        let actions = host
            .encounter_actions(encounter, 0, 16)
            .await
            .expect("actions")
            .expect("encounter");
        assert_eq!(actions.total, count);
        if matches!(
            mode,
            SelectiveMode::GraphsExports | SelectiveMode::GraphsExportsAndAlliedTotals
        ) {
            let catalog = host.text_report_catalog().await.expect("catalog");
            let options = aftercast::TextReportOptions {
                player_format: "{selective-name}".into(),
                sorting: "selective-name".into(),
                ..Default::default()
            };
            let report = host
                .text_report(encounter, catalog.revision, &options)
                .await
                .expect("report")
                .expect("encounter");
            assert_eq!(report.text, "Source\n");
        }
        if mode == SelectiveMode::EncountersStrict {
            assert_eq!(
                host.save_history().await.expect("save selective history"),
                1
            );
            let history = host
                .query_history(&aftercast::HistoryQuery::default())
                .await
                .expect("history");
            assert_eq!(
                history
                    .items
                    .iter()
                    .find(|entry| entry.already_restored)
                    .expect("saved current")
                    .selective_recording,
                Some(true)
            );
            assert_eq!(actions.items[0].attacker, "Source");
        }
        host.shutdown().await.expect("shutdown");
    }
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn selective_recording_filters_synthetic_live_append(deployment: Deployment) {
    use aftercast::{HostConfig, LiveLogOptions, SelectiveMode, SelectiveOptions};
    use std::{
        io::Write,
        time::{Duration, Instant},
    };
    let host = Host::start(
        HostConfig::builder()
            .application_executable(common::application())
            .dotnet_root(std::env::var("DOTNET_ROOT").expect("dotnet"))
            .bridge_directory(&deployment.bridge)
            .data_directory(&deployment.state)
            .selective(SelectiveOptions {
                mode: SelectiveMode::EncountersStrict,
                selected: vec!["Source".into()],
            })
            .build(),
    )
    .await
    .expect("host");
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.SelectiveFixturePlugin")
                .build(),
        )
        .await
        .expect("plugin");
    host.initialize_plugin(plugin).await.expect("initialize");
    let log = deployment.directory().join("synthetic-live.log");
    std::fs::write(&log, "").expect("empty");
    let live = host
        .open_live_logs(&log, LiveLogOptions::default())
        .await
        .expect("open");
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&log)
        .expect("append");
    file.write_all(b"0|Other|Target\n1|Other|Source\n2|Source|Target\n")
        .expect("synthetic lines");
    file.flush().expect("flush");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let page = host.encounters(0, 16).await.expect("encounters");
        if let Some(encounter) = page.items.first() {
            let actions = host
                .encounter_actions(encounter.id, 0, 16)
                .await
                .expect("actions")
                .expect("encounter");
            if actions.total > 0 {
                assert_eq!(actions.total, 1);
                assert_eq!(actions.items[0].attacker, "Source");
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "synthetic live action not delivered"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    host.close_live_logs(live.selection).await.expect("close");
    host.end_combat().await.expect("finish");
    host.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn zero_damage_hit_option_reaches_plugins_reparse_and_history(deployment: Deployment) {
    use aftercast::{HostConfig, TableLevel, TableQuery, TableScope};
    for enabled in [true, false] {
        let state = deployment
            .directory()
            .join(if enabled { "hit-on" } else { "hit-off" });
        let config = HostConfig::builder()
            .application_executable(common::application())
            .dotnet_root(std::env::var("DOTNET_ROOT").expect("dotnet"))
            .bridge_directory(&deployment.bridge)
            .data_directory(&state)
            .count_zero_damage_as_hit(enabled)
            .build();
        let plugin_path = Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll",
        );
        let plugin = || {
            PluginConfig::builder()
                .path(plugin_path.clone())
                .type_name("Aftercast.PluginFixture.ZeroDamagePlugin")
                .build()
        };
        let host = Host::start(config.clone()).await.expect("host");
        let id = host.create_plugin(plugin()).await.expect("plugin");
        assert_eq!(
            host.initialize_plugin(id).await.expect("init"),
            if enabled {
                "zero-hit:on"
            } else {
                "zero-hit:off"
            }
        );
        let source = deployment.directory().join("zero-hit-synthetic.log");
        std::fs::write(&source, "0|0\n1|100\n").expect("synthetic input");
        host.import_log(&source).await.expect("first import");
        let encounter = host.encounters(0, 16).await.expect("encounters").items[0].id;
        check_zero_hits(&host, encounter, enabled).await;
        let catalog = host
            .table_columns(TableLevel::Encounter)
            .await
            .expect("catalog");
        let query = TableQuery {
            scope: TableScope::Encounters,
            revision: catalog.revision,
            columns: vec!["zero-hit-probe".into()],
            offset: 0,
            limit: 16,
            sort: None,
        };
        let page = host
            .table_rows(&query)
            .await
            .expect("table")
            .expect("scope");
        assert_eq!(
            page.rows[0].cells[0].text.as_deref(),
            Some(if enabled {
                "True:0,True:100"
            } else {
                "False:0,False:100"
            })
        );
        host.import_log(&source).await.expect("reparse");
        let encounters = host.encounters(0, 16).await.expect("reparsed");
        assert_eq!(encounters.total, 2);
        for row in &encounters.items {
            check_zero_hits(&host, row.id, enabled).await;
        }
        assert_eq!(host.save_history().await.expect("save"), 2);
        host.shutdown().await.expect("stop");
        let host = Host::start(config).await.expect("restart");
        let id = host.create_plugin(plugin()).await.expect("plugin");
        host.initialize_plugin(id).await.expect("init");
        assert_eq!(host.restore_history("").await.expect("restore"), 2);
        for row in host.encounters(0, 16).await.expect("restored").items {
            check_zero_hits(&host, row.id, enabled).await;
        }
        host.shutdown().await.expect("stop");
    }
}

async fn check_zero_hits(host: &Host, encounter: aftercast::EncounterId, enabled: bool) {
    let actions = host
        .encounter_actions(encounter, 0, 16)
        .await
        .expect("actions")
        .expect("encounter");
    assert_eq!(
        actions
            .items
            .iter()
            .map(|row| row.amount)
            .collect::<Vec<_>>(),
        [0, 100]
    );
    let attacks = host
        .encounter_attacks(encounter, 0, 128)
        .await
        .expect("attacks")
        .expect("encounter");
    let strike = attacks
        .items
        .iter()
        .find(|row| row.combatant == "Source" && row.category == "Damage" && row.attack == "Strike")
        .expect("strike");
    assert_eq!(strike.damage, 100);
    assert_eq!(strike.hits, if enabled { 2 } else { 1 });
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn correction_editor_is_atomic_session_owned_and_only_affects_new_actions(
    deployment: Deployment,
) {
    use aftercast::{RedirectDefinition, RenameDefinition, TableLevel, TableQuery, TableScope};
    let plugin_path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = || {
        PluginConfig::builder()
            .path(plugin_path.clone())
            .type_name("Aftercast.PluginFixture.CorrectionProbePlugin")
            .build()
    };
    let host = Host::start(deployment.config()).await.expect("host");
    let id = host.create_plugin(plugin()).await.expect("plugin");
    host.initialize_plugin(id).await.expect("initialize");
    let first = host.corrections().await.expect("read");
    let rename = RenameDefinition {
        before: "Source".into(),
        after: "Hero".into(),
        active: true,
    };
    let redirect = RedirectDefinition {
        ability: "Strike".into(),
        destination: "Owner".into(),
        r#type: 1,
        active: true,
    };
    let saved = host
        .save_corrections(
            first.revision,
            std::slice::from_ref(&rename),
            std::slice::from_ref(&redirect),
        )
        .await
        .expect("save");
    host.save_corrections(first.revision, &[], &[])
        .await
        .expect_err("stale revision");
    host.save_corrections(
        saved.revision,
        &[RenameDefinition {
            before: " ".into(),
            ..rename.clone()
        }],
        &[],
    )
    .await
    .expect_err("empty name");
    let path = deployment.directory().join("correction-synthetic.log");
    std::fs::write(&path, "0|100\n").expect("source");
    host.import_log(&path).await.expect("import");
    let old = host.encounters(0, 16).await.expect("encounters").items[0].id;
    let action = host
        .encounter_actions(old, 0, 16)
        .await
        .expect("actions")
        .expect("encounter");
    assert_eq!(action.items[0].attacker, "Owner");
    let catalog = host
        .table_columns(TableLevel::Encounter)
        .await
        .expect("columns");
    let query = TableQuery {
        scope: TableScope::Encounters,
        revision: catalog.revision,
        columns: vec!["correction-probe".into()],
        offset: 0,
        limit: 16,
        sort: None,
    };
    let page = host
        .table_rows(&query)
        .await
        .expect("table")
        .expect("scope");
    assert_eq!(
        page.rows[0].cells[0].text.as_deref(),
        Some("Owner:Target:Source")
    );
    let changed = host
        .save_corrections(saved.revision, std::slice::from_ref(&rename), &[])
        .await
        .expect("remove redirect");
    host.import_log(&path).await.expect("reparse");
    let rows = host.encounters(0, 16).await.expect("encounters");
    assert_eq!(rows.total, 2);
    let mut names = Vec::new();
    for row in &rows.items {
        names.push(
            host.encounter_actions(row.id, 0, 16)
                .await
                .expect("actions")
                .expect("encounter")
                .items[0]
                .attacker
                .clone(),
        );
    }
    names.sort();
    assert_eq!(names, ["Hero", "Owner"]);
    assert_eq!(host.save_history().await.expect("save history"), 2);
    host.shutdown().await.expect("stop");
    let host = Host::start(deployment.config()).await.expect("restart");
    let id = host.create_plugin(plugin()).await.expect("plugin");
    host.initialize_plugin(id).await.expect("initialize");
    host.save_corrections(changed.revision, &[], &[])
        .await
        .expect_err("old session");
    let current = host.corrections().await.expect("restored definitions");
    assert_eq!(current.renames, [rename]);
    assert!(current.redirects.is_empty());
    assert_eq!(host.restore_history("").await.expect("restore"), 2);
    let mut names = Vec::new();
    for row in host.encounters(0, 16).await.expect("restored").items {
        names.push(
            host.encounter_actions(row.id, 0, 16)
                .await
                .expect("actions")
                .expect("encounter")
                .items[0]
                .attacker
                .clone(),
        );
    }
    names.sort();
    assert_eq!(names, ["Hero", "Owner"]);
    let settings = deployment.state.join("DataCorrections.xml");
    let bytes = std::fs::read(&settings).expect("settings");
    std::fs::write(&settings, "<DataCorrections/>").expect("external change");
    host.save_corrections(current.revision, &[], &[])
        .await
        .expect_err("external edit");
    assert_eq!(
        host.corrections()
            .await
            .expect("unchanged runtime")
            .renames
            .len(),
        1
    );
    std::fs::write(&settings, bytes).expect("restore fixture");
    host.save_corrections(current.revision, &[], &[])
        .await
        .expect("delete rules");
    host.shutdown().await.expect("stop");
    let host = Host::start(deployment.config()).await.expect("restart");
    assert!(
        host.corrections()
            .await
            .expect("deleted")
            .renames
            .is_empty()
    );
    host.shutdown().await.expect("stop");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn selective_runtime_apply_is_session_only_and_filters_new_records(deployment: Deployment) {
    use aftercast::{SelectiveMode, SelectiveOptions};
    let host = Host::start(deployment.config()).await.unwrap();
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path.clone())
                .type_name("Aftercast.PluginFixture.SelectiveFixturePlugin")
                .build(),
        )
        .await
        .unwrap();
    assert_eq!(host.initialize_plugin(plugin).await.unwrap(), "None");
    let options = SelectiveOptions {
        mode: SelectiveMode::EncountersStrict,
        selected: vec!["Source".into()],
    };
    host.apply_selective_options(&options).await.unwrap();
    let log = deployment.directory().join("runtime-selective.log");
    std::fs::write(&log, "0|Other|Target\n1|Other|Source\n2|Source|Target\n").unwrap();
    host.import_log(&log).await.unwrap();
    let encounter = host.encounters(0, 16).await.unwrap().items[0].id;
    assert_eq!(
        host.encounter_actions(encounter, 0, 16)
            .await
            .unwrap()
            .unwrap()
            .total,
        1
    );
    host.apply_selective_options(&SelectiveOptions {
        mode: SelectiveMode::GraphsExports,
        selected: vec!["Source".into()],
    })
    .await
    .unwrap();
    assert_eq!(
        host.encounter_actions(encounter, 0, 16)
            .await
            .unwrap()
            .unwrap()
            .total,
        1,
        "runtime transition must not reconstruct omitted history"
    );
    let old = host.handle();
    host.shutdown().await.unwrap();
    old.apply_selective_options(&options)
        .await
        .expect_err("closed session");
    let fresh = Host::start(deployment.config()).await.unwrap();
    let plugin = fresh
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Aftercast.PluginFixture.SelectiveFixturePlugin")
                .build(),
        )
        .await
        .unwrap();
    assert_eq!(fresh.initialize_plugin(plugin).await.unwrap(), "None");
    fresh
        .apply_selective_options(&SelectiveOptions {
            mode: SelectiveMode::GraphsExports,
            selected: vec!["Source".into()],
        })
        .await
        .unwrap();
    fresh.import_log(&log).await.unwrap();
    let encounter = fresh.encounters(0, 16).await.unwrap().items[0].id;
    assert_eq!(
        fresh
            .encounter_actions(encounter, 0, 16)
            .await
            .unwrap()
            .unwrap()
            .total,
        3
    );
    fresh.shutdown().await.unwrap();
}
