use super::{AppState, Phase, tr};
use crate::{
    config::ConfigFile,
    table_settings::{Preferences, level_key},
};
use aftercast::{HostHandle, TableCatalog, TablePage, TableScope};
use dioxus::{core::spawn_forever, prelude::*};
use std::collections::BTreeMap;
mod controls;
mod render;
const PAGE: u32 = 16;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct View {
    session: u64,
    generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Refresh,
    Auto,
    Apply,
    Saved,
}
#[derive(Clone)]
struct Token {
    view: View,
    base: TableScope,
    scope: TableScope,
    mode: Mode,
    editor_version: u64,
    reverse: Option<bool>,
}
#[derive(Clone)]
struct Editor {
    draft: Preferences,
    applied: Preferences,
    version: u64,
}
#[derive(Clone)]
struct Frame {
    token: Token,
    catalog: TableCatalog,
    page: Option<TablePage>,
    offset: u32,
    missing: bool,
    notices: Vec<Notice>,
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Notice {
    kind: &'static str,
    keys: Vec<String>,
}
struct Loaded {
    layout_error: Option<String>,
    catalog: TableCatalog,
    preferences: Preferences,
    page: Option<TablePage>,
    offset: u32,
    missing: bool,
    notices: Vec<Notice>,
}
#[derive(Clone)]
struct SaveToken {
    id: u64,
    view: View,
    level: String,
    preferences: Preferences,
}
#[derive(Default)]
pub(super) struct BrowserState {
    generation: u64,
    base: Option<TableScope>,
    scope: Option<TableScope>,
    history: Vec<TableScope>,
    zone_anchor: Option<aftercast::EncounterId>,
    zone_id: Option<aftercast::ZoneId>,
    zones: Vec<aftercast::ZoneSummary>,
    zones_generation: u64,
    zones_pending: bool,
    zones_error: Option<String>,
    editors: BTreeMap<String, Editor>,
    pending: Option<Token>,
    auto_fallback: Option<View>,
    attempted: Option<Token>,
    frame: Option<Frame>,
    error: Option<String>,
    save: Option<SaveToken>,
    next_save: u64,
    save_error: Option<String>,
    message: String,
}
impl BrowserState {
    pub(super) fn invalidate(&mut self) {
        self.generation += 1;
        self.pending = None;
        self.attempted = None;
        self.frame = None;
        self.error = None;
    }
    pub(super) fn zone_scope(&self) -> TableScope {
        self.zone_id
            .map(TableScope::ZoneId)
            .or_else(|| self.zone_anchor.map(TableScope::Zone))
            .unwrap_or(TableScope::Encounters)
    }
    pub(super) fn reset_session(&mut self) {
        self.invalidate();
        self.auto_fallback = None;
        self.zone_anchor = None;
        self.zone_id = None;
        self.zones.clear();
        self.zones_generation += 1;
        self.zones_pending = false;
        self.zones_error = None;
        self.base = None;
        self.scope = None;
        self.history.clear();
    }
}
fn uses_global_reverse(level: aftercast::TableLevel) -> bool {
    matches!(
        level,
        aftercast::TableLevel::Combatant
            | aftercast::TableLevel::Attack
            | aftercast::TableLevel::Action
    )
}
fn base_scope(state: &AppState) -> TableScope {
    match (state.selected, &state.combatant, &state.category) {
        (Some(encounter), Some(actor), Some((category, _))) => TableScope::Attacks {
            encounter,
            combatant: actor.clone(),
            category: category.clone(),
        },
        (Some(encounter), Some(actor), None) => TableScope::Categories {
            encounter,
            combatant: actor.clone(),
        },
        (Some(encounter), None, _) => TableScope::Combatants(encounter),
        _ => state.plugin_tables.zone_scope(),
    }
}
pub(super) fn scope(state: &AppState) -> TableScope {
    let base = base_scope(state);
    if state.plugin_tables.base.as_ref() == Some(&base) {
        state.plugin_tables.scope.clone().unwrap_or(base)
    } else {
        base
    }
}
fn view(state: &AppState) -> View {
    View {
        session: state.revision.session,
        generation: state.plugin_tables.generation,
    }
}
fn target_matches(state: &AppState, token: &Token) -> bool {
    (token.mode != Mode::Auto || (state.screen == "analysis" && !state.exit_requested))
        && view(state) == token.view
        && base_scope(state) == token.base
        && scope(state) == token.scope
        && token.reverse
            == uses_global_reverse(token.scope.level())
                .then_some(state.settings.ui.main_table_reverse)
}
fn idle(state: &AppState) -> bool {
    state.phase == Phase::Ready
        && !state.loading
        && !state.detail_loading
        && !state.saving
        && !state.plugin_ui_pending
        && state.plugin_tables.pending.is_none()
}
fn current_view(state: &AppState, expected: View) -> bool {
    idle(state)
        && view(state) == expected
        && state
            .plugin_tables
            .frame
            .as_ref()
            .is_some_and(|frame| target_matches(state, &frame.token))
}
pub(super) fn dynamic(state: &AppState) -> bool {
    state.plugin_tables.frame.as_ref().is_some_and(|frame| {
        target_matches(state, &frame.token) && !frame.catalog.columns.is_empty()
    })
}
impl AppState {
    fn retire_table_view(&mut self) {
        if self.plugin_tables.zones_pending {
            self.plugin_tables.zones_generation += 1;
            self.plugin_tables.zones_pending = false;
        }
        let fallback = self.plugin_tables.auto_fallback.take().is_some();
        if fallback {
            self.loading = false;
        }
        let table = self
            .plugin_tables
            .pending
            .as_ref()
            .is_some_and(|token| token.mode == Mode::Auto);
        if table && self.phase == Phase::ReadingTables {
            self.phase = Phase::Ready;
        }
        if fallback || table {
            self.plugin_tables.invalidate();
        }
    }
    fn select_zone(&mut self, anchor: Option<aftercast::EncounterId>) -> bool {
        if !idle(self) {
            return false;
        }
        if anchor.is_some() && self.selected != anchor {
            return false;
        }
        self.apply_zone(anchor, None);
        true
    }
    fn select_listed_zone(&mut self, generation: u64, index: usize) -> bool {
        if !idle(self) || self.plugin_tables.zones_generation != generation {
            return false;
        }
        let Some(zone) = self.plugin_tables.zones.get(index) else {
            return false;
        };
        let id = zone.id;
        self.apply_zone(None, Some(id));
        true
    }
    fn apply_zone(
        &mut self,
        anchor: Option<aftercast::EncounterId>,
        id: Option<aftercast::ZoneId>,
    ) {
        self.plugin_tables.zone_anchor = anchor;
        self.plugin_tables.zone_id = id;
        self.plugin_tables.history.clear();
        self.plugin_tables.invalidate();
        self.selected = None;
        self.detail = None;
        self.combatant = None;
        self.category = None;
        self.attacks.clear();
        self.encounters.clear();
        self.total = 0;
        self.offset = 0;
        self.revision.selection += 1;
    }
    fn begin_table(&mut self, mode: Mode) -> Option<(Token, Preferences)> {
        if !idle(self) {
            return None;
        }
        let base = base_scope(self);
        let scope = scope(self);
        let key = level_key(scope.level());
        let editor = self.plugin_tables.editors.get(key);
        let preferences = match (mode, editor) {
            (Mode::Apply, Some(editor)) => editor.draft.clone(),
            (Mode::Refresh | Mode::Auto, Some(editor)) => editor.applied.clone(),
            _ => self.settings.tables.get(key).cloned().unwrap_or_default(),
        };
        if mode == Mode::Apply {
            editor?;
            if let Err(error) = preferences.validate() {
                self.plugin_tables.error = Some(error.into());
                return None;
            }
        }
        if mode == Mode::Saved && editor.is_some_and(|editor| editor.draft != editor.applied) {
            return None;
        }
        let editor_version = editor.map_or(0, |editor| editor.version);
        self.plugin_tables.generation += 1;
        let token = Token {
            view: view(self),
            base: base.clone(),
            scope: scope.clone(),
            mode,
            editor_version,
            reverse: uses_global_reverse(scope.level())
                .then_some(self.settings.ui.main_table_reverse),
        };
        if self.plugin_tables.base.as_ref() != Some(&base) {
            self.plugin_tables.history.clear();
        }
        self.plugin_tables.base = Some(base);
        self.plugin_tables.scope = Some(scope);
        self.plugin_tables.pending = Some(token.clone());
        self.plugin_tables.error = None;
        self.plugin_tables.message.clear();
        self.phase = Phase::ReadingTables;
        Some((token, preferences))
    }
    fn receive_table(&mut self, token: Token, result: Result<Loaded, String>) -> bool {
        if self
            .plugin_tables
            .pending
            .as_ref()
            .is_none_or(|pending| pending.view != token.view)
            || token.view.session != self.revision.session
        {
            return false;
        }
        self.plugin_tables.pending = None;
        if self.phase == Phase::ReadingTables {
            self.phase = Phase::Ready;
        }
        if !target_matches(self, &token) {
            self.plugin_tables.frame = None;
            return false;
        }
        self.plugin_tables.attempted = Some(token.clone());
        match result {
            Ok(mut loaded) => {
                let key = level_key(token.scope.level()).to_owned();
                let draft = match refreshed_draft(
                    self.plugin_tables.editors.get(&key),
                    &token,
                    &mut loaded,
                ) {
                    Ok(draft) => draft,
                    Err(error) => {
                        self.plugin_tables.error = Some(error.into());
                        return false;
                    }
                };
                self.plugin_tables.error = loaded.layout_error.clone();
                self.plugin_tables.editors.insert(
                    key,
                    Editor {
                        draft,
                        applied: loaded.preferences,
                        version: token.editor_version + 1,
                    },
                );
                loaded.notices.sort();
                loaded.notices.dedup();
                self.plugin_tables.frame = Some(Frame {
                    token,
                    catalog: loaded.catalog,
                    page: loaded.page,
                    offset: loaded.offset,
                    missing: loaded.missing,
                    notices: loaded.notices,
                });
            }
            Err(error) => {
                self.plugin_tables.error = Some(error);
                self.plugin_tables.frame = None;
            }
        }
        true
    }
    fn edit_table(&mut self, expected: View, change: impl FnOnce(&mut Preferences)) -> bool {
        if !current_view(self, expected) {
            return false;
        }
        let key = level_key(scope(self).level());
        let Some(editor) = self.plugin_tables.editors.get_mut(key) else {
            return false;
        };
        change(&mut editor.draft);
        editor.version += 1;
        self.plugin_tables.error = editor.draft.validate().err().map(str::to_owned);
        true
    }
    fn navigate_table(&mut self, expected: View, next: Option<TableScope>) -> bool {
        if !current_view(self, expected) {
            return false;
        }
        if let Some(next) = next {
            self.plugin_tables.history.push(scope(self));
            self.plugin_tables.scope = Some(next);
        } else {
            let previous = self
                .plugin_tables
                .history
                .pop()
                .unwrap_or_else(|| base_scope(self));
            self.plugin_tables.scope = Some(previous);
        }
        self.plugin_tables.invalidate();
        true
    }
    fn begin_table_save(
        &mut self,
        expected: View,
    ) -> Option<(SaveToken, camino::Utf8PathBuf, ConfigFile)> {
        if !current_view(self, expected) {
            return None;
        }
        let frame = self.plugin_tables.frame.as_ref()?;
        if frame.catalog.columns.is_empty()
            || frame
                .page
                .as_ref()
                .is_some_and(|page| page.sort_error.is_some())
        {
            return None;
        }
        let level = level_key(frame.catalog.level).to_owned();
        let editor = self.plugin_tables.editors.get(&level)?;
        if editor.draft != editor.applied || editor.applied.validate().is_err() {
            return None;
        }
        let preferences = editor.applied.clone();
        let mut file = self.file.clone()?;
        file.settings = self.settings.clone();
        file.settings
            .tables
            .insert(level.clone(), preferences.clone());
        self.plugin_tables.next_save += 1;
        let token = SaveToken {
            id: self.plugin_tables.next_save,
            view: expected,
            level,
            preferences,
        };
        self.plugin_tables.save = Some(token.clone());
        self.plugin_tables.save_error = None;
        self.saving = true;
        Some((token, self.root.clone(), file))
    }
    fn receive_table_save(
        &mut self,
        token: SaveToken,
        result: miette::Result<ConfigFile>,
        message: String,
    ) -> bool {
        if self
            .plugin_tables
            .save
            .as_ref()
            .is_none_or(|pending| pending.id != token.id)
        {
            return false;
        }
        self.plugin_tables.save = None;
        self.saving = false;
        match result {
            Ok(file) => {
                // Accepted disk I/O survives Stop; sync durable keys without acknowledging or replacing a new draft/frame.
                self.settings.tables.insert(token.level, token.preferences);
                self.file = Some(file);
                if view(self) == token.view {
                    self.plugin_tables.message = message;
                }
            }
            Err(error) => {
                self.plugin_tables.save_error = Some(
                    error
                        .chain()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(": "),
                )
            }
        }
        true
    }
}
fn refreshed_draft(
    editor: Option<&Editor>,
    token: &Token,
    loaded: &mut Loaded,
) -> Result<Preferences, &'static str> {
    let Some(editor) = editor else {
        return Ok(loaded.preferences.clone());
    };
    if !matches!(token.mode, Mode::Refresh | Mode::Auto) && editor.version == token.editor_version {
        return Ok(loaded.preferences.clone());
    }
    let reconciled = editor.draft.reconcile(&loaded.catalog)?;
    loaded.notices.extend(notices(&reconciled));
    Ok(reconciled.preferences)
}
fn notices(reconciled: &crate::table_settings::Reconciled) -> Vec<Notice> {
    let mut notices = Vec::new();
    for (kind, keys) in [
        ("table-removed", &reconciled.removed),
        ("table-added", &reconciled.added),
        ("table-sort-reset", &reconciled.reset_sorts),
    ] {
        if !keys.is_empty() {
            notices.push(Notice {
                kind,
                keys: keys.clone(),
            });
        }
    }
    notices
}
async fn read(
    handle: &HostHandle,
    token: &Token,
    preferences: Preferences,
    offset: u32,
) -> Result<Loaded, String> {
    let catalog = handle
        .table_columns(token.scope.level())
        .await
        .map_err(|error| error.to_string())?;
    let reconciled = preferences.reconcile(&catalog).map_err(str::to_owned)?;
    let notices = notices(&reconciled);
    let preferences = reconciled.preferences;
    let query = match preferences.query(token.scope.clone(), &catalog, offset, PAGE) {
        Ok(query) => query.map(|mut query| {
            if let (Some(sort), Some(reverse)) = (&mut query.sort, token.reverse) {
                sort.descending = reverse;
            }
            query
        }),
        Err(error) => {
            return Ok(Loaded {
                layout_error: Some(error.into()),
                catalog,
                preferences,
                page: None,
                offset,
                missing: false,
                notices,
            });
        }
    };
    let (mut page, mut missing) = (None, false);
    if let Some(query) = query {
        page = if let Some(reverse) = token.reverse {
            handle
                .main_table_rows(
                    &query,
                    reverse,
                    token.mode != Mode::Auto && query.sort.is_some(),
                )
                .await
        } else {
            handle.table_rows(&query).await
        }
        .map_err(|error| error.to_string())?;
        missing = page.is_none();
    }
    Ok(Loaded {
        layout_error: None,
        catalog,
        preferences,
        page,
        offset,
        missing,
        notices,
    })
}
fn load(mut state: Signal<AppState>, mode: Mode, offset: u32) {
    let Some(handle) = state.read().handle() else {
        return;
    };
    let Some((token, preferences)) = state.write().begin_table(mode) else {
        return;
    };
    spawn_forever(async move {
        let result = read(&handle, &token, preferences, offset).await;
        let accepted = state.write().receive_table(token, result);
        let current = state.read();
        let refresh_sidebar = accepted
            && current.screen == "analysis"
            && !current.exit_requested
            && scope(&current).level() == aftercast::TableLevel::Encounter
            && (current.plugin_tables.zone_scope() != TableScope::Encounters
                || current.encounters.is_empty());
        let sidebar_offset = current.offset;
        drop(current);
        if refresh_sidebar {
            refresh_fallback(state, sidebar_offset);
        }
    });
}
impl AppState {
    fn begin_auto_fallback(&mut self) -> Option<(View, super::Revision)> {
        if !idle(self) {
            return None;
        }
        let expected = view(self);
        self.plugin_tables.auto_fallback = Some(expected);
        self.loading = true;
        Some((expected, self.revision))
    }
    fn receive_auto_fallback(
        &mut self,
        expected: View,
        revision: super::Revision,
        offset: u32,
        result: Result<aftercast::EncounterPage<aftercast::EncounterSummary>, aftercast::Error>,
    ) -> bool {
        if self.plugin_tables.auto_fallback != Some(expected) {
            return false;
        }
        self.plugin_tables.auto_fallback = None;
        self.loading = false;
        if view(self) != expected
            || self.revision != revision
            || self.screen != "analysis"
            || self.exit_requested
        {
            return false;
        }
        match result {
            Ok(page) => {
                self.encounters = page.items;
                self.total = page.total;
                self.offset = offset;
            }
            Err(error) => self.fail(error),
        }
        true
    }
}
async fn read_zones(handle: &HostHandle) -> Result<Vec<aftercast::ZoneSummary>, String> {
    let mut zones = Vec::new();
    let mut ids = std::collections::HashSet::new();
    let mut total = None;
    loop {
        let page = handle
            .zones(zones.len() as u32, 128)
            .await
            .map_err(|error| error.to_string())?;
        if total.is_some_and(|value| value != page.total)
            || page.items.iter().any(|zone| !ids.insert(zone.id))
            || (page.items.is_empty() && zones.len() < page.total as usize)
        {
            return Err("Zone list changed while reading; refresh it.".to_owned());
        }
        total = Some(page.total);
        zones.extend(page.items);
        if zones.len() == page.total as usize {
            return Ok(zones);
        }
        if zones.len() > page.total as usize {
            return Err("Invalid Zone list.".to_owned());
        }
    }
}
fn load_zones(mut state: Signal<AppState>) {
    let Some(handle) = state.read().handle() else {
        return;
    };
    let (expected, generation) = {
        let mut current = state.write();
        if !idle(&current) || current.plugin_tables.zones_pending {
            return;
        }
        let expected = view(&current);
        current.plugin_tables.zones_generation += 1;
        current.plugin_tables.zones_pending = true;
        current.plugin_tables.zones.clear();
        current.plugin_tables.zones_error = None;
        (expected, current.plugin_tables.zones_generation)
    };
    spawn_forever(async move {
        let result = read_zones(&handle).await;
        let mut current = state.write();
        if current.plugin_tables.zones_generation != generation {
            return;
        }
        current.plugin_tables.zones_pending = false;
        if view(&current) != expected || current.screen != "analysis" || current.exit_requested {
            return;
        }
        match result {
            Ok(zones) => current.plugin_tables.zones = zones,
            Err(error) => current.plugin_tables.zones_error = Some(error),
        }
    });
}
fn refresh_fallback(mut state: Signal<AppState>, offset: u32) {
    let Some(handle) = state.read().handle() else {
        return;
    };
    let zone_scope = state.read().plugin_tables.zone_scope();
    let Some((expected, revision)) = state.write().begin_auto_fallback() else {
        return;
    };
    spawn_forever(async move {
        let result = super::list_encounters(&handle, zone_scope, offset).await;
        state
            .write()
            .receive_auto_fallback(expected, revision, offset, result);
    });
}

fn save(mut state: Signal<AppState>, expected: View) {
    let Some((token, root, file)) = state.write().begin_table_save(expected) else {
        return;
    };
    let message = tr("table-saved");
    spawn_forever(async move {
        let result = tokio::task::spawn_blocking(move || file.save(&root))
            .await
            .map_err(|error| miette::miette!(error))
            .and_then(|result| result);
        state.write().receive_table_save(token, result, message);
    });
}
// A single pane-owned clock admits at most one refresh. A changed setting/session
// starts a new interval; hidden panes and disabled refresh have no deadline.
#[derive(Default)]
struct RefreshClock {
    key: Option<(u64, u8)>,
    deadline: std::time::Duration,
}
impl RefreshClock {
    fn tick(
        &mut self,
        session: u64,
        seconds: u8,
        visible: bool,
        ready: bool,
        now: std::time::Duration,
    ) -> bool {
        if !visible || seconds == 0 {
            self.key = None;
            return false;
        }
        let interval = std::time::Duration::from_secs(u64::from(seconds));
        if self.key != Some((session, seconds)) {
            self.key = Some((session, seconds));
            self.deadline = now + interval;
            return false;
        }
        if ready && now >= self.deadline {
            self.deadline = now + interval;
            return true;
        }
        false
    }
}

#[component]
pub(super) fn PluginTablesPane() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    use_drop(move || {
        if let Ok(mut current) = state.try_write() {
            current.retire_table_view();
        }
    });
    use_future(move || async move {
        let start = std::time::Instant::now();
        let mut clock = RefreshClock::default();
        loop {
            dioxus_sdk::time::sleep(std::time::Duration::from_millis(100)).await;
            let current = state.read();
            let visible = current.screen == "analysis"
                && !current.exit_requested
                && current.running.is_some();
            let fallback = current
                .plugin_tables
                .frame
                .as_ref()
                .is_some_and(|frame| frame.catalog.columns.is_empty());
            let offset = if fallback {
                current.offset
            } else {
                current
                    .plugin_tables
                    .frame
                    .as_ref()
                    .map_or(0, |frame| frame.offset)
            };
            let due = clock.tick(
                current.revision.session,
                current.settings.ui.main_table_refresh_seconds,
                visible,
                idle(&current),
                start.elapsed(),
            );
            drop(current);
            if due {
                if fallback {
                    refresh_fallback(state, offset);
                } else {
                    load(state, Mode::Auto, offset);
                }
            }
        }
    });
    use_effect(move || {
        let current = state.read();
        let needed = idle(&current)
            && current
                .plugin_tables
                .attempted
                .as_ref()
                .is_none_or(|token| !target_matches(&current, token));
        drop(current);
        if needed {
            load(state, Mode::Refresh, 0);
        }
    });
    rsx! {
        controls::Panel {}
    }
}
#[cfg(test)]
mod tests;
