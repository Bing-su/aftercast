mod common;

use aftercast::{ActFormat, Error, Host, PluginConfig};
use camino::Utf8Path;
use common::{Deployment, deployment};

#[rstest::rstest]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
fn archives_survive_restart_and_round_trip_through_public_apis(deployment: Deployment) {
    let plugin = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll");
    let initialize = |host: &mut Host| {
        let id = host
            .create_plugin(
                PluginConfig::builder()
                    .path(&plugin)
                    .type_name("Aftercast.PluginFixture.ImportPlugin")
                    .build(),
            )
            .expect("create category provider");
        host.initialize_plugin(id).expect("initialize categories");
    };
    let mut host = Host::start(deployment.config()).expect("start archive host");
    initialize(&mut host);
    let log = deployment.directory().join("fixture.log");
    std::fs::write(&log, "0|100\n2|300\n").expect("write raw fixture");
    host.import_log(&log).expect("parse fixture");
    let result = host.encounters(0, 16).expect("retained encounters");
    let encounter = result.items.first().expect("parsed encounter");
    assert_eq!(encounter.damage, 400);
    let old_id = encounter.id;
    let gzip = deployment.directory().join("encounter.act");
    let xml = deployment.directory().join("encounter.xml");
    host.export_act(old_id, &gzip, ActFormat::Gzip)
        .expect("export ACT gzip");
    host.export_act(old_id, &xml, ActFormat::Xml)
        .expect("export ACT XML");
    assert!(matches!(
        host.export_act(old_id, &gzip, ActFormat::Gzip),
        Err(Error::Managed { .. })
    ));
    assert_eq!(host.save_history().expect("checkpoint"), 1);
    host.stop().expect("stop before restart");

    let mut restarted = Host::start(deployment.config()).expect("restart with saved database");
    initialize(&mut restarted);
    assert!(matches!(
        restarted.export_act(old_id, &xml, ActFormat::Xml),
        Err(Error::InvalidArchiveRequest(_))
    ));
    assert_eq!(
        restarted
            .restore_history("missing")
            .expect("search history"),
        0
    );
    assert_eq!(
        restarted.restore_history("").expect("restore checkpoint"),
        1
    );
    assert_eq!(
        restarted
            .restore_history("")
            .expect("restore without duplicates"),
        0
    );
    let restored = restarted.encounters(0, 16).expect("query restored graph");
    assert_eq!(
        restored.items.first().expect("restored encounter").damage,
        400
    );
    assert_eq!(restarted.import_act(&xml).expect("read exported XML"), 1);
    assert_eq!(restarted.import_act(&gzip).expect("read exported ACT"), 1);
    assert_eq!(
        restarted
            .encounters(0, 16)
            .expect("all imported encounters")
            .total,
        3
    );
    restarted.stop().expect("stop archive host");
}
