use super::*;

#[rstest::rstest]
#[case("[audio]\ntimer_start = ['tts', ''] # cue note\n")]
#[case("audio = { timer_start = ['tts', ''] } # audio note\n")]
fn sequence_form_settings_remain_values_inside_inline_tables(#[case] source: &str) {
    let temporary = tempfile::tempdir().expect("config fixture");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8");
    fs::write(root.join("config.toml"), source).expect("fixture");
    let file = ConfigFile::load(root).expect("load sequence form accepted by Serde");
    assert_eq!(file.settings.audio.timer_start.mode, "tts");
    file.save(root).expect("save");
    let loaded = ConfigFile::load(root).expect("reload");
    assert_eq!(loaded.settings.audio.timer_start.mode, "tts");
    let saved = fs::read_to_string(root.join("config.toml")).expect("saved text");
    assert!(saved.contains(if source.contains("# cue note") {
        "# cue note"
    } else {
        "# audio note"
    }));
    loaded.save(root).expect("unchanged save");
    assert_eq!(
        fs::read_to_string(root.join("config.toml")).expect("unchanged text"),
        saved
    );
}

#[rstest::rstest]
#[case("[host]\n[host.plugin_types]\n'parser.dll' = 'Fixture.Parser'\n")]
#[case("host = { plugin_types = { 'parser.dll' = 'Fixture.Parser' } }\n")]
fn removed_settings_do_not_return_after_restart(#[case] source: &str) {
    let temporary = tempfile::tempdir().expect("config fixture");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8");
    fs::write(root.join("config.toml"), source).expect("fixture");
    let mut file = ConfigFile::load(root).expect("load");
    file.settings.host.plugin_types.clear();
    file.settings.graphs.solo_categories = Some(vec!["Damage".into()]);
    file.settings
        .text_profiles
        .insert("temporary".into(), Default::default());
    let mut file = file.save(root).expect("add settings");
    file.settings.graphs.solo_categories = None;
    file.settings.text_profiles.clear();
    file.save(root).expect("remove settings");
    let loaded = ConfigFile::load(root).expect("reload");
    assert!(loaded.settings.host.plugin_types.is_empty());
    assert!(loaded.settings.graphs.solo_categories.is_none());
    assert!(loaded.settings.text_profiles.is_empty());
}

#[rstest::rstest]
#[case("[host]\ncustom = 'keep'\n[host.plugin_types]\n")]
#[case("host = { custom = 'keep', plugin_types = {} } # host note\n")]
#[case("host.custom = 'keep'\nhost.plugin_types = {}\n")]
#[case("host = { custom = 'keep', plugin_types.'other.dll' = 'Other.Parser' } # host note\n")]
fn nested_settings_roundtrip_without_losing_extensions(#[case] source: &str) {
    let temporary = tempfile::tempdir().expect("config fixture");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8");
    fs::write(root.join("config.toml"), source).expect("fixture");
    let mut file = ConfigFile::load(root).expect("load");
    file.settings
        .host
        .plugin_types
        .insert("parser.dll".into(), "Fixture.Parser".into());
    let expected = toml_edit::ser::to_string(&file.settings).expect("serialize model");
    file.save(root).expect("save");
    let saved = fs::read_to_string(root.join("config.toml")).expect("saved text");
    let document: DocumentMut = saved.parse().expect("valid TOML");
    assert_eq!(
        document
            .get("host")
            .and_then(|host| host.get("custom"))
            .and_then(Item::as_str),
        Some("keep")
    );
    if source.contains("# host note") {
        assert!(saved.contains("# host note"));
    }
    let loaded = ConfigFile::load(root).expect("reload");
    assert_eq!(
        toml_edit::ser::to_string(&loaded.settings).expect("serialize reloaded model"),
        expected
    );
    loaded.save(root).expect("unchanged save");
    assert_eq!(
        fs::read_to_string(root.join("config.toml")).expect("unchanged text"),
        saved
    );
}

#[test]
fn graph_settings_roundtrip_defaults_validation_and_failed_save() {
    let temp = tempfile::tempdir().expect("temporary configuration");
    let root = camino::Utf8Path::from_path(temp.path()).expect("UTF-8 path");
    let old = "schema_version = 1\n# preserved\n[ui]\nlanguage = \"ko-KR\"\nunknown = \"keep\"\n";
    std::fs::write(root.join("config.toml"), old).expect("synthetic config");
    let mut file = super::ConfigFile::load(root).expect("old schema");
    assert_eq!(file.settings.graphs, super::GraphSettings::default());
    assert!(file.settings.graphs.use_suffix && file.settings.graphs.suffix_decimals);
    file.settings.graphs.levels = [true, true, false, false];
    file.settings.graphs.simple_column = "key".into();
    file.settings.graphs.solo_categories = Some(vec!["Damage".into()]);
    file.save(root).expect("save graph settings");
    let saved = std::fs::read_to_string(root.join("config.toml")).expect("read saved");
    assert!(saved.contains("# preserved") && saved.contains("unknown = \"keep\""));
    let mut changed = super::ConfigFile::load(root).expect("first restart");
    changed.settings.graphs.levels = [false, true, false, false];
    changed.settings.graphs.use_suffix = false;
    changed.settings.graphs.suffix_decimals = false;
    changed.settings.graphs.bar_colors = ["#123456".into(), "#abcdef".into(), "#abcdef".into()];
    changed.settings.graphs.surface_colors.background = "#112233".into();
    changed.settings.graphs.surface_colors.totals = "#fedcba".into();
    changed.save(root).expect("save changed levels");
    let loaded = super::ConfigFile::load(root).expect("restart");
    assert_eq!(loaded.settings.graphs.levels, [false, true, false, false]);
    assert!(!loaded.settings.graphs.use_suffix && !loaded.settings.graphs.suffix_decimals);
    assert_eq!(loaded.settings.graphs.bar_colors[0], "#123456");
    assert_eq!(loaded.settings.graphs.surface_colors.background, "#112233");
    assert_eq!(loaded.settings.graphs.surface_colors.totals, "#fedcba");
    let saved = std::fs::read_to_string(root.join("config.toml")).expect("changed config");
    assert_eq!(
        loaded.settings.graphs.solo_categories,
        Some(vec!["Damage".into()])
    );
    assert_eq!(loaded.settings.graphs.simple_column, "key");
    let mut bad_color = loaded.clone();
    bad_color.settings.graphs.bar_colors = [
        "red;display:none".into(),
        "#abcdef".into(),
        "#123456".into(),
    ];
    assert!(bad_color.save(root).is_err());
    assert_eq!(
        std::fs::read_to_string(root.join("config.toml")).expect("unchanged colors"),
        saved
    );
    let mut bad_surface = loaded.clone();
    bad_surface.settings.graphs.surface_colors.grid = "red;display:none".into();
    assert!(bad_surface.save(root).is_err());
    assert_eq!(
        std::fs::read_to_string(root.join("config.toml")).expect("unchanged surface"),
        saved
    );
    let mut bad = loaded.clone();
    bad.settings.graphs.window = 0;
    assert!(bad.save(root).is_err());
    assert_eq!(
        std::fs::read_to_string(root.join("config.toml")).expect("unchanged"),
        saved
    );
    assert!(loaded.save(&root.join("missing")).is_err());
    assert_eq!(
        std::fs::read_to_string(root.join("config.toml")).expect("unchanged after failure"),
        saved
    );
    std::fs::write(root.join("config.toml"), "[graphs]\nwindow = [broken")
        .expect("corrupt fixture");
    assert!(super::ConfigFile::load(root).is_err());
}

#[test]
fn edits_keep_comments_unknown_values_and_executable_relative_paths() {
    let temp = tempfile::tempdir().expect("temporary app");
    let root = Utf8Path::from_path(temp.path()).expect("UTF-8 fixture");
    let source =
        "# user configuration\nschema_version = 1\n[ui]\ntheme = 'mocha' # evening\ncustom = 42\n";
    fs::write(root.join("config.toml"), source).expect("write fixture");
    let mut file = ConfigFile::load(root).expect("load settings");
    file.settings.ui.theme = "latte".into();
    file.save(root).expect("save settings");
    let saved = fs::read_to_string(root.join("config.toml")).expect("read settings");
    assert!(saved.contains("# user configuration"));
    assert!(saved.contains("# evening"));
    assert!(saved.contains("custom = 42"));
    assert_eq!(
        ConfigFile::load(root).expect("reload").settings.ui.theme,
        "latte"
    );
    assert_eq!(
        resolve(root, "plugins/parser.dll"),
        root.join("plugins/parser.dll")
    );
}

#[test]
fn enablement_survives_restart_without_reordering_plugin_dependencies() {
    let temp = tempfile::tempdir().expect("app");
    let root = Utf8Path::from_path(temp.path()).expect("UTF-8 root");
    let source = "schema_version = 1\n[host]\nplugins = ['first', 'middle', 'last']\ndisabled_plugins = [\n  'last', # preserve this order and note\n  'first',\n]\n";
    fs::write(root.join("config.toml"), source).expect("config");
    let file = ConfigFile::load(root).expect("load enabled set");
    assert_eq!(
        file.settings
            .host
            .enabled_plugins()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["middle"]
    );
    let mut file = file.save(root).expect("save unchanged enablement");
    assert!(
        fs::read_to_string(root.join("config.toml"))
            .expect("saved")
            .contains("'last', # preserve this order and note")
    );
    file.settings.host.disabled_plugins.clear();
    file.save(root).expect("enable again");
    let reloaded = ConfigFile::load(root).expect("restart");
    assert_eq!(
        reloaded
            .settings
            .host
            .enabled_plugins()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["first", "middle", "last"]
    );
}

#[test]
fn timer_cues_migrate_silently_and_new_configs_use_act_designer_default() {
    let temp = tempfile::tempdir().expect("temporary settings");
    let root = Utf8Path::from_path(temp.path()).expect("UTF8");
    let partial =
        ConfigFile::parse("[audio.timer_warning]\npath = 'unused.wav'\n").expect("partial cue");
    assert_eq!(partial.settings.audio.settings().timer_warning, "none");
    let fresh = ConfigFile::load(root).expect("new config");
    assert_eq!(fresh.settings.audio.settings().timer_warning, "beep");
    std::fs::write(root.join("config.toml"), "[audio]\nsound_volume = 61\n").expect("old config");
    let mut old = ConfigFile::load(root).expect("migrate");
    assert_eq!(old.settings.audio.settings().timer_warning, "none");
    old.settings.audio.timer_start.mode = "tts".into();
    old.settings.audio.timer_warning.mode = "beep".into();
    old.settings.audio.timer_expire.mode = "wav".into();
    old.settings.audio.timer_expire.path = "synthetic.wav".into();
    old.save(root).expect("save preferences");
    let mut restored = ConfigFile::load(root).expect("restart");
    let cues = restored.settings.audio.settings();
    assert_eq!(
        (
            &*cues.timer_start,
            &*cues.timer_warning,
            &*cues.timer_expire
        ),
        ("tts", "beep", "synthetic.wav")
    );
    let before = std::fs::read(root.join("config.toml")).expect("stored");
    restored.settings.audio.timer_expire.path = "beep".into();
    restored.save(root).err().expect("ambiguous path rejected");
    assert_eq!(
        std::fs::read(root.join("config.toml")).expect("preserved"),
        before
    );
    let mut oversized = ConfigFile::load(root).expect("original");
    oversized.settings.audio.timer_start.mode = "wav".into();
    oversized.settings.audio.timer_start.path = "a".repeat(3000);
    oversized.settings.audio.timer_warning.mode = "wav".into();
    oversized.settings.audio.timer_warning.path = "b".repeat(3000);
    oversized.save(root).err().expect("combined request bound");
    assert_eq!(
        std::fs::read(root.join("config.toml")).expect("preserved"),
        before
    );
    let mut failed = ConfigFile::load(root).expect("original");
    failed.settings.audio.timer_warning.mode = "none".into();
    failed
        .save(&root.join("missing"))
        .err()
        .expect("write failure");
    assert_eq!(
        ConfigFile::load(root)
            .expect("unchanged")
            .settings
            .audio
            .timer_warning
            .mode,
        "beep"
    );
}

#[test]
fn audio_volumes_migrate_validate_and_survive_restart() {
    let temp = tempfile::tempdir().expect("app");
    let root = Utf8Path::from_path(temp.path()).expect("UTF8");
    let mut file = ConfigFile::parse("[ui]\ntheme = 'mocha'\n").expect("old config");
    assert_eq!(
        (
            file.settings.audio.sound_volume,
            file.settings.audio.tts_volume
        ),
        (100, 100)
    );
    file.settings.audio.sound_volume = 61;
    file.settings.audio.tts_volume = 37;
    file.settings.audio.misc_mode = "tts".into();
    file.settings.audio.misc_text = "fixture".into();
    file.save(root).expect("save");
    let mut saved = ConfigFile::load(root).expect("restart");
    let settings = saved.settings.audio.settings();
    assert_eq!((settings.sound_volume, settings.tts_volume), (61, 37));
    assert_eq!(
        (&*settings.misc_mode, &*settings.misc_text),
        ("tts", "fixture")
    );
    saved.settings.audio.sound_volume = 101;
    saved.save(root).err().expect("range");
    assert_eq!(
        ConfigFile::load(root)
            .expect("preserved")
            .settings
            .audio
            .sound_volume,
        61
    );
    assert!(ConfigFile::parse("[audio]\ntts_volume = 'bad'\n").is_err());
}

#[test]
fn import_extension_default_migrates_and_survives_restart() {
    let temp = tempfile::tempdir().expect("app");
    let root = Utf8Path::from_path(temp.path()).expect("UTF-8");
    let mut file = ConfigFile::parse("[ui]\ntheme = 'mocha'\n").expect("old config");
    assert!(!file.settings.ui.import_extend_default);
    assert!(file.settings.ui.export_include_logs_default);
    assert!(!file.settings.ui.archive_trigger_results_default);
    file.settings.ui.import_extend_default = true;
    file.settings.ui.export_include_logs_default = false;
    file.settings.ui.archive_trigger_results_default = true;
    file.save(root).expect("save");
    assert!(
        ConfigFile::load(root)
            .expect("restart")
            .settings
            .ui
            .import_extend_default
    );
    assert!(
        !ConfigFile::load(root)
            .expect("restart")
            .settings
            .ui
            .export_include_logs_default
    );
    assert!(
        ConfigFile::load(root)
            .expect("restart")
            .settings
            .ui
            .archive_trigger_results_default
    );
    assert!(ConfigFile::parse("[ui]\narchive_trigger_results_default = 'yes'\n").is_err());
    let invalid_root = root.join("not-a-directory");
    std::fs::write(&invalid_root, "synthetic").expect("file instead of directory");
    let mut unsaved = ConfigFile::load(root).expect("saved defaults");
    unsaved.settings.ui.import_extend_default = false;
    assert!(unsaved.save(&invalid_root).is_err());
    assert!(ConfigFile::parse("[ui]\nimport_extend_default = 'yes'\n").is_err());
    assert!(
        ConfigFile::load(root)
            .expect("unchanged")
            .settings
            .ui
            .import_extend_default
    );
}

#[test]
fn main_table_refresh_defaults_and_limits_survive_restart() {
    let temp = tempfile::tempdir().expect("app");
    let root = Utf8Path::from_path(temp.path()).expect("UTF-8");
    let mut file = ConfigFile::load(root).expect("defaults");
    assert_eq!(file.settings.ui.main_table_refresh_seconds, 5);
    assert!(file.settings.ui.main_table_reverse);
    file.settings.ui.main_table_reverse = false;
    file.settings.ui.main_table_refresh_seconds = 0;
    file = file.save(root).expect("save disabled");
    assert_eq!(
        ConfigFile::load(root)
            .expect("restart")
            .settings
            .ui
            .main_table_refresh_seconds,
        0
    );
    assert!(
        !ConfigFile::load(root)
            .expect("reverse restart")
            .settings
            .ui
            .main_table_reverse
    );
    file.settings.ui.main_table_refresh_seconds = 60;
    file = file.save(root).expect("save max");
    file.settings.ui.main_table_refresh_seconds = 61;
    file.settings.validate().expect_err("invalid");
    assert!(file.save(root).is_err());
    assert_eq!(
        ConfigFile::load(root)
            .expect("preserved")
            .settings
            .ui
            .main_table_refresh_seconds,
        60
    );
}

#[test]
fn zero_damage_hit_default_and_persistence() {
    let temp = tempfile::tempdir().expect("app");
    let root = Utf8Path::from_path(temp.path()).expect("UTF-8");
    let mut file = ConfigFile::load(root).expect("defaults");
    assert!(file.settings.host.count_zero_damage_as_hit);
    file.settings.host.count_zero_damage_as_hit = false;
    file.save(root).expect("save");
    assert!(
        !ConfigFile::load(root)
            .expect("restart")
            .settings
            .host
            .count_zero_damage_as_hit
    );
    assert!(ConfigFile::parse("[host]\ncount_zero_damage_as_hit = 'bad'\n").is_err());
}

#[test]
fn selective_options_default_persist_and_reject_invalid_names() {
    let temp = tempfile::tempdir().expect("app");
    let root = Utf8Path::from_path(temp.path()).expect("UTF-8");
    let mut file = ConfigFile::load(root).expect("defaults");
    assert_eq!(
        file.settings.host.selective_mode,
        aftercast::SelectiveMode::None
    );
    file.settings.host.selective_mode = aftercast::SelectiveMode::EncountersStrict;
    file.settings.host.selective_names = "한글\nSource\n".into();
    file.save(root).expect("save");
    let mut file = ConfigFile::load(root).expect("restart");
    assert_eq!(
        file.settings.host.selective_mode,
        aftercast::SelectiveMode::EncountersStrict
    );
    assert_eq!(
        file.settings.host.selective_options().selected,
        ["한글", "Source"]
    );
    file.settings.host.selective_names = "   ".into();
    assert!(file.save(root).is_err());
    assert_eq!(
        ConfigFile::load(root)
            .expect("previous")
            .settings
            .host
            .selective_names,
        "한글\nSource\n"
    );
}

#[test]
fn main_table_commas_persist_across_restart() {
    let temp = tempfile::tempdir().expect("app");
    let root = Utf8Path::from_path(temp.path()).expect("UTF-8");
    let mut file = ConfigFile::load(root).expect("defaults");
    assert!(file.settings.host.main_table_commas);
    file.settings.host.main_table_commas = false;
    file.save(root).expect("save");
    assert!(
        !ConfigFile::load(root)
            .expect("restart")
            .settings
            .host
            .main_table_commas
    );
}

#[test]
fn silence_cut_options_persist_and_reject_invalid_limits() {
    let temp = tempfile::tempdir().expect("app");
    let root = Utf8Path::from_path(temp.path()).expect("UTF-8");
    let mut file = ConfigFile::load(root).expect("defaults");
    assert!(!file.settings.host.silence_cut_enabled);
    assert_eq!(file.settings.host.silence_cut_seconds, 6);
    file.settings.host.silence_cut_enabled = true;
    file.settings.host.silence_cut_seconds = 12;
    file.save(root).expect("save");
    let mut file = ConfigFile::load(root).expect("restart");
    assert!(file.settings.host.silence_cut_enabled);
    assert_eq!(file.settings.host.silence_cut_seconds, 12);
    for invalid in [0, 61] {
        file.settings.host.silence_cut_seconds = invalid;
        file.settings.validate().expect_err("invalid limit");
    }
    assert!(file.save(root).is_err());
    assert_eq!(
        ConfigFile::load(root)
            .expect("unchanged")
            .settings
            .host
            .silence_cut_seconds,
        12
    );
}

#[test]
fn idle_end_options_validate_and_survive_restart() {
    let temp = tempfile::tempdir().expect("app");
    let root = Utf8Path::from_path(temp.path()).expect("UTF8");
    let mut file = ConfigFile::load(root).expect("defaults");
    assert!(file.settings.host.idle_end_enabled);
    assert_eq!(file.settings.host.idle_end_seconds, 6);
    assert!(file.settings.host.idle_timer_enabled);
    file.settings.host.idle_timer_enabled = false;
    file.settings.host.idle_end_enabled = false;
    file.settings.host.idle_end_seconds = 30;
    file.save(root).expect("save");
    let mut reloaded = ConfigFile::load(root).expect("restart");
    assert!(!reloaded.settings.host.idle_timer_enabled);
    assert!(!reloaded.settings.host.idle_end_enabled);
    assert_eq!(reloaded.settings.host.idle_end_seconds, 30);
    for invalid in [0, 1, 601] {
        reloaded.settings.host.idle_end_seconds = invalid;
        assert!(reloaded.settings.validate().is_err());
    }
    assert!(reloaded.save(root).is_err());
    assert_eq!(
        ConfigFile::load(root)
            .expect("unchanged file")
            .settings
            .host
            .idle_end_seconds,
        30
    );
}

#[test]
fn recording_options_keep_comments_and_defaults_across_restart() {
    let temp = tempfile::tempdir().expect("app");
    let root = Utf8Path::from_path(temp.path()).expect("UTF-8 root");
    let source = "schema_version = 1\n[host]\nrecord_encounter_logs = true # retain annotation\ncustom = 42\n";
    fs::write(root.join("config.toml"), source).expect("config");
    let mut file = ConfigFile::load(root).expect("load defaults");
    assert!(file.settings.host.record_encounter_logs);
    assert!(!file.settings.host.original_logline_view_logs);
    file.settings.host.record_encounter_logs = false;
    file.settings.host.original_logline_view_logs = true;
    file.save(root).expect("save recording policy");
    let saved = fs::read_to_string(root.join("config.toml")).expect("saved text");
    assert!(saved.contains("record_encounter_logs = false # retain annotation"));
    assert!(saved.contains("custom = 42"));
    let reloaded = ConfigFile::load(root).expect("reload");
    assert!(!reloaded.settings.host.record_encounter_logs);
    assert!(reloaded.settings.host.original_logline_view_logs);
    reloaded.save(root).expect("unchanged save");
    assert_eq!(
        fs::read_to_string(root.join("config.toml")).expect("unchanged"),
        saved
    );
}

#[rstest::rstest]
#[case("host = { plugins = [] }\n")]
#[case("[host]\nplugins = []\n")]
fn adding_type_overrides_round_trips_table_forms(#[case] source: &str) {
    let temp = tempfile::tempdir().expect("temporary app");
    let root = Utf8Path::from_path(temp.path()).expect("UTF-8 root");
    fs::write(root.join("config.toml"), source).expect("config fixture");
    let mut file = ConfigFile::load(root).expect("load valid table");
    file.settings
        .host
        .plugin_types
        .insert("plugins/parser.dll".into(), "Fixture.Parser".into());
    file.save(root).expect("save override");
    assert_eq!(
        ConfigFile::load(root)
            .expect("reload override")
            .settings
            .host
            .plugin_types
            .get("plugins/parser.dll")
            .map(String::as_str),
        Some("Fixture.Parser")
    );
}

#[test]
fn notification_blink_defaults_and_annotations_survive_restart() {
    let temp = tempfile::tempdir().expect("app");
    let root = Utf8Path::from_path(temp.path()).expect("UTF8 root");
    assert!(
        ConfigFile::load(root)
            .expect("defaults")
            .settings
            .ui
            .notification_blink
    );
    fs::write(
        root.join("config.toml"),
        "schema_version = 1\n[ui]\nnotification_blink = true # badge preference\n",
    )
    .expect("fixture");
    let mut file = ConfigFile::load(root).expect("config");
    file.settings.ui.notification_blink = false;
    file.save(root).expect("save");
    assert!(
        !ConfigFile::load(root)
            .expect("restart")
            .settings
            .ui
            .notification_blink
    );
    assert!(
        fs::read_to_string(root.join("config.toml"))
            .expect("saved")
            .contains("# badge preference")
    );
}

#[rstest::rstest]
#[case("schema_version = 99")]
#[case("[ui]\nlanguage = 'unknown'")]
#[case("[ui]\ntheme = 'unknown'")]
#[case("[ui]\ntheme = [")]
#[case("[ui]\nnotification_blink = 'invalid'")]
#[case("[host]\nplugins = ['same.dll', 'same.dll']")]
fn rejects_invalid_configuration_without_rewriting(#[case] source: &str) {
    let temp = tempfile::tempdir().expect("temporary app");
    let root = Utf8Path::from_path(temp.path()).expect("UTF-8 fixture");
    fs::write(root.join("config.toml"), source).expect("fixture");
    assert!(ConfigFile::load(root).is_err());
    assert_eq!(
        fs::read_to_string(root.join("config.toml")).expect("original"),
        source
    );
}

#[test]
fn profiles_validate_migrate_and_preserve_failed_save() {
    let temporary = tempfile::tempdir().expect("temp");
    let root = Utf8Path::from_path(temporary.path()).expect("UTF8");
    let mut file = ConfigFile::load(root).expect("old config");
    assert!(file.settings.text_profiles.is_empty());
    file.settings.text_profiles.insert(
        "첫 프로필".into(),
        aftercast::TextReportOptions {
            player_format: "{name:5}".into(),
            sorting: "plugin-key".into(),
            ..Default::default()
        },
    );
    file.save(root).expect("save");
    let mut reopened = ConfigFile::load(root).expect("reload");
    assert_eq!(
        reopened.settings.text_profiles["첫 프로필"].sorting,
        "plugin-key"
    );
    reopened
        .settings
        .text_profiles
        .insert("".into(), Default::default());
    assert!(reopened.save(root).is_err());
    assert_eq!(
        ConfigFile::load(root)
            .expect("preserved")
            .settings
            .text_profiles
            .len(),
        1
    );
}
#[test]
fn mini_parse_preferences_roundtrip_and_validate() {
    let temp = tempfile::tempdir().expect("temp");
    let root = Utf8Path::from_path(temp.path()).expect("UTF8");
    let mut file = ConfigFile::load(root).expect("old config");
    assert_eq!(file.settings.mini_parse.interval_seconds, 5);
    assert!(file.settings.mini_parse.profile.is_empty());
    assert!(!file.settings.mini_parse.graph);
    file.settings.mini_parse.graph = true;
    file.settings.mini_parse.interval_seconds = 12;
    file.settings.mini_parse.profile = "synthetic".into();
    file.save(root).expect("save");
    let mut loaded = ConfigFile::load(root).expect("reload");
    assert_eq!(loaded.settings.mini_parse.interval_seconds, 12);
    assert!(loaded.settings.mini_parse.graph);
    assert_eq!(loaded.settings.mini_parse.profile, "synthetic");
    loaded.settings.mini_parse.interval_seconds = 0;
    assert!(loaded.save(root).is_err());
    assert_eq!(
        ConfigFile::load(root)
            .expect("prior")
            .settings
            .mini_parse
            .interval_seconds,
        12
    );
}
#[test]
fn appearance_migrates_roundtrips_and_rejects_invalid_values() {
    let temp = tempfile::tempdir().expect("temp");
    let root = Utf8Path::from_path(temp.path()).expect("UTF8");
    let mut file = ConfigFile::load(root).expect("old config");
    assert_eq!(file.settings.appearance, AppearancePreferences::default());
    let migrated =
        ConfigFile::parse("[appearance]\ngrid_font = 'serif'\n").expect("prior appearance");
    assert_eq!(migrated.settings.appearance.ally_foreground, "#228b22");
    assert_eq!(migrated.settings.appearance.self_background, "#c8ffc8");
    file.settings.appearance.invert_plugin_colors = true;
    file.settings.appearance.plugin_luminosity = 25;
    file.settings.appearance.plugin_saturation = -75;
    file.settings.appearance.mini_font = "monospace".into();
    file.settings.appearance.mini_size = 16;
    file.settings.appearance.mini_foreground = "#Aa12F0".into();
    file.settings.appearance.grid_background = "#123456".into();
    file.settings.appearance.ally_foreground = "#987654".into();
    file.settings.appearance.self_background = "#abc123".into();
    file.save(root).expect("save");
    let mut loaded = ConfigFile::load(root).expect("restart");
    assert!(
        loaded
            .settings
            .appearance
            .mini_style()
            .contains("font-size:16px;")
    );
    assert!(
        loaded
            .settings
            .appearance
            .mini_style()
            .contains("color:#Aa12F0;")
    );
    assert!(loaded.settings.appearance.invert_plugin_colors);
    assert_eq!(loaded.settings.appearance.plugin_luminosity, 25);
    assert_eq!(loaded.settings.appearance.plugin_saturation, -75);
    assert_eq!(
        loaded.settings.appearance.color_scalars(),
        aftercast::ColorScalars {
            invert: true,
            luminosity: 25,
            saturation: -75
        }
    );
    assert_eq!(loaded.settings.appearance.grid_background, "#123456");
    assert_eq!(loaded.settings.appearance.ally_foreground, "#987654");
    assert_eq!(loaded.settings.appearance.self_background, "#abc123");
    loaded.settings.appearance.mini_foreground = "#fff;display:none".into();
    assert!(!loaded.settings.appearance.mini_style().contains("display"));
    assert!(loaded.save(root).is_err());
    assert_eq!(
        ConfigFile::load(root)
            .expect("preserved")
            .settings
            .appearance
            .mini_foreground,
        "#Aa12F0"
    );
    for source in [
        "[appearance]\nmini_font = 'evil; color:red'",
        "[appearance]\nmini_size = 7",
        "[appearance]\ngrid_background = '#123'",
        "[appearance]\nplugin_luminosity = 101",
        "[appearance]\nplugin_saturation = -101",
    ] {
        assert!(ConfigFile::parse(source).is_err());
    }
}
