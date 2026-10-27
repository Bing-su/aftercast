use super::*;
use crate::config::ConfigFile;
use camino::Utf8Path;
fn preferences() -> Preferences {
    Preferences {
        order: vec![
            "probe".into(),
            "fixture".into(),
            "bad".into(),
            "null".into(),
            "mutate".into(),
        ],
        visible: vec!["fixture".into(), "probe".into()],
        primary: Some("fixture".into()),
        secondary: Some("probe".into()),
        descending: true,
    }
}
#[test]
fn column_preferences_validate_keys_not_labels_and_allow_all_hidden() {
    let valid = preferences();
    valid.validate().expect("valid");
    assert_eq!(valid.selected(), ["probe", "fixture"]);
    Preferences {
        visible: vec![],
        ..valid.clone()
    }
    .validate()
    .expect("all hidden is a deliberate layout");
    for invalid in [
        Preferences {
            order: vec!["same".into(), "same".into()],
            ..Default::default()
        },
        Preferences {
            order: vec!["NUL\0".into()],
            ..Default::default()
        },
        Preferences {
            order: vec!["x".repeat(1025)],
            ..Default::default()
        },
        Preferences {
            visible: vec!["unknown".into()],
            ..valid.clone()
        },
        Preferences {
            visible: vec!["fixture".into(), "fixture".into()],
            ..valid.clone()
        },
        Preferences {
            primary: None,
            secondary: Some("fixture".into()),
            ..valid.clone()
        },
        Preferences {
            primary: Some("unknown".into()),
            ..valid
        },
        Preferences {
            order: (0..33).map(|index| format!("key{index}")).collect(),
            visible: (0..33).map(|index| format!("key{index}")).collect(),
            ..Default::default()
        },
    ] {
        invalid
            .validate()
            .expect_err("invalid layout must not persist");
    }
}
#[test]
fn table_settings_save_reload_annotations_clear_and_failed_save_are_atomic() {
    let temporary = tempfile::tempdir().expect("config fixture");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8");
    let original = "# user annotation\nschema_version = 1\n[unrelated]\nkeep = 'untouched'\n[tables.combatant]\norder = ['fixture', 'probe'] # order note\nvisible = ['fixture'] # visible note\n";
    std::fs::write(root.join("config.toml"), original).expect("source");
    let mut file = ConfigFile::load(root).expect("load");
    let expected = preferences();
    file.settings
        .tables
        .insert("combatant".into(), expected.clone());
    let saved = file.save(root).expect("save");
    let content = std::fs::read_to_string(root.join("config.toml")).expect("saved");
    assert!(
        content.contains("# user annotation")
            && content.contains("# order note")
            && content.contains("# visible note")
    );
    assert!(content.contains("keep = 'untouched'"));
    assert_eq!(
        ConfigFile::load(root)
            .expect("reload")
            .settings
            .tables
            .get("combatant"),
        Some(&expected)
    );
    let mut edited = saved.clone();
    edited
        .settings
        .tables
        .get_mut("combatant")
        .expect("layout")
        .primary = None;
    edited
        .settings
        .tables
        .get_mut("combatant")
        .expect("layout")
        .secondary = None;
    edited.save(root).expect("remove sort");
    assert!(
        ConfigFile::load(root)
            .expect("no sort")
            .settings
            .tables
            .get("combatant")
            .expect("layout")
            .primary
            .is_none()
    );
    let before = std::fs::read(root.join("config.toml")).expect("prior file");
    let mut invalid = saved.clone();
    invalid
        .settings
        .tables
        .insert("unknown-level".into(), Preferences::default());
    invalid
        .save(root)
        .err()
        .expect("invalid level must not overwrite disk");
    assert_eq!(
        std::fs::read(root.join("config.toml")).expect("after failure"),
        before
    );
    // A file cannot be used as a temporary-file directory; failure keeps the valid source config and caller clone unchanged.
    let obstacle = root.join("not-a-directory");
    std::fs::write(&obstacle, "owned obstacle").expect("obstacle");
    saved.clone().save(&obstacle).err().expect("I/O failure");
    assert_eq!(saved.settings.tables.get("combatant"), Some(&expected));
    assert_eq!(
        std::fs::read(root.join("config.toml")).expect("unchanged config"),
        before
    );
    let mut cleared = saved;
    cleared.settings.tables.clear();
    cleared.save(root).expect("reset layouts");
    assert!(
        ConfigFile::load(root)
            .expect("cleared")
            .settings
            .tables
            .is_empty()
    );
}
#[rstest::rstest]
#[case("[tables.combatant]\norder = ['a', 'a']")]
#[case("[tables.combatant]\norder = ['a']\nvisible = ['unknown']")]
#[case("[tables.unknown]\norder = []")]
fn invalid_table_settings_do_not_rewrite_source(#[case] source: &str) {
    let temporary = tempfile::tempdir().expect("invalid fixture");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8");
    std::fs::write(root.join("config.toml"), source).expect("source");
    ConfigFile::load(root).err().expect("invalid config");
    assert_eq!(
        std::fs::read_to_string(root.join("config.toml")).expect("preserved"),
        source
    );
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires .NET 10 and synthetic plugin; run mise run test:gui-state"]
async fn table_settings_ui_contract_persists_only_keys_across_plugin_and_worker_changes() {
    use aftercast::{Host, PluginConfig, TableLevel};
    let temporary = tempfile::tempdir().expect("isolated settings Host");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8 root");
    let config = contract_config(root);
    let plugin =
        || {
            PluginConfig::builder().path(Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../managed/tests/PluginFixture/bin/Release/net10.0/Aftercast.PluginFixture.dll"))
        .type_name("Aftercast.PluginFixture.ColumnsPlugin").build()
        };
    let host = Host::start(config.clone()).await.expect("host");
    let id = host.create_plugin(plugin()).await.expect("plugin");
    host.initialize_plugin(id).await.expect("initialize");
    let catalog = host
        .table_columns(TableLevel::Combatant)
        .await
        .expect("catalog");
    let first = catalog
        .columns
        .iter()
        .find(|column| column.key == "fixture")
        .expect("fixture");
    let probe = catalog
        .columns
        .iter()
        .find(|column| column.key == "probe")
        .expect("probe");
    assert_eq!(
        first.label, probe.label,
        "same label must not merge two preferences"
    );
    let mut file = ConfigFile::load(root).expect("settings");
    file.settings
        .tables
        .insert("combatant".into(), preferences());
    file.save(root).expect("save layout");
    let saved = ConfigFile::load(root).expect("saved layout");
    let preference = saved.settings.tables.get("combatant").expect("profile");
    assert_eq!(preference.selected(), ["probe", "fixture"]);
    host.deinitialize_plugin(id).await.expect("disable");
    assert!(
        host.table_columns(TableLevel::Combatant)
            .await
            .expect("missing keys")
            .columns
            .is_empty()
    );
    assert_eq!(
        ConfigFile::load(root)
            .expect("disk was not silently changed")
            .settings
            .tables
            .get("combatant"),
        Some(preference)
    );
    let next = host.create_plugin(plugin()).await.expect("fresh plugin");
    host.initialize_plugin(next).await.expect("reenable");
    assert!(
        host.table_columns(TableLevel::Combatant)
            .await
            .expect("returning keys")
            .columns
            .iter()
            .any(|column| column.key == "probe")
    );
    host.shutdown().await.expect("shutdown");
    let host = Host::start(config).await.expect("restart");
    let id = host.create_plugin(plugin()).await.expect("restart plugin");
    host.initialize_plugin(id).await.expect("initialize");
    assert_eq!(
        ConfigFile::load(root)
            .expect("restart preferences")
            .settings
            .tables
            .get("combatant"),
        Some(preference)
    );
    assert!(
        host.table_columns(TableLevel::Combatant)
            .await
            .expect("restart metadata")
            .columns
            .iter()
            .any(|column| column.key == "fixture")
    );
    host.shutdown().await.expect("cleanup");
}

fn contract_config(root: &Utf8Path) -> aftercast::HostConfig {
    let bridge = root.join("bridge");
    aftercast::extract_bridge(&bridge).expect("payload");
    let executable = std::env::current_exe().expect("test binary");
    let application = executable
        .parent()
        .and_then(std::path::Path::parent)
        .expect("target")
        .join("examples")
        .join(format!("test-application{}", std::env::consts::EXE_SUFFIX));
    aftercast::HostConfig::builder()
        .dotnet_root(std::env::var("DOTNET_ROOT").expect("dotnet"))
        .bridge_directory(bridge.clone())
        .data_directory(root.join("state"))
        .application_executable(
            camino::Utf8PathBuf::try_from(application.clone()).expect("UTF8 app"),
        )
        .build()
}
