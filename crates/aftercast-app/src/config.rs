mod settings;
pub use settings::*;

use std::fs;
use std::io::Write;

use camino::Utf8Path;
use miette::{Context, IntoDiagnostic, Result};
use toml_edit::{DocumentMut, Item};

#[derive(Clone)]
pub struct ConfigFile {
    pub settings: Settings,
    document: DocumentMut,
}

impl ConfigFile {
    pub fn load(root: &Utf8Path) -> Result<Self> {
        let path = root.join("config.toml");
        let (text, existing) = match fs::read_to_string(&path) {
            Ok(text) => (text, true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (String::new(), false),
            Err(error) => {
                return Err(error)
                    .into_diagnostic()
                    .wrap_err_with(|| format!("Read {path}"));
            }
        };
        let mut file = Self::parse(&text).wrap_err_with(|| format!("Read {path}"))?;
        if !existing {
            file.settings.audio.timer_warning = TimerCuePreferences::warning_default();
        }
        Ok(file)
    }

    fn parse(text: &str) -> Result<Self> {
        let document = text.parse::<DocumentMut>().into_diagnostic()?;
        let mut settings: Settings = toml_edit::de::from_str(text).into_diagnostic()?;
        // Older aftercast configurations had no timer cues and always used silence.
        if document
            .get("audio")
            .and_then(|audio| audio.get("timer_warning"))
            .is_none()
        {
            settings.audio.timer_warning = TimerCuePreferences::none();
        }
        settings.validate()?;
        Ok(Self { settings, document })
    }

    pub fn save(mut self, root: &Utf8Path) -> Result<Self> {
        self.settings.validate()?;
        let previous: Settings =
            toml_edit::de::from_document(self.document.clone()).into_diagnostic()?;
        let before = toml_edit::ser::to_document(&previous).into_diagnostic()?;
        let after = toml_edit::ser::to_document(&self.settings).into_diagnostic()?;
        merge(
            self.document.as_item_mut(),
            Some(before.as_item()),
            after.as_item(),
        );
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

fn merge(item: &mut Item, before: Option<&Item>, after: &Item) {
    let inline = item.is_inline_table();
    let (Some(target), Some(updated)) = (item.as_table_like_mut(), after.as_table_like()) else {
        set(item, after.clone());
        return;
    };
    // Remove only keys owned by Settings, e.g. a deleted profile; keep user extensions.
    let previous = before.and_then(Item::as_table_like);
    if let Some(previous) = previous {
        for (key, _) in previous.iter() {
            if !updated.contains_key(key) {
                target.remove(key);
            }
        }
    }
    for (key, replacement) in updated.iter() {
        if let Some(existing) = target.get_mut(key) {
            merge(
                existing,
                previous.and_then(|table| table.get(key)),
                replacement,
            );
        } else if !replacement
            .as_table_like()
            .is_some_and(|table| table.is_empty())
        {
            let mut replacement = replacement.clone();
            // Inline tables require values, e.g. host = { plugin_types = { ... } }.
            if inline {
                replacement.make_value();
            }
            target.insert(key, replacement);
        }
    }
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

#[cfg(test)]
mod tests;
