use std::collections::BTreeMap;
use std::fs;
use std::io::Write;

use camino::{Utf8Path, Utf8PathBuf};
use miette::{Context, IntoDiagnostic, Result, miette};
use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, value};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub ui: UiSettings,
    pub host: HostSettings,
    pub tables: BTreeMap<String, crate::table_settings::Preferences>,
    pub graphs: GraphSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            ui: UiSettings::default(),
            host: HostSettings::default(),
            tables: BTreeMap::new(),
            graphs: GraphSettings::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct GraphSettings {
    pub levels: [bool; 4],
    pub window: u32,
    pub rolling: bool,
    pub simple: bool,
    pub simple_column: String,
    pub only_allies: bool,
    pub totals: bool,
    pub distribution: String,
    pub action_sort: String,
    pub all_label: String,
    pub ward_prefix: String,
    pub solo_categories: Option<Vec<String>>,
}
impl Default for GraphSettings {
    fn default() -> Self {
        Self {
            levels: [true; 4],
            window: 10,
            rolling: true,
            simple: true,
            simple_column: String::new(),
            only_allies: true,
            totals: true,
            distribution: String::new(),
            action_sort: String::new(),
            all_label: "All".into(),
            ward_prefix: "Warded/".into(),
            solo_categories: None,
        }
    }
}
impl GraphSettings {
    pub(crate) fn validate(&self) -> Result<()> {
        if !(1..=120).contains(&self.window)
            || [
                &self.simple_column,
                &self.distribution,
                &self.action_sort,
                &self.all_label,
                &self.ward_prefix,
            ]
            .iter()
            .any(|v| v.len() > 1024 || v.contains('\0'))
            || self.all_label.is_empty()
            || (self.levels[2] && self.distribution == "Resist" && self.ward_prefix.is_empty())
            || self.solo_categories.as_ref().is_some_and(|keys| {
                keys.len() > 256
                    || keys
                        .iter()
                        .any(|key| key.is_empty() || key.len() > 1024 || key.contains('\0'))
                    || keys.iter().collect::<std::collections::HashSet<_>>().len() != keys.len()
            })
        {
            return Err(miette!("Invalid report graph settings"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct UiSettings {
    pub language: String,
    pub theme: String,
    pub notification_blink: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            language: "ko-KR".into(),
            theme: "mocha".into(),
            notification_blink: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct HostSettings {
    pub dotnet_root: String,
    pub plugins: Vec<String>,
    pub plugin_types: BTreeMap<String, String>,
    pub disabled_plugins: Vec<String>,
    pub record_encounter_logs: bool,
    pub original_logline_view_logs: bool,
    pub silence_cut_enabled: bool,
    pub silence_cut_seconds: u8,
    pub idle_end_enabled: bool,
    pub idle_timer_enabled: bool,
    pub idle_end_seconds: u16,
}

impl Default for HostSettings {
    fn default() -> Self {
        Self {
            dotnet_root: "dotnet".into(),
            plugins: Vec::new(),
            plugin_types: BTreeMap::new(),
            disabled_plugins: Vec::new(),
            record_encounter_logs: true,
            original_logline_view_logs: false,
            silence_cut_enabled: false,
            silence_cut_seconds: 6,
            idle_end_enabled: true,
            idle_timer_enabled: true,
            idle_end_seconds: 6,
        }
    }
}

impl HostSettings {
    pub fn enabled_plugins(&self) -> impl Iterator<Item = &String> {
        self.plugins
            .iter()
            .filter(|path| !self.disabled_plugins.contains(*path))
    }
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1 {
            return Err(miette!(
                "Unsupported config schema_version: {}",
                self.schema_version
            ));
        }
        if !matches!(self.ui.language.as_str(), "ko-KR" | "en-US") {
            return Err(miette!("Unsupported UI language: {}", self.ui.language));
        }
        if !matches!(
            self.ui.theme.as_str(),
            "mocha" | "macchiato" | "frappe" | "latte"
        ) {
            return Err(miette!("Unsupported UI theme: {}", self.ui.theme));
        }
        if self.host.dotnet_root.trim().is_empty()
            || self.host.plugins.iter().any(|p| p.trim().is_empty())
            || self
                .host
                .disabled_plugins
                .iter()
                .any(|p| p.trim().is_empty())
        {
            return Err(miette!("Runtime and plugin paths must not be empty"));
        }
        let mut paths = std::collections::HashSet::new();
        if self.host.plugins.iter().any(|path| !paths.insert(path)) {
            return Err(miette!(
                "Duplicate configured plugin path; each entry must have a unique path"
            ));
        }
        if !(1..=60).contains(&self.host.silence_cut_seconds) {
            return Err(miette!("Silence cut threshold must be 1–60 seconds"));
        }
        if !(2..=600).contains(&self.host.idle_end_seconds) {
            return Err(miette!("Idle end threshold must be 2–600 seconds"));
        }
        self.graphs.validate()?;
        for (level, preferences) in &self.tables {
            if !crate::table_settings::LEVELS
                .iter()
                .any(|(key, _)| *key == level)
            {
                return Err(miette!("Unknown plugin table level: {level}"));
            }
            preferences
                .validate()
                .map_err(|error| miette!("tables.{level}: {error}"))?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct ConfigFile {
    pub settings: Settings,
    document: DocumentMut,
}

impl ConfigFile {
    pub fn load(root: &Utf8Path) -> Result<Self> {
        let path = root.join("config.toml");
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => {
                return Err(error)
                    .into_diagnostic()
                    .wrap_err_with(|| format!("Read {path}"));
            }
        };
        Self::parse(&text).wrap_err_with(|| format!("Read {path}"))
    }

    fn parse(text: &str) -> Result<Self> {
        let document = text.parse::<DocumentMut>().into_diagnostic()?;
        let settings: Settings = toml_edit::de::from_str(text).into_diagnostic()?;
        settings.validate()?;
        Ok(Self { settings, document })
    }

    #[expect(
        clippy::indexing_slicing,
        reason = "TOML mutable key indexing inserts missing tables; deserialization validates existing shapes"
    )]
    pub fn save(mut self, root: &Utf8Path) -> Result<Self> {
        self.settings.validate()?;
        let settings = &self.settings;
        set(
            &mut self.document["schema_version"],
            value(i64::from(settings.schema_version)),
        );
        set(
            &mut self.document["ui"]["language"],
            value(&settings.ui.language),
        );
        set(&mut self.document["ui"]["theme"], value(&settings.ui.theme));
        set(
            &mut self.document["ui"]["notification_blink"],
            value(settings.ui.notification_blink),
        );
        set(
            &mut self.document["host"]["dotnet_root"],
            value(&settings.host.dotnet_root),
        );
        set(
            &mut self.document["host"]["record_encounter_logs"],
            value(settings.host.record_encounter_logs),
        );
        set(
            &mut self.document["host"]["original_logline_view_logs"],
            value(settings.host.original_logline_view_logs),
        );
        set(
            &mut self.document["host"]["silence_cut_enabled"],
            value(settings.host.silence_cut_enabled),
        );
        set(
            &mut self.document["host"]["silence_cut_seconds"],
            value(i64::from(settings.host.silence_cut_seconds)),
        );
        set(
            &mut self.document["host"]["idle_end_enabled"],
            value(settings.host.idle_end_enabled),
        );
        set(
            &mut self.document["host"]["idle_timer_enabled"],
            value(settings.host.idle_timer_enabled),
        );
        set(
            &mut self.document["host"]["idle_end_seconds"],
            value(i64::from(settings.host.idle_end_seconds)),
        );
        let plugins = settings
            .host
            .plugins
            .iter()
            .map(String::as_str)
            .collect::<toml_edit::Array>();
        set(&mut self.document["host"]["plugins"], value(plugins));
        set(
            &mut self.document["host"]["disabled_plugins"],
            value(
                settings
                    .host
                    .disabled_plugins
                    .iter()
                    .map(String::as_str)
                    .collect::<toml_edit::Array>(),
            ),
        );
        for (path, type_name) in &settings.host.plugin_types {
            if self.document["host"].get("plugin_types").is_none() {
                self.document["host"]["plugin_types"] =
                    toml_edit::Item::Table(toml_edit::Table::new());
            }
            set(
                &mut self.document["host"]["plugin_types"][path],
                value(type_name),
            );
        }
        save_graphs(&mut self.document, &settings.graphs);
        save_tables(&mut self.document, &settings.tables)?;
        let path = root.join("config.toml");
        let mut temporary = tempfile::NamedTempFile::new_in(root).into_diagnostic()?;
        temporary
            .write_all(self.document.to_string().as_bytes())
            .into_diagnostic()?;
        temporary.as_file().sync_all().into_diagnostic()?;
        temporary
            .persist(&path)
            .map_err(|error| error.error)
            .into_diagnostic()
            .wrap_err_with(|| format!("Save {path}"))?;
        Ok(self)
    }
}

#[expect(
    clippy::indexing_slicing,
    reason = "TOML indexing inserts graph fields"
)]
fn save_graphs(document: &mut DocumentMut, graph: &GraphSettings) {
    set(
        &mut document["graphs"]["levels"],
        value(graph.levels.iter().copied().collect::<toml_edit::Array>()),
    );
    set(
        &mut document["graphs"]["window"],
        value(i64::from(graph.window)),
    );
    set(&mut document["graphs"]["rolling"], value(graph.rolling));
    set(&mut document["graphs"]["simple"], value(graph.simple));
    set(
        &mut document["graphs"]["simple_column"],
        value(&graph.simple_column),
    );
    set(
        &mut document["graphs"]["only_allies"],
        value(graph.only_allies),
    );
    set(&mut document["graphs"]["totals"], value(graph.totals));
    set(
        &mut document["graphs"]["distribution"],
        value(&graph.distribution),
    );
    set(
        &mut document["graphs"]["action_sort"],
        value(&graph.action_sort),
    );
    set(
        &mut document["graphs"]["all_label"],
        value(&graph.all_label),
    );
    set(
        &mut document["graphs"]["ward_prefix"],
        value(&graph.ward_prefix),
    );
    if let Some(keys) = &graph.solo_categories {
        set(
            &mut document["graphs"]["solo_categories"],
            value(
                keys.iter()
                    .map(String::as_str)
                    .collect::<toml_edit::Array>(),
            ),
        );
    } else if let Some(table) = document["graphs"].as_table_like_mut() {
        table.remove("solo_categories");
    }
}

fn save_tables(
    document: &mut DocumentMut,
    preferences: &BTreeMap<String, crate::table_settings::Preferences>,
) -> Result<()> {
    use toml_edit::{Item, Table};
    if preferences.is_empty() && !document.as_table().contains_key("tables") {
        return Ok(());
    }
    let tables = document
        .as_table_mut()
        .entry("tables")
        .or_insert(Item::Table(Table::new()))
        .as_table_like_mut()
        .ok_or_else(|| miette!("tables must be a TOML table"))?;
    let removed = tables
        .iter()
        .filter(|(key, _)| !preferences.contains_key(*key))
        .map(|(key, _)| key.to_owned())
        .collect::<Vec<_>>();
    for key in removed {
        tables.remove(&key);
    }
    for (level, preferences) in preferences {
        let table = tables
            .entry(level)
            .or_insert(Item::Table(Table::new()))
            .as_table_like_mut()
            .ok_or_else(|| miette!("tables.{level} must be a TOML table"))?;
        for (key, values) in [
            ("order", &preferences.order),
            ("visible", &preferences.visible),
        ] {
            set(
                table.entry(key).or_insert(Item::None),
                value(
                    values
                        .iter()
                        .map(String::as_str)
                        .collect::<toml_edit::Array>(),
                ),
            );
        }
        set(
            table.entry("descending").or_insert(Item::None),
            value(preferences.descending),
        );
        for (key, selected) in [
            ("primary", &preferences.primary),
            ("secondary", &preferences.secondary),
        ] {
            if let Some(selected) = selected {
                set(table.entry(key).or_insert(Item::None), value(selected));
            } else {
                table.remove(key);
            }
        }
    }
    Ok(())
}

fn set(item: &mut toml_edit::Item, mut replacement: toml_edit::Item) {
    if let (Some(old), Some(new)) = (item.as_value(), replacement.as_value()) {
        let same = match (old, new) {
            (toml_edit::Value::String(old), toml_edit::Value::String(new)) => {
                old.value() == new.value()
            }
            (toml_edit::Value::Integer(old), toml_edit::Value::Integer(new)) => {
                old.value() == new.value()
            }
            (toml_edit::Value::Boolean(old), toml_edit::Value::Boolean(new)) => {
                old.value() == new.value()
            }
            (toml_edit::Value::Array(old), toml_edit::Value::Array(new)) => {
                old.len() == new.len()
                    && old
                        .iter()
                        .zip(new.iter())
                        .all(|(left, right)| match (left, right) {
                            (toml_edit::Value::String(a), toml_edit::Value::String(b)) => {
                                a.value() == b.value()
                            }
                            (toml_edit::Value::Boolean(a), toml_edit::Value::Boolean(b)) => {
                                a.value() == b.value()
                            }
                            (toml_edit::Value::Integer(a), toml_edit::Value::Integer(b)) => {
                                a.value() == b.value()
                            }
                            _ => false,
                        })
            }
            _ => false,
        };
        if same {
            return;
        }
    }
    // Keep user annotations when changing a value, e.g. `theme = "dark" # evening`.
    if let (Some(old), Some(new)) = (item.as_value(), replacement.as_value_mut()) {
        *new.decor_mut() = old.decor().clone();
    }
    *item = replacement;
}

pub fn resolve(root: &Utf8Path, path: &str) -> Utf8PathBuf {
    let path = Utf8Path::new(path);
    if path.is_absolute() {
        path.to_owned()
    } else {
        root.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_settings_roundtrip_defaults_validation_and_failed_save() {
        let temp = tempfile::tempdir().expect("temporary configuration");
        let root = camino::Utf8Path::from_path(temp.path()).expect("UTF-8 path");
        let old =
            "schema_version = 1\n# preserved\n[ui]\nlanguage = \"ko-KR\"\nunknown = \"keep\"\n";
        std::fs::write(root.join("config.toml"), old).expect("synthetic config");
        let mut file = super::ConfigFile::load(root).expect("old schema");
        assert_eq!(file.settings.graphs, super::GraphSettings::default());
        file.settings.graphs.levels = [true, true, false, false];
        file.settings.graphs.simple_column = "key".into();
        file.settings.graphs.solo_categories = Some(vec!["Damage".into()]);
        file.save(root).expect("save graph settings");
        let saved = std::fs::read_to_string(root.join("config.toml")).expect("read saved");
        assert!(saved.contains("# preserved") && saved.contains("unknown = \"keep\""));
        let mut changed = super::ConfigFile::load(root).expect("first restart");
        changed.settings.graphs.levels = [false, true, false, false];
        changed.save(root).expect("save changed levels");
        let loaded = super::ConfigFile::load(root).expect("restart");
        assert_eq!(loaded.settings.graphs.levels, [false, true, false, false]);
        let saved = std::fs::read_to_string(root.join("config.toml")).expect("changed config");
        assert_eq!(
            loaded.settings.graphs.solo_categories,
            Some(vec!["Damage".into()])
        );
        assert_eq!(loaded.settings.graphs.simple_column, "key");
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
        let source = "# user configuration\nschema_version = 1\n[ui]\ntheme = 'mocha' # evening\ncustom = 42\n";
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
}
