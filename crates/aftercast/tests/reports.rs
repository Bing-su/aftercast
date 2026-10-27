mod common;
use aftercast::{Error, Host, PluginConfig, TextReportOptions};
use camino::Utf8Path;
use common::{Deployment, deployment};
fn plugin() -> PluginConfig {
    PluginConfig::builder()
        .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll",
        ))
        .type_name("Aftercast.PluginFixture.TextReportPlugin")
        .build()
}
#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and fixture; run mise run test:plugins"]
async fn owned_reports_render_once_fail_explicitly_and_reject_stale_sessions(
    deployment: Deployment,
) {
    let host = Host::start(deployment.config()).await.expect("worker");
    let cold = host.text_report_catalog().await.expect("cold catalog");
    let plugin_id = host.create_plugin(plugin()).await.expect("plugin");
    host.initialize_plugin(plugin_id).await.expect("init");
    let catalog = host.text_report_catalog().await.expect("catalog");
    assert_ne!(cold.revision, catalog.revision);
    assert!(catalog.player_variables.contains(&"report-unicode".into()));
    assert!(catalog.allied_variables.contains(&"report-group".into()));
    assert!(catalog.sort_columns.contains(&"report-name".into()));
    assert!(
        !catalog
            .sort_columns
            .contains(&"Displayed report name".into())
    );
    let path = deployment.directory().join("reports.log");
    std::fs::write(&path, "0|100\n1|300\n").expect("synthetic input");
    host.import_log(&path).await.expect("import");
    let encounter = host.encounters(0, 16).await.expect("encounters").items[0].id;
    let options = TextReportOptions {
        player_format: "{report-name}:{report-unicode}:{report-count}".into(),
        sorting: "report-name".into(),
        show_allied_info: true,
        allies_format: "{report-group}:".into(),
        ..Default::default()
    };
    let report = host
        .text_report(encounter, catalog.revision, &options)
        .await
        .expect("report")
        .expect("selected");
    assert_eq!(report.encounter, encounter);
    assert_eq!(report.revision, catalog.revision);
    assert_eq!(
        report.text,
        "group:Target:한글日本😀:1Source:한글日本😀:2\n"
    );
    let again = host
        .text_report(encounter, catalog.revision, &options)
        .await
        .expect("second")
        .expect("report");
    assert_eq!(again.text, "group:Target:한글日本😀:3Source:한글日本😀:4\n");
    for template in ["{report-fail}", "{report-invalid}", "{report-limit}"] {
        let options = TextReportOptions {
            player_format: template.into(),
            ..Default::default()
        };
        assert!(
            host.text_report(encounter, catalog.revision, &options)
                .await
                .is_err(),
            "{template}"
        );
    }
    let bad = TextReportOptions {
        player_format: "x".repeat(16385),
        ..Default::default()
    };
    assert!(matches!(
        host.text_report(encounter, catalog.revision, &bad).await,
        Err(Error::InvalidReportRequest(_))
    ));
    host.text_report(
        encounter,
        catalog.revision,
        &TextReportOptions {
            player_format: "{report-mutate}".into(),
            ..Default::default()
        },
    )
    .await
    .expect_err("registry mutation rejected");
    assert!(
        host.text_report(encounter, catalog.revision, &options)
            .await
            .is_err(),
        "old registry rejected"
    );
    let fresh = host.text_report_catalog().await.expect("fresh revision");
    assert_ne!(fresh.revision, catalog.revision);
    assert!(fresh.player_variables.contains(&"report-added".into()));
    let recovered = host
        .text_report(
            encounter,
            fresh.revision,
            &TextReportOptions {
                player_format: "{report-added}".into(),
                ..Default::default()
            },
        )
        .await
        .expect("recover")
        .expect("report");
    assert_eq!(recovered.text, "addedadded\n");
    assert!(
        host.text_report(
            encounter,
            fresh.revision,
            &TextReportOptions {
                player_format: "{report-remove}".into(),
                ..Default::default()
            }
        )
        .await
        .is_err(),
        "scope removal rejected"
    );
    assert!(
        host.text_report(encounter, fresh.revision, &options)
            .await
            .expect("removed selection")
            .is_none(),
        "no replacement encounter"
    );
    let old_handle = host.handle();
    host.shutdown().await.expect("stop");
    assert!(matches!(
        old_handle.text_report_catalog().await,
        Err(Error::Closed)
    ));
    let restarted = Host::start(deployment.config()).await.expect("restart");
    assert!(matches!(
        restarted
            .text_report(encounter, fresh.revision, &options)
            .await,
        Err(Error::InvalidReportRequest(_))
    ));
    let id = restarted.create_plugin(plugin()).await.expect("new plugin");
    restarted.initialize_plugin(id).await.expect("new init");
    let new_catalog = restarted
        .text_report_catalog()
        .await
        .expect("restart catalog");
    restarted.import_log(&path).await.expect("restart import");
    let new_encounter = restarted
        .encounters(0, 16)
        .await
        .expect("new encounters")
        .items[0]
        .id;
    assert!(matches!(
        restarted
            .text_report(new_encounter, fresh.revision, &options)
            .await,
        Err(Error::InvalidReportRequest(_))
    ));
    assert!(
        restarted
            .text_report(new_encounter, new_catalog.revision, &options)
            .await
            .expect("new report")
            .is_some()
    );
    restarted.shutdown().await.expect("final stop");
}
