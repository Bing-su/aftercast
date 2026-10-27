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
