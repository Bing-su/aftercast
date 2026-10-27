mod common;

use aftercast::{ActFormat, Error, Host, PluginConfig};
use camino::Utf8Path;
use common::{Deployment, deployment};

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn archives_survive_restart_and_round_trip_through_public_apis(deployment: Deployment) {
    let plugin = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let initialize = async |host: &mut Host| {
        let id = host
            .create_plugin(
                PluginConfig::builder()
                    .path(&plugin)
                    .type_name("Aftercast.PluginFixture.ImportPlugin")
                    .build(),
            )
            .await
            .expect("create category provider");
        host.initialize_plugin(id)
            .await
            .expect("initialize categories");
    };
    let mut host = Host::start(deployment.config())
        .await
        .expect("start archive host");
    initialize(&mut host).await;
    let log = deployment.directory().join("fixture.log");
    std::fs::write(&log, "0|100\n2|300\n").expect("write raw fixture");
    host.import_log(&log).await.expect("parse fixture");
    let result = host.encounters(0, 16).await.expect("retained encounters");
    let encounter = result.items.first().expect("parsed encounter");
    assert_eq!(encounter.damage, 400);
    let actions = host
        .encounter_actions(encounter.id, 0, 1)
        .await
        .expect("read actions")
        .expect("encounter");
    assert_eq!(actions.total, 2);
    assert_eq!(
        actions.items[0].source,
        aftercast::SourceLogLink::Retained(0)
    );
    let logs = host
        .encounter_logs(encounter.id, 0, 1)
        .await
        .expect("read logs")
        .expect("encounter");
    assert_eq!(logs.page.total, 2);
    assert_eq!(logs.dropped, 0);
    assert_eq!(logs.page.items[0].time_sorter, actions.items[0].time_sorter);
    assert_eq!(logs.page.items[0].text, "0|100");
    let old_id = encounter.id;
    let gzip = deployment.directory().join("encounter.act");
    let xml = deployment.directory().join("encounter.xml");
    host.export_act(old_id, &gzip, ActFormat::Gzip)
        .await
        .expect("export ACT gzip");
    host.export_act(old_id, &xml, ActFormat::Xml)
        .await
        .expect("export ACT XML");
    let exported = std::fs::read_to_string(&xml).expect("read encounter logs");
    assert_eq!(exported.matches("<LogLine ").count(), 2);
    assert!(exported.contains("Line=\"0|100\"") && exported.contains("Line=\"2|300\""));
    assert!(matches!(
        host.export_act(old_id, &gzip, ActFormat::Gzip).await,
        Err(Error::Managed { .. })
    ));
    assert_eq!(host.save_history().await.expect("checkpoint"), 1);
    host.shutdown().await.expect("stop before restart");

    let mut restarted = Host::start(deployment.config())
        .await
        .expect("restart with saved database");
    initialize(&mut restarted).await;
    assert!(matches!(
        restarted.export_act(old_id, &xml, ActFormat::Xml).await,
        Err(Error::InvalidArchiveRequest(_))
    ));
    assert_eq!(
        restarted
            .restore_history("missing")
            .await
            .expect("search history"),
        0
    );
    assert_eq!(
        restarted
            .restore_history("")
            .await
            .expect("restore checkpoint"),
        1
    );
    assert_eq!(
        restarted
            .restore_history("")
            .await
            .expect("restore without duplicates"),
        0
    );
    let restored = restarted
        .encounters(0, 16)
        .await
        .expect("query restored graph");
    assert_eq!(
        restored.items.first().expect("restored encounter").damage,
        400
    );
    let restored_id = restored.items[0].id;
    let actions = restarted
        .encounter_actions(restored_id, 1, 1)
        .await
        .expect("restored actions")
        .expect("encounter");
    assert_eq!(
        actions.items[0].source,
        aftercast::SourceLogLink::Retained(1)
    );
    let logs = restarted
        .encounter_logs(restored_id, 1, 1)
        .await
        .expect("restored logs")
        .expect("encounter");
    assert_eq!(logs.page.items[0].time_sorter, actions.items[0].time_sorter);
    assert_eq!(logs.page.items[0].text, "2|300");
    assert!(matches!(
        restarted.encounter_logs(old_id, 0, 1).await,
        Err(Error::InvalidCombatRequest(_))
    ));
    let restored_xml = deployment.directory().join("restored.xml");
    restarted
        .export_act(restored.items[0].id, &restored_xml, ActFormat::Xml)
        .await
        .expect("export restored encounter logs");
    let restored_xml = std::fs::read_to_string(&restored_xml).expect("read restored logs");
    assert_eq!(
        restored_xml
            .lines()
            .filter(|line| line.trim_start().starts_with("<LogLine "))
            .collect::<Vec<_>>(),
        exported
            .lines()
            .filter(|line| line.trim_start().starts_with("<LogLine "))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        restarted.import_act(&xml).await.expect("read exported XML"),
        1
    );
    assert_eq!(
        restarted
            .import_act(&gzip)
            .await
            .expect("read exported ACT"),
        1
    );
    assert_eq!(
        restarted
            .encounters(0, 16)
            .await
            .expect("all imported encounters")
            .total,
        3
    );
    restarted.shutdown().await.expect("stop archive host");
}

#[rstest::rstest]
#[case(true, false)]
#[case(true, true)]
#[case(false, false)]
#[case(false, true)]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
async fn recording_policy_applies_before_parsing_and_survives_host_restart(
    deployment: Deployment,
    #[case] record: bool,
    #[case] original: bool,
) {
    let path = deployment.directory().join("recording.log");
    std::fs::write(&path, "0|100\n").expect("fixture");
    let plugin = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    for _ in 0..2 {
        let host = Host::start(
            aftercast::HostConfig::builder()
                .application_executable(common::application())
                .dotnet_root(std::env::var("DOTNET_ROOT").expect("runtime"))
                .bridge_directory(&deployment.bridge)
                .data_directory(&deployment.state)
                .record_encounter_logs(record)
                .original_logline_view_logs(original)
                .build(),
        )
        .await
        .expect("recording host");
        let id = host
            .create_plugin(
                PluginConfig::builder()
                    .path(&plugin)
                    .type_name("Aftercast.PluginFixture.RecordingPlugin")
                    .build(),
            )
            .await
            .expect("plugin");
        host.initialize_plugin(id).await.expect("initialize parser");
        assert_eq!(host.import_log(&path).await.expect("import").damage, 100);
        let id = host.encounters(0, 1).await.expect("encounters").items[0].id;
        let logs = host
            .encounter_logs(id, 0, 1)
            .await
            .expect("logs")
            .expect("encounter");
        assert_eq!(logs.page.total, u32::from(record));
        assert_eq!(logs.dropped, 0);
        if record {
            assert_eq!(
                logs.page.items[0].text,
                if original { "0|100" } else { "processed:0|100" }
            );
        }
        host.shutdown().await.expect("shutdown");
    }
}

#[rstest::rstest]
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10; run mise run test:plugins"]
async fn explicit_export_log_policy_round_trips_and_preserves_private_history(
    deployment: Deployment,
) {
    let initialize = async |host: &Host| {
        let parser=host.create_plugin(PluginConfig::builder()
            .path(Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll"))
            .type_name("Aftercast.PluginFixture.ImportPlugin").build()).await.expect("fixture");
        host.initialize_plugin(parser).await.expect("initialize");
    };
    let host = Host::start(deployment.config()).await.expect("worker");
    initialize(&host).await;
    let source = deployment.directory().join("source.log");
    std::fs::write(&source, "1|10\n1|20\n3|30\n").expect("synthetic log");
    host.import_log(source).await.expect("seed");
    let id = host
        .encounters(0, 1)
        .await
        .expect("encounter")
        .items
        .first()
        .expect("row")
        .id;
    for (format, extension) in [(ActFormat::Xml, "xml"), (ActFormat::Gzip, "act")] {
        for include_logs in [true, false] {
            let path = deployment
                .directory()
                .join(format!("export-{include_logs}.{extension}"));
            let options = aftercast::ActExportOptions {
                format,
                include_logs,
            };
            host.export_act_with_options(id, &path, options)
                .await
                .expect("explicit export");
            if format == ActFormat::Xml {
                let text = std::fs::read_to_string(&path).expect("XML");
                assert_eq!(
                    text.matches("<LogLine ").count(),
                    if include_logs { 3 } else { 0 }
                );
            }
            let bytes = std::fs::read(&path).expect("written image");
            assert!(matches!(
                host.export_act_with_options(id, &path, options).await,
                Err(Error::Managed { .. })
            ));
            assert_eq!(
                std::fs::read(&path).expect("preserved existing file"),
                bytes
            );
            assert_eq!(
                host.import_act(&path).await.expect("interchange roundtrip"),
                1
            );
            let imported = host.encounters(0, 32).await.expect("imported graph");
            let last = imported.items.last().expect("last imported");
            assert_eq!(last.damage, 60);
            let logs = host
                .encounter_logs(last.id, 0, 16)
                .await
                .expect("logs query")
                .expect("imported encounter");
            assert_eq!(logs.page.total, if include_logs { 3 } else { 0 });
        }
    }
    assert_eq!(
        host.encounter_logs(id, 0, 16)
            .await
            .expect("source logs")
            .expect("original")
            .page
            .total,
        3
    );
    host.export_act_with_options(
        id,
        deployment.directory().join("missing/destination.xml"),
        aftercast::ActExportOptions::default(),
    )
    .await
    .expect_err("missing output parent");
    host.save_history().await.expect("precise checkpoint");
    host.shutdown().await.expect("stop");
    let host = Host::start(deployment.config()).await.expect("restart");
    initialize(&host).await;
    host.restore_history("")
        .await
        .expect("restore private history");
    let restored = host.encounters(0, 32).await.expect("history");
    let first = restored.items.first().expect("original restored");
    assert_eq!(
        host.encounter_logs(first.id, 0, 16)
            .await
            .expect("private logs")
            .expect("encounter")
            .page
            .total,
        3
    );
    host.shutdown().await.expect("close");
}
