use super::*;
use aftercast::{TableLevel, TableReportFormat, TableReportOptions, TableScope, TableSort};
fn table_plugin() -> PluginConfig {
    PluginConfig::builder()
        .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll",
        ))
        .type_name("Aftercast.PluginFixture.TableReportPlugin")
        .build()
}
fn xml() -> TableReportOptions {
    TableReportOptions {
        format: TableReportFormat::TableXml,
        name: "한글日本😀\r\n<caption>&\"".into(),
        selected_rows: vec![],
        columns: None,
        sort: None,
    }
}
async fn start(deployment: &Deployment, input: &str) -> (Host, aftercast::EncounterId) {
    let host = Host::start(deployment.config())
        .await
        .expect("isolated worker");
    let id = host
        .create_plugin(table_plugin())
        .await
        .expect("synthetic plugin");
    host.initialize_plugin(id).await.expect("init");
    let path = deployment.directory().join("table-report.log");
    std::fs::write(&path, input).expect("synthetic log");
    host.import_log(&path).await.expect("import");
    let encounter = host
        .encounters(0, 16)
        .await
        .expect("graph")
        .items
        .first()
        .expect("encounter")
        .id;
    (host, encounter)
}
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn table_xml_roots_selection_sorting_html_and_sessions(deployment: Deployment) {
    let (host, encounter) = start(&deployment, "0|100\n1|300\n2|200\n").await;
    let scopes = [
        TableScope::Combatants(encounter),
        TableScope::Categories {
            encounter,
            combatant: "Source".into(),
        },
        TableScope::Attacks {
            encounter,
            combatant: "Source".into(),
            category: "Damage".into(),
        },
        TableScope::Actions {
            encounter,
            combatant: "Source".into(),
            category: "Damage".into(),
            attack: "All".into(),
        },
    ];
    let mut previous = None;
    for (scope, root) in scopes.iter().zip([
        "EncounterTable",
        "CombatantTable",
        "DamageTypeTable",
        "AttackTypeTable",
    ]) {
        let catalog = host
            .table_report_catalog(scope)
            .await
            .expect("catalog")
            .expect("scope");
        if let Some(old) = previous.take() {
            host.table_report(&old, &xml())
                .await
                .expect_err("previous capture retired");
        }
        assert_eq!(catalog.selection.scope(), scope);
        assert!(catalog.rows.iter().all(|row| row.cells.is_empty()));
        let report = host
            .table_report(&catalog.selection, &xml())
            .await
            .expect("XML");
        assert!(
            report.content.contains(&format!("<{root} Name=\"")),
            "{}",
            report.content
        );
        assert!(
            report
                .content
                .contains("&#xD;&#xA;&lt;caption&gt;&amp;&quot;")
        );
        assert!(report.content.contains("<sql_name>sql:한글日本😀\r\n"));
        assert!(!report.content.contains("<fixture>"));
        assert!(!report.content.contains("<script>"));
        assert_eq!(report.rows, catalog.rows.len() as u32);
        let mut one = xml();
        one.selected_rows = vec![0];
        assert_eq!(
            host.table_report(&catalog.selection, &one)
                .await
                .expect("one means full")
                .rows,
            report.rows
        );
        let mut html = xml();
        html.format = TableReportFormat::StaticHtml;
        html.columns = Some(vec!["fixture".into()]);
        let rendered = host
            .table_report(&catalog.selection, &html)
            .await
            .expect("static HTML");
        assert!(rendered.content.contains("&lt;script&gt;&amp;&quot;"));
        assert!(!rendered.content.contains("<script>"));
        assert!(rendered.content.contains("Content-Security-Policy"));
        assert!(rendered.content.contains("<th>fixture</th>"));
        if root == "EncounterTable" {
            assert!(rendered.content.contains("한글&#128512;:1</td>"));
            assert!(rendered.content.contains("한글&#128512;:2</td>"));
            let again = host
                .table_report(&catalog.selection, &html)
                .await
                .expect("repeat exactly once");
            assert!(again.content.contains("한글&#128512;:3</td>"));
            assert!(again.content.contains("한글&#128512;:4</td>"));
        }
        previous = Some(catalog.selection);
    }
    let scope = scopes[3].clone();
    let catalog = host
        .table_report_catalog(&scope)
        .await
        .expect("actions")
        .expect("scope");
    let mut subset = xml();
    subset.selected_rows = vec![0, 2];
    let report = host
        .table_report(&catalog.selection, &subset)
        .await
        .expect("XML subset");
    assert_eq!(report.rows, 2);
    assert!(report.content.contains("100</sql_name>"));
    assert!(report.content.contains("200</sql_name>"));
    assert!(!report.content.contains("300</sql_name>"));
    let actors = host
        .table_report_catalog(&scopes[0])
        .await
        .expect("actors")
        .expect("scope");
    let mut sorted = xml();
    sorted.sort = Some(TableSort {
        primary: "fixture".into(),
        secondary: None,
        descending: true,
    });
    assert_eq!(
        host.table_report(&actors.selection, &sorted)
            .await
            .expect("sort")
            .rows,
        2
    );
    let ordered = host
        .table_report(&actors.selection, &sorted)
        .await
        .expect("ordered SQL values");
    assert!(
        ordered.content.find("Target</sql_name>").expect("target")
            < ordered.content.find("Source</sql_name>").expect("source")
    );
    sorted.sort.as_mut().expect("sort").primary = "bad-sort".into();
    let error = host
        .table_report(&actors.selection, &sorted)
        .await
        .expect_err("callback sort failure");
    assert!(
        error.to_string().contains("synthetic report sort failure"),
        "{error}"
    );
    host.table_report(&actors.selection, &xml())
        .await
        .expect("guard recovered");
    let missing = TableScope::Categories {
        encounter,
        combatant: "absent".into(),
    };
    assert!(
        host.table_report_catalog(&missing)
            .await
            .expect("missing")
            .is_none()
    );
    host.table_report(&actors.selection, &xml())
        .await
        .expect_err("missing capture also retires selection");
    let old_handle = host.handle();
    host.shutdown().await.expect("stop");
    assert!(matches!(
        old_handle.table_report(&actors.selection, &xml()).await,
        Err(Error::Closed)
    ));
    let (restarted, new_encounter) = start(&deployment, "0|100\n").await;
    assert!(matches!(
        restarted.table_report(&actors.selection, &xml()).await,
        Err(Error::InvalidReportRequest(_))
    ));
    assert!(matches!(
        restarted.table_report_catalog(&scopes[0]).await,
        Err(Error::InvalidReportRequest(_))
    ));
    let fresh = restarted
        .table_report_catalog(&TableScope::Combatants(new_encounter))
        .await
        .expect("new scope")
        .expect("capture");
    restarted
        .table_report(&fresh.selection, &xml())
        .await
        .expect("new session success");
    restarted.shutdown().await.expect("final stop");
}
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn table_report_worker_surfaces_sql_invalid_unicode_and_cell_limits(deployment: Deployment) {
    for amount in [555, 666, 777, 888] {
        let (host, encounter) = start(&deployment, &format!("0|{amount}\n")).await;
        let catalog = host
            .table_report_catalog(&TableScope::Combatants(encounter))
            .await
            .expect("catalog")
            .expect("scope");
        host.table_report(&catalog.selection, &xml())
            .await
            .expect_err("SQL error/invalid UTF16/cell limit");
        let fresh = host
            .table_report_catalog(&TableScope::Combatants(encounter))
            .await
            .expect("recapture after failure")
            .expect("scope");
        let mut html = xml();
        html.format = TableReportFormat::StaticHtml;
        html.columns = Some(vec!["fixture".into()]);
        host.table_report(&fresh.selection, &html)
            .await
            .expect("guard and worker recover after SQL failure");
        host.shutdown().await.expect("stop after failure");
    }
}
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn table_report_worker_rejects_escaped_serialization_expansion(deployment: Deployment) {
    let input = (0..900)
        .map(|index| format!("{index}|999\n"))
        .collect::<String>();
    let (host, encounter) = start(&deployment, &input).await;
    let scope = TableScope::Actions {
        encounter,
        combatant: "Source".into(),
        category: "Damage".into(),
        attack: "All".into(),
    };
    let catalog = host
        .table_report_catalog(&scope)
        .await
        .expect("catalog")
        .expect("scope");
    assert_eq!(catalog.rows.len(), 900);
    let error = host
        .table_report(&catalog.selection, &xml())
        .await
        .expect_err("escaping exceeds serialized budget");
    assert!(
        error
            .to_string()
            .contains("Serialized table report exceeds 16 MiB"),
        "{error}"
    );
    host.shutdown().await.expect("stop after limit");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn offline_odbc_previews_use_owned_worker_snapshots(deployment: Deployment) {
    use aftercast::HierarchyXmlDepth;
    let host = Host::start(deployment.config()).await.expect("worker");
    let plugin = PluginConfig::builder()
        .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll",
        ))
        .type_name("Aftercast.PluginFixture.OdbcPreviewPlugin")
        .build();
    let id = host.create_plugin(plugin).await.expect("plugin");
    host.initialize_plugin(id).await.expect("init");
    let path = deployment.directory().join("odbc-synthetic.log");
    std::fs::write(&path, "0|100\n1|200\n").expect("synthetic input");
    host.import_log(path).await.expect("import");
    let encounter = host
        .encounters(0, 16)
        .await
        .expect("encounters")
        .items
        .first()
        .expect("encounter")
        .id;
    let scope = TableScope::Combatants(encounter);
    let catalog = host
        .table_report_catalog(&scope)
        .await
        .expect("catalog")
        .expect("scope");
    let current = host
        .odbc_current_preview(&catalog.selection, &["encid".into(), "second".into()])
        .await
        .expect("current");
    assert_eq!(
        current.reset_sql.as_deref(),
        Some("DELETE FROM \"current_table\";")
    );
    assert!(
        current
            .parameters
            .iter()
            .all(|row| row.first().is_some_and(String::is_empty))
    );
    let selection = host
        .capture_hierarchy_xml(encounter, HierarchyXmlDepth::AllTables)
        .await
        .expect("capture")
        .expect("scope");
    let columns = [
        vec!["enc_sql".into()],
        vec!["second".into()],
        vec!["first".into()],
        vec!["first".into()],
        vec!["second".into()],
    ];
    let plan = host
        .odbc_hierarchy_preview(&selection, &columns, "All")
        .await
        .expect("hierarchy");
    assert_eq!(plan.tables.len(), 5);
    assert_eq!(
        plan.tables
            .first()
            .expect("encounter")
            .parameters
            .first()
            .expect("row")
            .first()
            .map(String::as_str),
        Some("encounter")
    );
    assert!(
        plan.tables
            .iter()
            .all(|table| table.reset_sql.is_none() && table.insert_sql.contains('?'))
    );
    let replacement = host
        .capture_hierarchy_xml(encounter, HierarchyXmlDepth::AllTables)
        .await
        .expect("recapture")
        .expect("scope");
    host.odbc_hierarchy_preview(&selection, &columns, "All")
        .await
        .expect_err("old revision");
    let handle = host.handle();
    host.shutdown().await.expect("stop");
    handle
        .odbc_hierarchy_preview(&replacement, &columns, "All")
        .await
        .expect_err("closed session");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn offline_odbc_batch_owns_encounters_and_rejects_stale(deployment: Deployment) {
    let host = Host::start(deployment.config()).await.expect("worker");
    let plugin = PluginConfig::builder()
        .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll",
        ))
        .type_name("Aftercast.PluginFixture.OdbcPreviewPlugin")
        .build();
    let plugin = host.create_plugin(plugin).await.expect("plugin");
    host.initialize_plugin(plugin).await.expect("init");
    let path = deployment.directory().join("odbc-batch-synthetic.log");
    std::fs::write(&path, "0|100\n1|200\n").expect("synthetic input");
    host.import_log(&path).await.expect("first");
    host.import_log(&path).await.expect("second");
    let page = host.encounters(0, 16).await.expect("encounters");
    let ids: Vec<_> = page.items.iter().map(|row| row.id).collect();
    assert_eq!(ids.len(), 2);
    host.capture_hierarchy_html(&[ids[0], ids[0]], TableLevel::Action)
        .await
        .expect_err("duplicate");
    let capture = host
        .capture_hierarchy_html(&[ids[1], ids[0]], TableLevel::Action)
        .await
        .expect("capture")
        .expect("selection");
    assert_eq!(capture.encounters(), ids);
    let columns = [
        vec!["enc_sql".into()],
        vec!["second".into()],
        vec!["first".into()],
        vec!["first".into()],
        vec!["second".into()],
    ];
    let plan = host
        .odbc_batch_preview(&capture, &columns, "All", &[])
        .await
        .expect("batch");
    assert_eq!(plan.tables.len(), 5);
    assert_eq!(plan.tables[0].parameters.len(), 2);
    let rule = aftercast::OdbcRewriteRule {
        connection_pattern: "^offline-preview$".into(),
        find: "encounter_table".into(),
        replacement: "synthetic_table".into(),
    };
    let rewritten = host
        .odbc_batch_preview(&capture, &columns, "All", &[rule])
        .await
        .expect("safe table rewrite");
    assert!(rewritten.tables[0].create_sql.contains("synthetic_table"));
    assert!(rewritten.tables[0].insert_sql.contains("synthetic_table"));
    assert_eq!(rewritten.tables[0].parameters, plan.tables[0].parameters);
    let chained = [
        aftercast::OdbcRewriteRule {
            connection_pattern: "offline-preview".into(),
            find: "encounter_table".into(),
            replacement: "first_name".into(),
        },
        aftercast::OdbcRewriteRule {
            connection_pattern: "offline-preview".into(),
            find: "first_name".into(),
            replacement: "second_name".into(),
        },
    ];
    let ordered = host
        .odbc_batch_preview(&capture, &columns, "All", &chained)
        .await
        .expect("ordered rules");
    assert!(ordered.tables[0].insert_sql.contains("second_name"));
    let skipped = aftercast::OdbcRewriteRule {
        connection_pattern: "^other$".into(),
        find: "encounter_table".into(),
        replacement: "ignored".into(),
    };
    let unchanged = host
        .odbc_batch_preview(&capture, &columns, "All", &[skipped])
        .await
        .expect("nonmatching connection");
    assert_eq!(unchanged.tables[0].insert_sql, plan.tables[0].insert_sql);
    let invalid = aftercast::OdbcRewriteRule {
        connection_pattern: ".*".into(),
        find: "(".into(),
        replacement: "x".into(),
    };
    host.odbc_batch_preview(&capture, &columns, "All", &[invalid])
        .await
        .expect_err("invalid regex");
    let unsafe_rule = aftercast::OdbcRewriteRule {
        connection_pattern: ".*".into(),
        find: r"\?".into(),
        replacement: "1".into(),
    };
    host.odbc_batch_preview(&capture, &columns, "All", &[unsafe_rule])
        .await
        .expect_err("placeholder rewriting rejected");
    assert!(
        plan.tables
            .iter()
            .all(|table| table.insert_sql.contains('?') && table.reset_sql.is_none())
    );
    let replacement = host
        .capture_hierarchy_html(&ids, TableLevel::Action)
        .await
        .expect("recapture")
        .expect("selection");
    host.odbc_batch_preview(&capture, &columns, "All", &[])
        .await
        .expect_err("stale");
    let old = host.handle();
    host.shutdown().await.expect("stop");
    old.odbc_batch_preview(&replacement, &columns, "All", &[])
        .await
        .expect_err("closed");
}
