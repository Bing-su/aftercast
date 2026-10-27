use super::Preferences;
use aftercast::{TableCatalog, TableLevel, TableQuery, TableScope, TableSort};
use std::collections::{HashMap, HashSet};
pub fn level_key(level: TableLevel) -> &'static str {
    match level {
        TableLevel::Encounter => "encounter",
        TableLevel::Combatant => "combatant",
        TableLevel::Category => "category",
        TableLevel::Attack => "attack",
        TableLevel::Action => "action",
    }
}
#[derive(Debug)]
pub struct Reconciled {
    pub preferences: Preferences,
    pub removed: Vec<String>,
    pub added: Vec<String>,
    pub reset_sorts: Vec<String>,
}
impl Preferences {
    pub fn reconcile(&self, catalog: &TableCatalog) -> Result<Reconciled, &'static str> {
        self.validate_structure()?;
        let columns = catalog
            .columns
            .iter()
            .map(|column| (&column.key, column))
            .collect::<HashMap<_, _>>();
        let known = self.order.iter().collect::<HashSet<_>>();
        let removed = self
            .order
            .iter()
            .filter(|key| !columns.contains_key(key))
            .cloned()
            .collect();
        let mut order = self
            .order
            .iter()
            .filter(|key| columns.contains_key(key))
            .cloned()
            .collect::<Vec<_>>();
        let added = catalog
            .columns
            .iter()
            .filter(|column| !known.contains(&column.key))
            .map(|column| column.key.clone())
            .collect::<Vec<_>>();
        order.extend(added.iter().cloned());
        let visible = self.visible.iter().collect::<HashSet<_>>();
        let selected = order
            .iter()
            .filter(|key| {
                visible.contains(key)
                    || !known.contains(key)
                        && columns
                            .get(key)
                            .is_some_and(|column| column.default_visible)
            })
            .cloned()
            .collect();
        let mut preferences = Self {
            order,
            visible: selected,
            primary: self.primary.clone(),
            secondary: self.secondary.clone(),
            descending: self.descending,
        };
        let mut reset_sorts = Vec::new();
        if preferences
            .primary
            .as_ref()
            .is_some_and(|key| !columns.get(key).is_some_and(|column| column.can_sort))
            && let Some(key) = preferences.primary.take()
        {
            reset_sorts.push(key);
        }
        if preferences.secondary.as_ref().is_some_and(|key| {
            preferences.primary.is_none() || !columns.get(key).is_some_and(|column| column.can_sort)
        }) && let Some(key) = preferences.secondary.take()
            && !reset_sorts.contains(&key)
        {
            reset_sorts.push(key);
        }
        // Preserve >32 defaults in the draft: validation explains the limit instead of silently hiding columns.
        Ok(Reconciled {
            preferences,
            removed,
            added,
            reset_sorts,
        })
    }
    pub fn query(
        &self,
        scope: TableScope,
        catalog: &TableCatalog,
        offset: u32,
        limit: u32,
    ) -> Result<Option<TableQuery>, &'static str> {
        self.validate()?;
        if scope.level() != catalog.level
            || self
                .order
                .iter()
                .any(|key| !catalog.columns.iter().any(|column| &column.key == key))
        {
            return Err("Refresh column metadata before applying this layout");
        }
        for key in [&self.primary, &self.secondary].into_iter().flatten() {
            if !catalog
                .columns
                .iter()
                .any(|column| &column.key == key && column.can_sort)
            {
                return Err("A selected sort column no longer supports comparison");
            }
        }
        let columns = self.selected();
        if columns.is_empty() {
            return Ok(None);
        }
        let sort = self.primary.as_ref().map(|primary| TableSort {
            primary: primary.clone(),
            secondary: self.secondary.clone(),
            descending: self.descending,
        });
        Ok(Some(TableQuery {
            scope,
            revision: catalog.revision,
            columns,
            offset,
            limit,
            sort,
        }))
    }
}
