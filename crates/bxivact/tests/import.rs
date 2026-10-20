use bxivact::{Error, Host, HostConfig, ImportReport, PluginConfig, extract_bridge};
use camino::Utf8Path;

#[test]
#[ignore = "requires .NET 10 and C# fixtures; run mise run test:plugins"]
fn imports_drain_actions_and_recover_after_input_and_plugin_failures() {
    let root = std::env::var("DOTNET_ROOT").expect("set a UTF-8 DOTNET_ROOT");
    let temporary = tempfile::tempdir().expect("create import fixture");
    let directory = Utf8Path::from_path(temporary.path()).expect("UTF-8 test directory");
    let bridge = directory.join("bridge");
    extract_bridge(&bridge).expect("extract prebuilt component");
    let mut host = Host::start(
        HostConfig::builder()
            .dotnet_root(root)
            .bridge_directory(&bridge)
            .data_directory(directory.join("state"))
            .build(),
    )
    .expect("start import host");
    let log = directory.join("input 한글.log");
    std::fs::write(&log, "\u{feff}0|100\r\n\n2|300").expect("write import records");
    assert!(matches!(
        host.import_log(&log),
        Err(Error::Managed { command: 8, .. })
    ));
    let path = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Bxivact.PluginFixture.dll");
    let plugin = host
        .create_plugin(
            PluginConfig::builder()
                .path(path)
                .type_name("Bxivact.PluginFixture.ImportPlugin")
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
