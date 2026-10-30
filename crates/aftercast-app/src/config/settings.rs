use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};
use miette::{IntoDiagnostic, Result, miette};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub ui: UiSettings,
    pub host: HostSettings,
    pub tables: BTreeMap<String, crate::table_settings::Preferences>,
    pub graphs: GraphSettings,
    pub audio: AudioPreferences,
    pub text_profiles: BTreeMap<String, aftercast::TextReportOptions>,
    pub mini_parse: MiniParsePreferences,
    pub appearance: AppearancePreferences,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            ui: UiSettings::default(),
            host: HostSettings::default(),
            tables: BTreeMap::new(),
            graphs: GraphSettings::default(),
            audio: AudioPreferences::default(),
            text_profiles: BTreeMap::new(),
            mini_parse: MiniParsePreferences::default(),
            appearance: AppearancePreferences::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppearancePreferences {
    pub invert_plugin_colors: bool,
    pub plugin_luminosity: i8,
    pub plugin_saturation: i8,
    pub grid_font: String,
    pub grid_size: u8,
    pub grid_foreground: String,
    pub grid_background: String,
    pub ally_foreground: String,
    pub self_background: String,
    pub mini_font: String,
    pub mini_size: u8,
    pub mini_foreground: String,
    pub mini_background: String,
}
impl Default for AppearancePreferences {
    fn default() -> Self {
        Self {
            invert_plugin_colors: false,
            plugin_luminosity: 0,
            plugin_saturation: 0,
            grid_font: String::new(),
            grid_size: 0,
            grid_foreground: String::new(),
            grid_background: String::new(),
            ally_foreground: "#228b22".into(),
            self_background: "#c8ffc8".into(),
            mini_font: String::new(),
            mini_size: 0,
            mini_foreground: String::new(),
            mini_background: String::new(),
        }
    }
}
fn valid_font(font: &str) -> bool {
    matches!(font, "" | "sans-serif" | "serif" | "monospace")
}
pub(crate) fn valid_color(color: &str) -> bool {
    color.is_empty()
        || (color.len() == 7
            && color.starts_with('#')
            && color
                .as_bytes()
                .get(1..)
                .is_some_and(|bytes| bytes.iter().all(u8::is_ascii_hexdigit)))
}
fn surface_style(font: &str, size: u8, foreground: &str, background: &str) -> String {
    let mut style = String::new();
    if valid_font(font) && !font.is_empty() {
        style.push_str(&format!("font-family:{font};"));
    }
    if (8..=32).contains(&size) {
        style.push_str(&format!("font-size:{size}px;"));
    }
    if valid_color(foreground) && !foreground.is_empty() {
        style.push_str(&format!("color:{foreground};"));
    }
    if valid_color(background) && !background.is_empty() {
        style.push_str(&format!("background-color:{background};"));
    }
    style
}
impl AppearancePreferences {
    pub(crate) fn color_controls(&self) -> aftercast::ColorControls {
        fn rgb(value: &str) -> Option<u32> {
            value
                .get(1..)
                .and_then(|digits| u32::from_str_radix(digits, 16).ok())
        }
        aftercast::ColorControls {
            ally: rgb(&self.ally_foreground),
            personal: rgb(&self.self_background),
        }
    }
    pub(crate) fn color_scalars(&self) -> aftercast::ColorScalars {
        aftercast::ColorScalars {
            invert: self.invert_plugin_colors,
            luminosity: self.plugin_luminosity,
            saturation: self.plugin_saturation,
        }
    }
    pub(crate) fn valid(&self) -> bool {
        (-100..=100).contains(&self.plugin_luminosity)
            && (-100..=100).contains(&self.plugin_saturation)
            && valid_font(&self.grid_font)
            && valid_font(&self.mini_font)
            && (self.grid_size == 0 || (8..=32).contains(&self.grid_size))
            && (self.mini_size == 0 || (8..=32).contains(&self.mini_size))
            && [
                &self.grid_foreground,
                &self.grid_background,
                &self.ally_foreground,
                &self.self_background,
                &self.mini_foreground,
                &self.mini_background,
            ]
            .into_iter()
            .all(|value| valid_color(value))
    }
    pub(crate) fn mini_style(&self) -> String {
        surface_style(
            &self.mini_font,
            self.mini_size,
            &self.mini_foreground,
            &self.mini_background,
        )
    }
    pub(crate) fn grid_style(&self) -> String {
        surface_style(&self.grid_font, self.grid_size, "", "")
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct MiniParsePreferences {
    pub graph: bool,
    pub interval_seconds: u8,
    pub profile: String,
}
impl Default for MiniParsePreferences {
    fn default() -> Self {
        Self {
            graph: false,
            interval_seconds: 5,
            profile: String::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct AudioPreferences {
    pub sound_volume: u8,
    pub tts_volume: u8,
    pub misc_mode: String,
    pub misc_text: String,
    pub timer_start: TimerCuePreferences,
    pub timer_warning: TimerCuePreferences,
    pub timer_expire: TimerCuePreferences,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct TimerCuePreferences {
    pub mode: String,
    pub path: String,
}
impl Default for TimerCuePreferences {
    fn default() -> Self {
        Self::none()
    }
}
impl TimerCuePreferences {
    pub(super) fn none() -> Self {
        Self {
            mode: "none".into(),
            path: String::new(),
        }
    }
    pub(super) fn warning_default() -> Self {
        Self {
            mode: "beep".into(),
            path: String::new(),
        }
    }
    fn valid(&self) -> bool {
        matches!(self.mode.as_str(), "none" | "beep" | "wav" | "tts")
            && self.path.len() <= 4000
            && !self.path.chars().any(char::is_control)
            && (self.mode != "wav"
                || (!self.path.trim().is_empty()
                    && !matches!(self.path.as_str(), "none" | "beep" | "tts")
                    && !self.path.starts_with("tts ")))
    }
    fn cue(&self) -> String {
        if self.mode == "wav" {
            self.path.clone()
        } else {
            self.mode.clone()
        }
    }
}
impl Default for AudioPreferences {
    fn default() -> Self {
        Self {
            sound_volume: 100,
            tts_volume: 100,
            misc_mode: "none".into(),
            misc_text: String::new(),
            timer_start: TimerCuePreferences::none(),
            timer_warning: TimerCuePreferences::warning_default(),
            timer_expire: TimerCuePreferences::none(),
        }
    }
}
impl AudioPreferences {
    pub fn settings(&self) -> aftercast::AudioSettings {
        aftercast::AudioSettings {
            sound_volume: self.sound_volume,
            tts_volume: self.tts_volume,
            misc_mode: self.misc_mode.clone(),
            misc_text: self.misc_text.clone(),
            timer_start: self.timer_start.cue(),
            timer_warning: self.timer_warning.cue(),
            timer_expire: self.timer_expire.cue(),
        }
    }
}

/// ACT encounter-bar graph surface colors. System colors use explicit cross-platform fallbacks.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct GraphSurfaceColors {
    pub background: String,
    pub legend: String,
    pub outline: String,
    pub grid: String,
    pub text: String,
    pub totals: String,
}
impl Default for GraphSurfaceColors {
    fn default() -> Self {
        Self {
            background: "#f5f5f5".into(),
            legend: "#000000".into(),
            outline: "#000000".into(),
            grid: "#a0a0a0".into(),
            text: "#ffffff".into(),
            totals: "#000000".into(),
        }
    }
}
impl GraphSurfaceColors {
    pub(crate) fn fields(&self) -> [&str; 6] {
        [
            &self.background,
            &self.legend,
            &self.outline,
            &self.grid,
            &self.text,
            &self.totals,
        ]
    }
    pub(crate) fn field_mut(&mut self, index: usize) -> Option<&mut String> {
        match index {
            0 => Some(&mut self.background),
            1 => Some(&mut self.legend),
            2 => Some(&mut self.outline),
            3 => Some(&mut self.grid),
            4 => Some(&mut self.text),
            5 => Some(&mut self.totals),
            _ => None,
        }
    }
    pub(crate) fn valid(&self) -> bool {
        self.fields()
            .iter()
            .all(|color| !color.is_empty() && valid_color(color))
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
    pub use_suffix: bool,
    pub suffix_decimals: bool,
    pub bar_colors: [String; 3],
    pub surface_colors: GraphSurfaceColors,
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
            use_suffix: true,
            suffix_decimals: true,
            bar_colors: ["#8b0000".into(), "#00008b".into(), "#9400d3".into()],
            surface_colors: GraphSurfaceColors::default(),
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
            || !self.surface_colors.valid()
            || self
                .bar_colors
                .iter()
                .any(|color| color.is_empty() || !valid_color(color))
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
    pub import_extend_default: bool,
    pub export_include_logs_default: bool,
    pub archive_trigger_results_default: bool,
    pub main_table_refresh_seconds: u8,
    pub main_table_reverse: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            language: "ko-KR".into(),
            theme: "mocha".into(),
            notification_blink: true,
            import_extend_default: false,
            export_include_logs_default: true,
            archive_trigger_results_default: false,
            main_table_refresh_seconds: 5,
            main_table_reverse: true,
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
    pub main_table_commas: bool,
    pub count_zero_damage_as_hit: bool,
    pub selective_mode: aftercast::SelectiveMode,
    pub selective_names: String,
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
            main_table_commas: true,
            count_zero_damage_as_hit: true,
            selective_mode: aftercast::SelectiveMode::None,
            selective_names: String::new(),
            silence_cut_enabled: false,
            silence_cut_seconds: 6,
            idle_end_enabled: true,
            idle_timer_enabled: true,
            idle_end_seconds: 6,
        }
    }
}

impl HostSettings {
    pub fn selective_options(&self) -> aftercast::SelectiveOptions {
        aftercast::SelectiveOptions {
            mode: self.selective_mode,
            selected: self
                .selective_names
                .lines()
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect(),
        }
    }
    pub fn enabled_plugins(&self) -> impl Iterator<Item = &String> {
        self.plugins
            .iter()
            .filter(|path| !self.disabled_plugins.contains(*path))
    }
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        if self.audio.sound_volume > 100
            || self.audio.tts_volume > 100
            || !matches!(
                self.audio.misc_mode.as_str(),
                "none" | "beep" | "wav" | "tts"
            )
            || self.audio.misc_text.len() > 4000
            || self.audio.misc_text.chars().any(char::is_control)
            || !self.audio.timer_start.valid()
            || !self.audio.timer_warning.valid()
            || !self.audio.timer_expire.valid()
        {
            return Err(miette!("Invalid audio volumes or timer cue settings"));
        }
        let cues = self.audio.settings();
        if cues.timer_start.len()
            + cues.timer_warning.len()
            + cues.timer_expire.len()
            + cues.misc_mode.len()
            + cues.misc_text.len()
            + 4
            > 4096
        {
            return Err(miette!(
                "Combined audio cue settings exceed 4096 UTF-8 bytes"
            ));
        }
        if !(1..=60).contains(&self.mini_parse.interval_seconds)
            || self.mini_parse.profile.len() > 80
            || self.mini_parse.profile.chars().any(char::is_control)
        {
            return Err(miette!("Invalid mini parse preferences"));
        }
        if !self.appearance.valid() {
            return Err(miette!("Invalid appearance preferences"));
        }
        if self.text_profiles.len() > 32
            || self
                .text_profiles
                .iter()
                .any(|(name, profile)| !valid_text_profile(name, profile))
        {
            return Err(miette!("Invalid text report profile"));
        }
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
        if self.ui.main_table_refresh_seconds > 60 {
            return Err(miette!("Main table refresh must be 0–60 seconds"));
        }
        if !(1..=60).contains(&self.host.silence_cut_seconds) {
            return Err(miette!("Silence cut threshold must be 1–60 seconds"));
        }
        if !(2..=600).contains(&self.host.idle_end_seconds) {
            return Err(miette!("Idle end threshold must be 2–600 seconds"));
        }
        self.host.selective_options().validate().into_diagnostic()?;
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

pub(crate) fn valid_text_profile(name: &str, profile: &aftercast::TextReportOptions) -> bool {
    !name.trim().is_empty()
        && name.len() <= 80
        && !name.chars().any(char::is_control)
        && profile.player_format.len() <= 16384
        && profile.allies_format.len() <= 16384
        && profile.sorting.len() <= 256
        && !profile.sorting.chars().any(char::is_control)
}

pub fn resolve(root: &Utf8Path, path: &str) -> Utf8PathBuf {
    let path = Utf8Path::new(path);
    if path.is_absolute() {
        path.to_owned()
    } else {
        root.join(path)
    }
}
