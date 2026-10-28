use super::*;
use aftercast::{
    EncounterId, HierarchyHtmlGraphOptions, HierarchyHtmlLevel, HierarchyXmlDepth, TableLevel,
    TableScope, TableSort,
};
async fn start(deployment: &Deployment, text: &str) -> (Host, EncounterId) {
    let host = Host::start(deployment.config())
        .await
        .expect("isolated worker");
    let config = PluginConfig::builder()
        .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll",
        ))
        .type_name("Aftercast.PluginFixture.HierarchyHtmlPlugin")
        .build();
    let plugin = host.create_plugin(config).await.expect("synthetic plugin");
    host.initialize_plugin(plugin).await.expect("initialize");
    let path = deployment.directory().join("hierarchy HTML synthetic.log");
    std::fs::write(&path, text).expect("synthetic text");
    host.import_log(&path).await.expect("import");
    let id = host
        .encounters(0, 128)
        .await
        .expect("encounters")
        .items
        .first()
        .expect("encounter")
        .id;
    (host, id)
}
fn levels(last: TableLevel) -> Vec<HierarchyHtmlLevel> {
    [
        TableLevel::Combatant,
        TableLevel::Category,
        TableLevel::Attack,
        TableLevel::Action,
    ]
    .into_iter()
    .take(last as usize)
    .map(|level| HierarchyHtmlLevel {
        level,
        columns: vec!["registered-b".into(), "registered-a".into()],
        sort: None,
    })
    .collect()
}
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn html_worker_depths_tree_order_escaped_display_sort_and_session_ownership(
    deployment: Deployment,
) {
    let (host, first) = start(&deployment, "0|100\n1|200\n").await;
    let path = deployment.directory().join("second HTML synthetic.log");
    std::fs::write(&path, "100|100\n").expect("second source");
    host.import_log(&path).await.expect("second import");
    let ids: Vec<_> = host
        .encounters(0, 128)
        .await
        .expect("tree")
        .items
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(ids.len(), 2);
    let requested: Vec<_> = ids.iter().copied().rev().collect();
    let mut previous = None;
    let xml = host
        .capture_hierarchy_xml(first, HierarchyXmlDepth::MainTable)
        .await
        .expect("XML capture")
        .expect("scope");
    for last in [
        TableLevel::Combatant,
        TableLevel::Category,
        TableLevel::Attack,
        TableLevel::Action,
    ] {
        let choice = host
            .capture_hierarchy_html(&requested, last)
            .await
            .expect("HTML capture")
            .expect("scope");
        assert_eq!(choice.encounters(), ids);
        assert_eq!(choice.last_level(), last);
        if let Some(old) = previous.take() {
            host.hierarchy_html(&old, &levels(old.last_level()))
                .await
                .expect_err("old HTML revision retired");
        }
        let mut settings = levels(last);
        settings.first_mut().expect("main").sort = Some(TableSort {
            primary: "registered-a".into(),
            secondary: None,
            descending: true,
        });
        let report = host
            .hierarchy_html(&choice, &settings)
            .await
            .expect("render");
        assert_eq!(report.selection, choice);
        let source = &report.content;
        assert!(
            source.contains("default-src 'none'")
                && source.contains("base-uri 'none'")
                && source.contains("form-action 'none'")
        );
        assert!(
            source.contains("&lt;script&gt;")
                && source.contains("&lt;img")
                && source.contains("&quot;")
                && source.contains("日本")
                && source.contains("한글")
        );
        assert!(
            !source.contains("<script") && !source.contains("<img") && !source.contains(" src=\"")
        );
        assert!(source.contains("<th>registered-b</th><th>registered-a</th>"));
        assert!(!source.contains("<th>Label</th>"));
        assert_eq!(
            source.matches("<table>").count(),
            choice.table_count() as usize
        );
        let main = source.split("</table>").next().expect("main");
        assert!(
            main.find("<td>Target").expect("target") < main.find("<td>Source").expect("source")
        );
        assert_eq!(source.contains("<td>category</td>"), last as u32 >= 2);
        assert_eq!(source.contains("<td>attack</td>"), last as u32 >= 3);
        assert_eq!(source.contains("<td>action:"), last as u32 >= 4);
        assert_eq!(
            host.hierarchy_html(&choice, &settings)
                .await
                .expect("repeat")
                .content,
            report.content
        );
        host.hierarchy_xml(&xml)
            .await
            .expect("HTML capture does not retire XML");
        let current = host
            .table_report_catalog(&TableScope::Combatants(first))
            .await
            .expect("current capture")
            .expect("scope");
        assert!(!current.rows.is_empty());
        host.hierarchy_html(&choice, &settings)
            .await
            .expect("current-table capture does not retire HTML");
        previous = Some(choice);
    }
    let choice = previous.expect("selection");
    let old = host.handle();
    host.shutdown().await.expect("stop");
    assert!(matches!(
        old.hierarchy_html(&choice, &levels(choice.last_level()))
            .await,
        Err(Error::Closed)
    ));
    let (restarted, fresh) = start(&deployment, "0|100\n").await;
    assert!(matches!(
        restarted
            .capture_hierarchy_html(&ids, TableLevel::Combatant)
            .await,
        Err(Error::InvalidReportRequest(_))
    ));
    assert!(matches!(
        restarted
            .hierarchy_html(&choice, &levels(choice.last_level()))
            .await,
        Err(Error::InvalidReportRequest(_))
    ));
    let choice = restarted
        .capture_hierarchy_html(&[fresh], TableLevel::Combatant)
        .await
        .expect("new capture")
        .expect("scope");
    restarted
        .hierarchy_html(&choice, &levels(TableLevel::Combatant))
        .await
        .expect("new session render");
    restarted.shutdown().await.expect("shutdown");
}
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn html_worker_errors_timeout_removed_scope_and_explicit_recovery(deployment: Deployment) {
    for (amount, message) in [
        (444, "exceeded"),
        (555, "scope"),
        (666, "synthetic hierarchy display failure"),
        (777, ""),
        (888, "oversized report cell"),
        (333, "synthetic hierarchy action display failure"),
        (222, "synthetic hierarchy sort failure"),
    ] {
        let (host, id) = start(&deployment, &format!("0|{amount}\n")).await;
        let choice = host
            .capture_hierarchy_html(&[id], TableLevel::Action)
            .await
            .expect("capture")
            .expect("scope");
        let mut settings = levels(TableLevel::Action);
        if amount == 222 {
            settings.first_mut().expect("main").sort = Some(TableSort {
                primary: "registered-a".into(),
                secondary: None,
                descending: false,
            });
        }
        let error = host
            .hierarchy_html(&choice, &settings)
            .await
            .expect_err("explicit callback/budget failure")
            .to_string();
        if amount == 777 {
            assert!(
                error.contains("Unicode")
                    || error.contains("surrogate")
                    || error.contains("translate"),
                "{error}"
            );
        } else {
            assert!(error.contains(message), "{error}");
        }
        if amount == 555 {
            assert!(
                host.capture_hierarchy_html(&[id], TableLevel::Combatant)
                    .await
                    .expect("removed capture")
                    .is_none()
            );
            host.hierarchy_html(&choice, &settings)
                .await
                .expect_err("missing capture retired revision");
        } else if amount == 444 {
            host.hierarchy_html(&choice, &settings)
                .await
                .expect("returning timeout allows explicit same-selection retry");
        } else if amount == 333 || amount == 222 {
            let shallow = host
                .capture_hierarchy_html(&[id], TableLevel::Combatant)
                .await
                .expect("shallow capture")
                .expect("scope");
            host.hierarchy_html(&shallow, &levels(TableLevel::Combatant))
                .await
                .expect("explicit shallow/no-sort recovery");
        } else {
            let shallow = host
                .capture_hierarchy_xml(id, HierarchyXmlDepth::MainTable)
                .await
                .expect("independent XML capture")
                .expect("scope");
            assert_eq!(shallow.encounter(), id);
        }
        host.shutdown().await.expect("shutdown");
    }
}
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn html_worker_maximum_selected_set_and_output_budget_failure_recover(
    deployment: Deployment,
) {
    let (host, _) = start(&deployment, "0|6400\n").await;
    let ids: Vec<_> = host
        .encounters(0, 128)
        .await
        .expect("64 synthetic encounters")
        .items
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(ids.len(), 64);
    let requested: Vec<_> = ids.iter().copied().rev().collect();
    let selected = host
        .capture_hierarchy_html(&requested, TableLevel::Combatant)
        .await
        .expect("64 capture")
        .expect("scope");
    assert_eq!(selected.encounters(), ids);
    assert_eq!(selected.table_count(), 64);
    let output = host
        .hierarchy_html(&selected, &levels(TableLevel::Combatant))
        .await
        .expect("64 small tables");
    assert_eq!(output.content.matches("<table>").count(), 64);
    let mut too_many = ids.clone();
    too_many.push(*ids.first().expect("id"));
    assert!(matches!(
        host.capture_hierarchy_html(&too_many, TableLevel::Combatant)
            .await,
        Err(Error::InvalidReportRequest(_))
    ));
    host.shutdown().await.expect("shutdown");
    let text: String = (0..8).map(|n| format!("{n}|999\n")).collect();
    let (host, id) = start(&deployment, &text).await;
    let selected = host
        .capture_hierarchy_html(&[id], TableLevel::Action)
        .await
        .expect("capture")
        .expect("scope");
    let mut expanded = levels(TableLevel::Action);
    expanded.last_mut().expect("action level").columns =
        (0..32).map(|index| format!("expanded-{index}")).collect();
    let error = host
        .hierarchy_html(&selected, &expanded)
        .await
        .expect_err("serialized output bound")
        .to_string();
    assert!(
        error.contains("Serialized table report exceeds 16 MiB"),
        "{error}"
    );
    let shallow = host
        .capture_hierarchy_html(&[id], TableLevel::Combatant)
        .await
        .expect("shallow")
        .expect("scope");
    host.hierarchy_html(&shallow, &levels(TableLevel::Combatant))
        .await
        .expect("explicit shallow recovery");
    host.shutdown().await.expect("shutdown");
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn html_worker_explicit_graphs_with_independent_table_depth(deployment: Deployment) {
    let (host, id) = start(&deployment, "0|100\n3|200\n").await;
    let choice = host
        .capture_hierarchy_html(&[id], TableLevel::Action)
        .await
        .unwrap()
        .unwrap();
    let mut options = HierarchyHtmlGraphOptions {
        window: 1,
        rolling: true,
        distribution_column: "Resist".into(),
        action_sort_column: "registered-a".into(),
        all_label: "All".into(),
        ward_prefix: "Warded/".into(),
        selected_combatants: None,
        solo_categories: None,
        levels: vec![TableLevel::Combatant],
        simple: false,
        simple_column: String::new(),
        only_allies: true,
        totals: true,
    };
    let report = host
        .hierarchy_html_with_graphs(&choice, &levels(TableLevel::Combatant), Some(&options))
        .await
        .unwrap();
    assert_eq!(report.content.matches("<table>").count(), 1);
    assert!(
        report.content.contains("<svg")
            && report.content.contains("<polyline")
            && report.content.contains("default-src 'none'")
    );
    assert!(!report.content.contains("<script") && !report.content.contains(" src=\""));
    options.levels = vec![
        TableLevel::Combatant,
        TableLevel::Category,
        TableLevel::Attack,
        TableLevel::Action,
    ];
    let report = host
        .hierarchy_html_with_graphs(&choice, &levels(TableLevel::Action), Some(&options))
        .await
        .unwrap();
    assert!(report.content.contains("<rect") && report.content.contains("A100 100"));
    options.ward_prefix.clear();
    host.hierarchy_html_with_graphs(&choice, &levels(TableLevel::Action), Some(&options))
        .await
        .expect_err("empty ward prefix");
    options.ward_prefix = "Warded/".into();
    options.levels = vec![TableLevel::Action];
    host.hierarchy_html_with_graphs(&choice, &levels(TableLevel::Combatant), Some(&options))
        .await
        .expect_err("graph requires corresponding table");
    let report = host
        .hierarchy_html_with_graphs(&choice, &levels(TableLevel::Action), Some(&options))
        .await
        .unwrap();
    assert!(report.content.contains("<rect") && !report.content.contains("<polyline"));
    host.shutdown().await.unwrap();
}
