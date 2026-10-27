use super::*;
use aftercast::{HierarchyXmlDepth, TableReportFormat, TableReportOptions, TableScope};
async fn start(deployment: &Deployment, text: &str) -> (Host, aftercast::EncounterId) {
    let host = Host::start(deployment.config())
        .await
        .expect("isolated worker");
    let config = PluginConfig::builder()
        .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll",
        ))
        .type_name("Aftercast.PluginFixture.HierarchyXmlPlugin")
        .build();
    let id = host.create_plugin(config).await.expect("synthetic plugin");
    host.initialize_plugin(id).await.expect("initialize");
    let source = deployment.directory().join("hierarchy synthetic.log");
    std::fs::write(&source, text).expect("synthetic text");
    host.import_log(&source).await.expect("import");
    let id = host
        .encounters(0, 16)
        .await
        .expect("encounters")
        .items
        .first()
        .expect("encounter")
        .id;
    (host, id)
}
fn values<'a>(source: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    source
        .split(&open)
        .skip(1)
        .map(|value| value.split(&close).next().expect("field"))
        .collect()
}
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn hierarchy_depth_sql_caption_ids_and_session_ownership(deployment: Deployment) {
    let (host, encounter) = start(&deployment, "0|100\n1|200\n").await;
    let current = host
        .table_report_catalog(&TableScope::Combatants(encounter))
        .await
        .expect("current catalog")
        .expect("scope");
    let options = TableReportOptions {
        format: TableReportFormat::TableXml,
        name: "current".into(),
        selected_rows: vec![],
        columns: None,
        sort: None,
    };
    let mut previous = None;
    for depth in [
        HierarchyXmlDepth::MainTable,
        HierarchyXmlDepth::DamageTypeTables,
        HierarchyXmlDepth::AllTables,
    ] {
        let selection = host
            .capture_hierarchy_xml(encounter, depth)
            .await
            .expect("capture")
            .expect("scope");
        if let Some(old) = previous.take() {
            host.hierarchy_xml(&old)
                .await
                .expect_err("old revision retired");
        }
        assert_eq!(selection.encounter(), encounter);
        assert_eq!(selection.depth(), depth);
        let report = host.hierarchy_xml(&selection).await.expect("render");
        assert_eq!(report.selection, selection);
        let source = &report.content;
        assert!(
            source.starts_with(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>\r\n<EncounterTable Name=\""
            )
        );
        assert!(source.contains("&#xD;&#xA;"), "caption CRLF: {source}");
        assert!(source.contains("한글😀"));
        assert!(source.contains("&lt;&amp;&gt;"));
        assert_eq!(
            source.matches("<Row>").count(),
            selection.row_count() as usize
        );
        assert_eq!(
            source.matches("<CombatantTable ").count() > 0,
            depth != HierarchyXmlDepth::MainTable
        );
        assert_eq!(
            source.matches("<DamageTypeTable ").count() > 0,
            depth != HierarchyXmlDepth::MainTable
        );
        assert_eq!(
            source.matches("<AttackTypeTable ").count() > 0,
            depth == HierarchyXmlDepth::AllTables
        );
        let first = source.find("<first>").expect("SQL first");
        let second = source.find("<second>").expect("SQL second");
        assert!(first < second);
        assert!(!source.contains("registered-a"));
        assert!(!source.contains("display callback"));
        let id = values(source, "first")
            .first()
            .copied()
            .expect("root encid");
        assert_eq!(id.len(), 8);
        assert!(id.bytes().all(|byte| byte.is_ascii_hexdigit()));
        for table in source.split("<AttackTypeTable ").skip(1) {
            let body = table.split("</AttackTypeTable>").next().expect("table");
            assert!(values(body, "first").iter().all(|value| *value == id));
        }
        assert_eq!(
            host.hierarchy_xml(&selection)
                .await
                .expect("repeat")
                .content,
            report.content
        );
        host.table_report(&current.selection, &options)
            .await
            .expect("independent current table capture");
        previous = Some(selection);
    }
    let selection = previous.expect("last selection");
    let old = host.handle();
    host.shutdown().await.expect("stop");
    assert!(matches!(
        old.hierarchy_xml(&selection).await,
        Err(Error::Closed)
    ));
    let (restarted, new_encounter) = start(&deployment, "0|100\n").await;
    assert!(matches!(
        restarted.hierarchy_xml(&selection).await,
        Err(Error::InvalidReportRequest(_))
    ));
    assert!(matches!(
        restarted
            .capture_hierarchy_xml(encounter, HierarchyXmlDepth::MainTable)
            .await,
        Err(Error::InvalidReportRequest(_))
    ));
    let fresh = restarted
        .capture_hierarchy_xml(new_encounter, HierarchyXmlDepth::AllTables)
        .await
        .expect("new capture")
        .expect("scope");
    restarted
        .hierarchy_xml(&fresh)
        .await
        .expect("new session render");
    restarted.shutdown().await.expect("final stop");
}
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn hierarchy_worker_errors_timeout_and_explicit_recovery(deployment: Deployment) {
    for (amount, message) in [
        (444, "exceeded"),
        (555, "scope"),
        (666, "synthetic hierarchy SQL failure"),
        (777, "Unicode"),
        (888, "oversized report cell"),
        (333, "synthetic first action SQL failure"),
    ] {
        let (host, encounter) = start(&deployment, &format!("0|{amount}\n")).await;
        let selection = host
            .capture_hierarchy_xml(encounter, HierarchyXmlDepth::AllTables)
            .await
            .expect("capture")
            .expect("scope");
        let error = host
            .hierarchy_xml(&selection)
            .await
            .expect_err("explicit failure");
        let diagnostic = error.to_string();
        if amount == 777 {
            assert!(
                diagnostic.contains("Unicode")
                    || diagnostic.contains("translate")
                    || diagnostic.contains("surrogate"),
                "{diagnostic}"
            );
        } else {
            assert!(diagnostic.contains(message), "{amount}: {diagnostic}");
        }
        if amount == 555 {
            assert!(
                host.capture_hierarchy_xml(encounter, HierarchyXmlDepth::MainTable)
                    .await
                    .expect("removed encounter")
                    .is_none()
            );
            host.hierarchy_xml(&selection)
                .await
                .expect_err("missing capture retires old selection");
        } else if amount == 444 {
            let recovered = host
                .hierarchy_xml(&selection)
                .await
                .expect("explicit retry after one synthetic delay");
            assert_eq!(recovered.selection, selection);
        } else {
            let small = host
                .capture_hierarchy_xml(encounter, HierarchyXmlDepth::MainTable)
                .await
                .expect("recapture")
                .expect("scope");
            assert_eq!(small.table_count(), 1);
        }
        host.shutdown().await.expect("stop after failure");
    }
}
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn hierarchy_worker_output_budget_is_error_and_shallower_capture_recovers(
    deployment: Deployment,
) {
    let text = (0..200)
        .map(|index| format!("{index}|999\n"))
        .collect::<String>();
    let (host, encounter) = start(&deployment, &text).await;
    let selection = host
        .capture_hierarchy_xml(encounter, HierarchyXmlDepth::AllTables)
        .await
        .expect("capture")
        .expect("scope");
    let error = host
        .hierarchy_xml(&selection)
        .await
        .expect_err("large escaped source cannot become partial success");
    assert!(
        error
            .to_string()
            .contains("Serialized table report exceeds 16 MiB"),
        "{error}"
    );
    let small = host
        .capture_hierarchy_xml(encounter, HierarchyXmlDepth::MainTable)
        .await
        .expect("small capture")
        .expect("scope");
    let report = host.hierarchy_xml(&small).await.expect("recovered");
    assert!(!report.content.contains("<AttackTypeTable"));
    host.shutdown().await.expect("stop");
}
