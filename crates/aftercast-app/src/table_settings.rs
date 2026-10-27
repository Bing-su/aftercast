//! Durable plugin-column keys, separate from transient catalog revisions and row scopes.
mod runtime;
use aftercast::TableLevel;
pub use runtime::{Reconciled, level_key};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
pub const LEVELS: [(&str, TableLevel); 5] = [
    ("encounter", TableLevel::Encounter),
    ("combatant", TableLevel::Combatant),
    ("category", TableLevel::Category),
    ("attack", TableLevel::Attack),
    ("action", TableLevel::Action),
];
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    /// All known keys in display order, including hidden columns. Labels never identify a preference.
    pub order: Vec<String>,
    pub visible: Vec<String>,
    pub primary: Option<String>,
    pub secondary: Option<String>,
    pub descending: bool,
}
impl Preferences {
    fn validate_structure(&self) -> Result<(), &'static str> {
        let keys = self.order.iter().collect::<HashSet<_>>();
        if self.order.len() > 256
            || keys.len() != self.order.len()
            || self
                .order
                .iter()
                .any(|key| key.is_empty() || key.len() > 1024 || key.contains('\0'))
        {
            return Err(
                "Column order must contain at most 256 distinct, nonempty keys of at most 1024 UTF-8 bytes",
            );
        }
        if self.selected().len() != self.visible.len() {
            return Err("Visible column keys must be distinct members of the column order");
        }
        if self.primary.as_ref().is_some_and(|key| !keys.contains(key))
            || self
                .secondary
                .as_ref()
                .is_some_and(|key| self.primary.is_none() || !keys.contains(key))
        {
            return Err(
                "Sort keys must exist in the column order; secondary sorting requires a primary key",
            );
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        self.validate_structure()?;
        if self.visible.len() > 32 {
            return Err("At most 32 visible plugin columns are supported");
        }
        Ok(())
    }
    pub fn selected(&self) -> Vec<String> {
        let visible = self.visible.iter().collect::<HashSet<_>>();
        self.order
            .iter()
            .filter(|key| visible.contains(key))
            .cloned()
            .collect()
    }
}
#[cfg(test)]
mod tests;
