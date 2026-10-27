use super::{AppState, Phase, tr};
use crate::components::{Accordion, Badge, Button, ButtonVariant, Input, Label};
use aftercast::{
    HistoryEntry, HistoryId, HistoryPage, HistoryQuery, HistoryRevision, HistorySearchScope,
    HostHandle,
};
use dioxus::{core::spawn_forever, prelude::*};
const PAGE: u32 = 16;
const MAX_TICKS: u64 = 3_155_378_975_999_999_999;
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SearchDraft {
    text: String,
    scope: HistorySearchScope,
    start: String,
    end: String,
}
#[derive(Debug, Clone)]
struct Applied {
    draft: SearchDraft,
    query: HistoryQuery,
}
impl SearchDraft {
    fn compile(&self) -> Result<Applied, &'static str> {
        let start = bound(&self.start, false)?;
        let end = bound(&self.end, true)?;
        if matches!((start,end),(Some(start),Some(end)) if start >= end) {
            return Err("history-invalid-range");
        }
        Ok(Applied {
            draft: self.clone(),
            query: HistoryQuery {
                text: self.text.clone(),
                scope: self.scope,
                start_ticks: start,
                end_ticks: end,
                ..Default::default()
            },
        })
    }
}
fn shape(value: &str, clock: bool) -> bool {
    let length = value.len();
    if clock && length != 19 && !(21..=27).contains(&length) || !clock && length != 10 {
        return false;
    }
    value.bytes().enumerate().all(|(index, byte)| match index {
        4 | 7 => byte == b'-',
        10 if clock => byte == b'T',
        13 | 16 if clock => byte == b':',
        19 if clock => byte == b'.',
        _ => byte.is_ascii_digit(),
    })
}
fn bound(value: &str, end: bool) -> Result<Option<u64>, &'static str> {
    let value = value.trim().replace(' ', "T");
    if value.is_empty() {
        return Ok(None);
    }
    let clock = value.len() > 10;
    if !shape(&value, clock) || clock && value.get(17..19).is_some_and(|seconds| seconds > "59") {
        return Err("history-invalid-date");
    }
    let datetime = if clock {
        value
            .parse::<jiff::civil::DateTime>()
            .map_err(|_error| "history-invalid-date")?
    } else {
        let mut date = value
            .parse::<jiff::civil::Date>()
            .map_err(|_error| "history-invalid-date")?;
        if !(1..=9999).contains(&date.year()) {
            return Err("history-invalid-date");
        }
        if end {
            if date == jiff::civil::date(9999, 12, 31) {
                return Ok(Some(MAX_TICKS + 1));
            }
            date = date.tomorrow().map_err(|_error| "history-invalid-date")?;
        }
        date.at(0, 0, 0, 0)
    };
    if !(1..=9999).contains(&datetime.year()) {
        return Err("history-invalid-date");
    }
    // UTC is a numeric origin for civil coordinates here, never an inferred offset for stored local times.
    let nanos = datetime
        .to_zoned(jiff::tz::TimeZone::UTC)
        .map_err(|_error| "history-invalid-date")?
        .timestamp()
        .as_nanosecond();
    let ticks = u64::try_from(nanos / 100 + 621_355_968_000_000_000)
        .map_err(|_error| "history-invalid-date")?;
    if ticks > MAX_TICKS {
        return Err("history-invalid-date");
    }
    Ok(Some(ticks))
}
#[derive(Debug, Clone)]
struct Selected {
    entry: HistoryEntry,
    ordinal: u32,
}
#[derive(Debug, Clone)]
struct Confirmation {
    generation: u64,
    version: u64,
    selection_version: u64,
    revision: HistoryRevision,
    rows: Vec<Selected>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Token {
    session: u64,
    generation: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Query,
    Restore,
}
#[derive(Default)]
pub(super) struct BrowserState {
    loaded: bool,
    draft: SearchDraft,
    applied: Option<Applied>,
    page: Option<HistoryPage>,
    generation: u64,
    pending: Option<(Token, Kind)>,
    stale: bool,
    selected: Vec<Selected>,
    selection_version: u64,
    confirmation_version: u64,
    confirmation: Option<Confirmation>,
}
impl BrowserState {
    fn clear_selection(&mut self) {
        self.selected.clear();
        self.selection_version += 1;
        self.confirmation = None;
    }
    pub(super) fn invalidate_snapshot(&mut self) {
        self.loaded = false;
        self.stale = true;
        self.clear_selection();
    }
    fn toggle(&mut self, generation: u64, revision: &HistoryRevision, id: &HistoryId) -> bool {
        if self.pending.is_some() || self.stale || generation != self.generation {
            return false;
        }
        let Some(page) = &self.page else {
            return false;
        };
        if &page.revision != revision {
            return false;
        }
        let Some((index, entry)) = page
            .items
            .iter()
            .enumerate()
            .find(|(_, entry)| &entry.id == id)
        else {
            return false;
        };
        if entry.already_restored || !entry.payload_present {
            return false;
        }
        if let Some(position) = self
            .selected
            .iter()
            .position(|selected| &selected.entry.id == id)
        {
            self.selected.remove(position);
        } else if self.selected.len() < 64 {
            self.selected.push(Selected {
                entry: entry.clone(),
                ordinal: self
                    .applied
                    .as_ref()
                    .map_or(0, |applied| applied.query.offset)
                    + index as u32
                    + 1,
            });
        } else {
            return false;
        }
        self.selection_version += 1;
        self.confirmation = None;
        true
    }
    fn confirm(&mut self) -> bool {
        if self.pending.is_some() || self.stale || self.selected.is_empty() {
            return false;
        }
        let Some(page) = &self.page else {
            return false;
        };
        self.confirmation_version += 1;
        self.confirmation = Some(Confirmation {
            generation: self.generation,
            version: self.confirmation_version,
            selection_version: self.selection_version,
            revision: page.revision.clone(),
            rows: self.selected.clone(),
        });
        true
    }
    fn cancel_confirmation(&mut self, version: u64) -> bool {
        if self.pending.is_some()
            || self
                .confirmation
                .as_ref()
                .is_none_or(|current| current.version != version)
        {
            return false;
        }
        self.confirmation = None;
        true
    }
    fn confirmation_current(&self, confirmation: &Confirmation) -> bool {
        !self.stale
            && self.generation == confirmation.generation
            && self.selection_version == confirmation.selection_version
            && self
                .page
                .as_ref()
                .is_some_and(|page| page.revision == confirmation.revision)
            && self.confirmation.as_ref().is_some_and(|current| {
                current.version == confirmation.version && current.revision == confirmation.revision
            })
            && self
                .selected
                .iter()
                .map(|row| &row.entry.id)
                .eq(confirmation.rows.iter().map(|row| &row.entry.id))
    }
    fn accept(&mut self, applied: Applied, page: HistoryPage) {
        if self
            .page
            .as_ref()
            .is_none_or(|old| old.revision != page.revision)
        {
            self.clear_selection();
        }
        self.selected.retain(|selected| {
            !page
                .items
                .iter()
                .any(|entry| entry.id == selected.entry.id && entry.already_restored)
        });
        self.applied = Some(applied);
        self.page = Some(page);
        self.stale = false;
    }
}
impl AppState {
    fn begin_history_query(&mut self, keep_selection: bool, clear_error: bool) -> Option<Token> {
        if self.phase != Phase::Ready || self.loading || self.history.pending.is_some() {
            return None;
        }
        self.history.generation += 1;
        self.history.confirmation = None;
        if !keep_selection {
            self.history.clear_selection();
        }
        let token = Token {
            session: self.revision.session,
            generation: self.history.generation,
        };
        self.history.pending = Some((token, Kind::Query));
        self.history.loaded = true;
        self.phase = Phase::ReadingHistory;
        if clear_error {
            self.error = None;
        }
        Some(token)
    }
    fn cancel_history_query(&mut self, expected: Token, message: String) -> bool {
        let Some((token, Kind::Query)) = self.history.pending else {
            return false;
        };
        if token != expected
            || token.session != self.revision.session
            || token.generation != self.history.generation
        {
            return false;
        }
        self.history.generation += 1;
        self.message = message;
        true
    }
    fn receive_history_query(
        &mut self,
        token: Token,
        result: Result<(Applied, HistoryPage), aftercast::Error>,
    ) -> bool {
        if token.session != self.revision.session
            || self.history.pending != Some((token, Kind::Query))
        {
            return false;
        }
        self.history.pending = None;
        if self.phase == Phase::ReadingHistory {
            self.phase = Phase::Ready;
        }
        if token.generation != self.history.generation {
            return false;
        } // Canceled reads cannot replace rows or surface old errors.
        match result {
            Ok((applied, page)) => self.history.accept(applied, page),
            Err(error) => {
                self.history.stale = true;
                self.history.clear_selection();
                self.fail(error);
            }
        }
        true
    }
    fn begin_history_restore(&mut self, confirmation: &Confirmation) -> Option<Token> {
        if self.phase != Phase::Ready
            || self.loading
            || self.history.pending.is_some()
            || !self
                .plugins
                .iter()
                .any(|plugin| plugin.info.state == aftercast::PluginState::Initialized)
            || !self.history.confirmation_current(confirmation)
        {
            return None;
        }
        let token = Token {
            session: self.revision.session,
            generation: self.history.generation,
        };
        self.history.pending = Some((token, Kind::Restore));
        self.phase = Phase::Archiving;
        self.error = None;
        self.invalidate_restored_context();
        Some(token)
    }
    fn receive_history_restore(
        &mut self,
        token: Token,
        result: Result<u32, aftercast::Error>,
        message: String,
    ) -> bool {
        if token.session != self.revision.session
            || self.history.pending != Some((token, Kind::Restore))
        {
            return false;
        }
        self.history.pending = None;
        self.invalidate_restored_context();
        self.phase = Phase::Ready;
        match result {
            Ok(count) => self.message = format!("{message}: {count}"),
            Err(error) => self.fail(error),
        }
        true
    }
    pub(super) fn invalidate_restored_context(&mut self) {
        self.log_windows.invalidate();
        self.revision.selection += 1;
        self.selected = None;
        self.detail = None;
        self.detail_loading = false;
        self.attacks.clear();
        self.combatant = None;
        self.category = None;
        self.history.invalidate_snapshot();
    }
}
async fn read(
    handle: &HostHandle,
    mut applied: Applied,
) -> Result<(Applied, HistoryPage), aftercast::Error> {
    let mut page = handle.query_history(&applied.query).await?;
    if applied.query.offset > 0 && page.items.is_empty() {
        applied.query.offset = 0;
        page = handle.query_history(&applied.query).await?;
    }
    Ok((applied, page))
}
fn load(mut state: Signal<AppState>, applied: Applied, keep_selection: bool, clear_error: bool) {
    let Some(handle) = state.read().handle() else {
        return;
    };
    let Some(token) = state
        .write()
        .begin_history_query(keep_selection, clear_error)
    else {
        return;
    };
    spawn_forever(async move {
        let result = read(&handle, applied).await;
        state.write().receive_history_query(token, result);
    });
}
fn search(mut state: Signal<AppState>) {
    let applied = state.read().history.draft.compile();
    match applied {
        Ok(applied) => load(state, applied, false, true),
        Err(error) => state.write().fail(tr(error)),
    }
}
fn move_page(state: Signal<AppState>, view: Token, offset: u32) {
    if !current_view(state, view) {
        return;
    }
    let applied = state.read().history.applied.clone();
    if let Some(mut applied) = applied {
        applied.query.offset = offset;
        load(state, applied, true, true);
    }
}
fn restore(mut state: Signal<AppState>, confirmation: Confirmation) {
    let Some(handle) = state.read().handle() else {
        return;
    };
    let Some(token) = state.write().begin_history_restore(&confirmation) else {
        return;
    };
    let message = tr("history-restored");
    spawn_forever(async move {
        let ids = confirmation
            .rows
            .iter()
            .map(|row| row.entry.id.clone())
            .collect::<Vec<_>>();
        let result = handle
            .restore_selected_history(&confirmation.revision, &ids)
            .await;
        let received = state
            .write()
            .receive_history_restore(token, result, message);
        if received {
            super::refresh(state, 0);
        }
    });
}
fn ready(state: Signal<AppState>) -> bool {
    let current = state.read();
    current.phase == Phase::Ready && !current.loading && current.history.pending.is_none()
}
fn current_view(state: Signal<AppState>, view: Token) -> bool {
    let current = state.read();
    current.revision.session == view.session
        && current.history.generation == view.generation
        && current.phase == Phase::Ready
        && !current.loading
        && current.history.pending.is_none()
}
fn edit(mut state: Signal<AppState>, field: &str, value: String) {
    if !ready(state) {
        return;
    }
    let mut current = state.write();
    match field {
        "history-text" => current.history.draft.text = value,
        "history-start" => current.history.draft.start = value,
        "history-end" => current.history.draft.end = value,
        _ => return,
    }
    current.history.confirmation = None;
}
fn scopes() -> [(HistorySearchScope, &'static str); 4] {
    [
        (HistorySearchScope::Any, "history-any"),
        (HistorySearchScope::Zone, "history-zone"),
        (HistorySearchScope::Title, "history-title"),
        (HistorySearchScope::Character, "history-character"),
    ]
}
#[component]
pub(super) fn HistoryPane() -> Element {
    let state = use_context::<Signal<AppState>>();
    use_effect(move || {
        let current = state.read();
        if current.phase != Phase::Ready
            || current.loading
            || current.history.loaded
            || current.history.pending.is_some()
        {
            return;
        }
        let mut applied = current
            .history
            .applied
            .clone()
            .unwrap_or_else(|| SearchDraft::default().compile().expect("default query"));
        applied.query.offset = 0;
        drop(current);
        load(state, applied, false, false);
    });
    rsx! {
        section { class: "space-y-6 p-6 min-[1000px]:p-8",
            h1 { class: "text-2xl font-semibold", {tr("history")} }
            p { class: "max-w-3xl text-sm leading-7 text-muted-foreground",
                {tr("history-browser-help")}
            }
            if state.read().phase == Phase::Idle {
                p { {tr("history-start-host")} }
            }
            HistorySearch {}
            HistoryResults {}
        }
    }
}
#[component]
fn HistorySearch() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let blocked = !ready(state);
    let draft = state.read().history.draft.clone();
    let pending = state.read().history.pending;
    let generation = state.read().history.generation;
    let cancelable =
        pending.is_some_and(|(token, kind)| kind == Kind::Query && token.generation == generation);
    rsx! {
        Accordion {
            default_open: true,
            class: "rounded-md border border-border".to_owned(),
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! {
                {tr("history-search")}
            },
            section { class: "space-y-4 p-4", "aria-label": tr("history-search"),
                for (id, value) in [
                    ("history-text", draft.text),
                    ("history-start", draft.start),
                    ("history-end", draft.end),
                ]
                {
                    div { class: "space-y-1",
                        Label { html_for: id.to_owned(), {tr(id)} }
                        Input {
                            id: id.to_owned(),
                            value,
                            disabled: blocked,
                            oninput: move |event: FormEvent| edit(state, id, event.value()),
                        }
                    }
                }
                div {
                    class: "flex flex-wrap gap-2",
                    role: "group",
                    "aria-label": tr("history-scope"),
                    for (scope, label) in scopes() {
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: blocked,
                            aria_pressed: draft.scope == scope,
                            onclick: move |_| {
                                if ready(state) {
                                    let mut current = state.write();
                                    current.history.draft.scope = scope;
                                    current.history.confirmation = None;
                                }
                            },
                            {tr(label)}
                        }
                    }
                }
                p { class: "text-xs leading-6 text-muted-foreground", {tr("history-date-help")} }
                div { class: "flex gap-2",
                    Button { disabled: blocked, onclick: move |_| search(state),
                        {tr("history-search-action")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: !cancelable,
                        onclick: move |_| {
                            if let Some((token, Kind::Query)) = pending {
                                state.write().cancel_history_query(token, tr("history-search-canceled"));
                            }
                        },
                        {tr("history-cancel-search")}
                    }
                }
                if let Some((token, kind)) = pending {
                    p { role: "status",
                        {
                            tr(
                                match kind {
                                    Kind::Restore => "history-restore-running",
                                    Kind::Query if token.generation != generation => {
                                        "history-query-finishing"
                                    }
                                    Kind::Query => "history-query-running",
                                },
                            )
                        }
                    }
                }
            }
        }
    }
}
#[component]
fn HistoryResults() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let blocked = !ready(state);
    let stale = current.history.stale;
    let generation = current.history.generation;
    let view = Token {
        session: current.revision.session,
        generation,
    };
    let page = current.history.page.clone();
    let selected = current.history.selected.clone();
    let confirmation = current.history.confirmation.clone();
    let confirmation_version = confirmation
        .as_ref()
        .map_or(0, |confirmation| confirmation.version);
    let applied = current.history.applied.clone();
    let parser = current
        .plugins
        .iter()
        .any(|plugin| plugin.info.state == aftercast::PluginState::Initialized);
    drop(current);
    let offset = applied.as_ref().map_or(0, |applied| applied.query.offset);
    let total = page.as_ref().map_or(0, |page| page.total);
    let can_restore = !blocked && !stale && !selected.is_empty() && parser;
    rsx! {
        Accordion {
            default_open: true,
            class: "rounded-md border border-border".to_owned(),
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! {
                {tr("history-results")}
            },
            section { class: "space-y-4 p-4", "aria-label": tr("history-results"),
                if let Some(applied) = applied.clone() {
                    p { class: "text-sm text-muted-foreground",
                        {tr("history-applied")}
                        " "
                        {
                            tr(
                                scopes()
                                    .into_iter()
                                    .find(|(scope, _)| *scope == applied.draft.scope)
                                    .map_or("history-any", |(_, label)| label),
                            )
                        }
                        " · {applied.draft.text} · {applied.draft.start} — {applied.draft.end}"
                    }
                }
                div { class: "flex flex-wrap gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked || applied.is_none(),
                        onclick: move |_| move_page(state, view, 0),
                        {tr("history-refresh")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked || selected.is_empty(),
                        onclick: move |_| {
                            if current_view(state, view) {
                                state.write().history.clear_selection();
                            }
                        },
                        {tr("history-clear-selection")}
                    }
                    Button {
                        disabled: !can_restore,
                        onclick: move |_| {
                            if current_view(state, view) {
                                state.write().history.confirm();
                            }
                        },
                        {tr("history-review-selection")}
                    }
                }
                p { class: "text-sm",
                    {tr("history-selection-count")}
                    " {selected.len()} / 64"
                }
                if stale {
                    p { class: "text-sm text-muted-foreground", {tr("history-stale")} }
                }
                if !parser {
                    p { class: "text-xs text-muted-foreground", {tr("history-parser-required")} }
                }
                if let Some(page) = page {
                    p { class: "text-xs text-muted-foreground",
                        "{offset} / {page.total} · "
                        {tr("history-indexed")}
                        " {page.indexed_count}"
                    }
                    if !page.index_present {
                        p { {tr("history-no-index")} }
                    } else if page.items.is_empty() {
                        p { {tr("history-no-matches")} }
                    }
                    for (index, entry) in page.items.into_iter().enumerate() {
                        {
                            let id = entry.id.clone();
                            let key = format!("{id:?}");
                            let revision = page.revision.clone();
                            let checked = selected.iter().any(|row| row.entry.id == id);
                            let row_blocked = blocked || stale || entry.already_restored
                                || !entry.payload_present || selected.len() == 64 && !checked;
                            rsx! {
                                Accordion {
                                    key: "{key}",
                                    default_open: true,
                                    header: rsx! {
                                        span { "{offset + index as u32 + 1}. {entry.title}" }
                                    },
                                    div { class: "space-y-2 p-3 text-sm",
                                        p { "{entry.zone} · {entry.character}" }
                                        p { class: "text-xs text-muted-foreground",
                                            {super::encounter_context::format_time(entry.start)}
                                            " — "
                                            {super::encounter_context::format_time(entry.end)}
                                        }
                                        p { class: "break-words text-xs text-muted-foreground",
                                            {tr("history-folder-hint")}
                                            " {entry.folder_hint}"
                                        }
                                        div { class: "flex flex-wrap gap-2",
                                            Badge { {tr(if entry.partial { "history-partial" } else { "history-complete" })} }
                                            Badge {
                                                {
                                                    tr(
                                                        if entry.payload_present {
                                                            "history-payload-present"
                                                        } else {
                                                            "history-payload-missing"
                                                        },
                                                    )
                                                }
                                            }
                                            if entry.already_restored {
                                                Badge { {tr("history-already-restored")} }
                                            }
                                        }
                                        Button {
                                            variant: ButtonVariant::Outline,
                                            aria_pressed: checked,
                                            disabled: row_blocked,
                                            onclick: move |_| {
                                                if current_view(state, view) {
                                                    state.write().history.toggle(generation, &revision, &id);
                                                }
                                            },
                                            {tr(if checked { "history-unselect" } else { "history-select" })}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(confirmation) = confirmation {

                    div {
                        class: "space-y-3 rounded-md border border-border p-3",
                        role: "group",
                        "aria-label": tr("history-confirm-title"),
                        p { {tr("history-confirm-help")} }
                        for row in &confirmation.rows {
                            p {
                                "{row.ordinal}. {row.entry.title} · {row.entry.zone} · "
                                {super::encounter_context::format_time(row.entry.start)}
                            }
                        }
                        Button {
                            disabled: !can_restore,
                            onclick: move |_| restore(state, confirmation.clone()),
                            {tr("history-confirm-restore")}
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: blocked,
                            onclick: move |_| {
                                if current_view(state, view) {
                                    state.write().history.cancel_confirmation(confirmation_version);
                                }
                            },
                            {tr("history-cancel-confirm")}
                        }
                    }
                }
                div { class: "flex gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked || offset == 0,
                        onclick: move |_| move_page(state, view, offset.saturating_sub(PAGE)),
                        {tr("previous")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked || offset.saturating_add(PAGE) >= total,
                        onclick: move |_| move_page(state, view, offset.saturating_add(PAGE)),
                        {tr("next")}
                    }
                }
            }
        }
    }
}
#[cfg(test)]
mod tests;
