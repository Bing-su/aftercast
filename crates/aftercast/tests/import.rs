mod common;

use aftercast::{Error, Host, ImportReport, PluginConfig};
use camino::Utf8Path;
use common::{Deployment, deployment};

#[rstest::rstest]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
fn imports_drain_actions_and_recover_after_input_and_plugin_failures(deployment: Deployment) {
    let directory = deployment.directory();
    let mut host = Host::start(deployment.config()).expect("start import host");
    let log = directory.join("input 한글.log");
    std::fs::write(&log, "\u{feff}0|100\r\n\n2|300").expect("write import records");
    assert!(matches!(
        host.import_log(&log),
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
        .expect("construct import fixture plugin");
    host.initialize_plugin(plugin)
        .expect("initialize fixture parser");
    let expected = ImportReport {
        lines_read: 3,
        records_dispatched: 2,
        encounters: 1,
        combatants: 2,
        damage: 400,
        healed: 0,
    };
    assert_eq!(
        host.import_log(&log)
            .expect("import and drain accepted actions"),
        expected
    );
    assert_eq!(
        host.import_log(&log)
            .expect("repeat with isolated import state"),
        expected
    );
    std::fs::write(&log, [0xff]).expect("write invalid UTF-8");
    assert!(matches!(
        host.import_log(&log),
        Err(Error::Managed { command: 8, .. })
    ));
    std::fs::write(&log, "not-a-timestamp|100\n").expect("write parser failure");
    assert!(matches!(
        host.import_log(&log),
        Err(Error::Managed { command: 8, .. })
    ));
    std::fs::write(&log, "0|100\n2|300\n").expect("restore valid input");
    assert_eq!(
        host.import_log(&log).expect("recover after failures"),
        ImportReport {
            lines_read: 2,
            ..expected
        }
    );
    host.deinitialize_plugin(plugin)
        .expect("deinitialize parser");
    assert!(matches!(
        host.import_log(&log),
        Err(Error::Managed { command: 8, .. })
    ));
    host.stop().expect("stop import host");
}
