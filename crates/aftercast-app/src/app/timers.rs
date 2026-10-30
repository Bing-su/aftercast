use super::{AppState, Phase, tr};
pub(super) mod config_batch;
mod snippet;
use crate::components::{Accordion, Button, ButtonVariant, Input, Label};
use aftercast::{HostHandle, SavedTimer, TimerDefinition, TimerItem, TimerPage, TimerRevision};
use dioxus::{core::spawn_forever, prelude::*};
const PAGE: u32 = 16;

#[derive(Debug, Clone, PartialEq)]
struct Draft {
    definition: TimerDefinition,
    duration: String,
    warning: String,
    removal: String,
    color: String,
}
impl Default for Draft {
    fn default() -> Self {
        Self::from(TimerDefinition::default())
    }
}
impl From<TimerDefinition> for Draft {
    fn from(definition: TimerDefinition) -> Self {
        Self {
            duration: definition.timer_value.to_string(),
            warning: definition.warning_value.to_string(),
            removal: definition.remove_value.to_string(),
            color: format!("#{:08X}", definition.fill_color_argb.cast_unsigned()),
            definition,
        }
    }
}
impl Draft {
    fn compile(&self) -> Result<TimerDefinition, &'static str> {
        let number = |text: &str| {
            text.parse::<i32>()
                .map_err(|_parse_error| "timer-invalid-number")
        };
        let mut definition = self.definition.clone();
        definition.timer_value = number(&self.duration)?;
        definition.warning_value = number(&self.warning)?;
        definition.remove_value = number(&self.removal)?;
        let color = self
            .color
            .strip_prefix('#')
            .filter(|value| value.len() == 8)
            .and_then(|value| u32::from_str_radix(value, 16).ok())
            .ok_or("timer-invalid-color")?;
        definition.fill_color_argb = color.cast_signed();
        Ok(definition)
    }
}
#[derive(Default)]
pub(super) struct EditorState {
    snippet: snippet::State,
    config_batch: config_batch::State,
    pub(super) loaded: bool,
    busy: bool,
    page: Option<TimerPage>,
    offset: u32,
    draft: Draft,
    key: Option<String>,
    revision: Option<TimerRevision>,
    dirty: bool,
    deleting: Option<(String, TimerRevision)>,
}
impl EditorState {
    pub(super) fn receive_recast_draft(
        &mut self,
        name: &str,
        attacker: &str,
        duration: i32,
        modable: bool,
    ) -> Result<(), &'static str> {
        if self.dirty || self.busy {
            return Err("Save or discard the current timer draft first.");
        }
        self.new_draft();
        self.draft.definition.name = name.to_owned();
        self.draft.definition.category = attacker.to_owned();
        self.draft.definition.modable = modable;
        self.draft.duration = duration.to_string();
        self.dirty = true;
        Ok(())
    }
    fn new_draft(&mut self) {
        self.key = None;
        self.draft = Draft::default();
        self.dirty = false;
        self.deleting = None;
        self.revision = self.page.as_ref().map(|page| page.revision);
    }
    fn select(&mut self, item: TimerItem, revision: TimerRevision) -> bool {
        if self.dirty {
            return false;
        }
        self.key = Some(item.key);
        self.draft = Draft::from(item.definition);
        self.revision = Some(revision);
        self.deleting = None;
        true
    }
    fn page(&mut self, offset: u32, page: TimerPage) {
        if self.key.is_none() {
            self.revision = Some(page.revision);
        }
        self.offset = offset;
        self.page = Some(page);
    }
    fn saved(&mut self, saved: SavedTimer) {
        // Keys/category normalization come from managed ACT; never reconstruct culture-dependent keys here.
        self.key = Some(saved.item.key);
        self.draft = Draft::from(saved.item.definition);
        self.revision = Some(saved.revision);
        self.dirty = false;
        self.deleting = None;
    }
    fn rebase(&mut self, key: &str, revision: TimerRevision) -> bool {
        if !self.dirty
            || self.key.as_deref() != Some(key)
            || self.page.as_ref().is_none_or(|page| {
                page.revision != revision || !page.items.iter().any(|item| item.key == key)
            })
        {
            return false;
        }
        self.revision = Some(revision);
        true
    }
    fn keep_as_new(&mut self, revision: TimerRevision) -> bool {
        if !self.dirty
            || self.key.is_none()
            || self
                .page
                .as_ref()
                .is_none_or(|page| page.revision != revision)
        {
            return false;
        }
        self.key = None;
        self.revision = Some(revision);
        self.deleting = None;
        true
    }
}
impl AppState {
    fn begin_timer_request(&mut self, mutation: bool) -> Option<u64> {
        if self.phase != Phase::Ready || self.timers.busy {
            return None;
        }
        self.timers.busy = true;
        if mutation {
            self.phase = Phase::UpdatingTimers;
            self.error = None;
        }
        Some(self.revision.session)
    }
    fn finish_timer_request(&mut self, session: u64) -> bool {
        if self.revision.session != session {
            return false;
        }
        self.timers.busy = false;
        if self.phase == Phase::UpdatingTimers {
            self.phase = Phase::Ready;
        }
        true
    }
    fn receive_timer_ack(
        &mut self,
        session: u64,
        mutation: &Mutation,
        result: Result<Change, aftercast::Error>,
        message: String,
    ) -> bool {
        if self.revision.session != session {
            return false;
        }
        match result {
            Ok(change) => {
                mutation.apply(&mut self.timers, change);
                self.message = message;
                true
            }
            Err(error) => {
                self.finish_timer_request(session);
                self.fail(error);
                false
            }
        }
    }
}
struct Request {
    handle: HostHandle,
    session: u64,
}
fn begin(mut state: Signal<AppState>, mutation: bool) -> Option<Request> {
    let mut current = state.write();
    let handle = current.handle()?;
    let session = current.begin_timer_request(mutation)?;
    Some(Request { handle, session })
}
async fn read_list(handle: &HostHandle, offset: u32) -> Result<(u32, TimerPage), aftercast::Error> {
    let page = handle.timer_definitions(offset, PAGE).await?;
    if offset > 0 && page.total > 0 && page.items.is_empty() {
        let offset = (page.total - 1) / PAGE * PAGE;
        return Ok((offset, handle.timer_definitions(offset, PAGE).await?));
    }
    Ok((offset, page))
}
fn load(mut state: Signal<AppState>, offset: u32) {
    let Some(request) = begin(state, false) else {
        return;
    };
    state.write().timers.loaded = true;
    spawn_forever(async move {
        let result = read_list(&request.handle, offset).await;
        if !state.write().finish_timer_request(request.session) {
            return;
        }
        match result {
            Ok((offset, page)) => state.write().timers.page(offset, page),
            Err(error) => state.write().fail(error),
        }
    });
}
enum Mutation {
    Save(TimerRevision, Option<String>, TimerDefinition),
    Delete(TimerRevision, String),
    Reload(TimerRevision),
}
struct Change {
    revision: TimerRevision,
    saved: Option<TimerItem>,
}
impl Mutation {
    async fn execute(&self, handle: &HostHandle) -> Result<Change, aftercast::Error> {
        match self {
            Self::Save(revision, key, definition) => handle
                .save_timer_definition(*revision, key.as_deref(), definition)
                .await
                .map(|saved| Change {
                    revision: saved.revision,
                    saved: Some(saved.item),
                }),
            Self::Delete(revision, key) => handle
                .delete_timer_definition(*revision, key)
                .await
                .map(|revision| Change {
                    revision,
                    saved: None,
                }),
            Self::Reload(revision) => {
                handle
                    .reload_timer_definitions(*revision)
                    .await
                    .map(|revision| Change {
                        revision,
                        saved: None,
                    })
            }
        }
    }
    fn message(&self) -> String {
        tr(match self {
            Self::Save(..) => "timer-saved",
            Self::Delete(..) => "timer-deleted",
            Self::Reload(..) => "timer-reloaded",
        })
    }
    fn apply(&self, editor: &mut EditorState, change: Change) {
        if let Some(item) = change.saved {
            editor.saved(SavedTimer {
                revision: change.revision,
                item,
            });
            return;
        }
        match self {
            Self::Delete(previous, key) => {
                if editor.key.as_ref() == Some(key) {
                    editor.new_draft();
                    editor.revision = Some(change.revision);
                } else if editor.revision == Some(*previous) {
                    editor.revision = Some(change.revision);
                }
                editor.deleting = None;
            }
            Self::Reload(_) => {
                editor.new_draft();
                editor.revision = Some(change.revision);
            }
            Self::Save(..) => {}
        }
    }
}
fn mutate(mut state: Signal<AppState>, mutation: Mutation) {
    let Some(request) = begin(state, true) else {
        return;
    };
    let message = mutation.message();
    spawn_forever(async move {
        let result = mutation.execute(&request.handle).await;
        if !state
            .write()
            .receive_timer_ack(request.session, &mutation, result, message)
        {
            return;
        }
        // Never retry an acknowledged write when only the following read fails.
        let offset = state.read().timers.offset;
        let result = read_list(&request.handle, offset).await;
        if state.read().revision.session != request.session {
            return;
        }
        match result {
            Ok((offset, page)) => state.write().timers.page(offset, page),
            Err(error) => state.write().fail(error),
        }
        state.write().finish_timer_request(request.session);
    });
}
fn ready(state: Signal<AppState>) -> bool {
    let current = state.read();
    current.phase == Phase::Ready && !current.timers.busy
}
fn save(mut state: Signal<AppState>) {
    if !ready(state) {
        return;
    }
    let result = {
        let current = state.read();
        let editor = &current.timers;
        if !editor.dirty {
            return;
        }
        let Some(revision) = editor.revision else {
            return;
        };
        editor
            .draft
            .compile()
            .map(|definition| Mutation::Save(revision, editor.key.clone(), definition))
    };
    match result {
        Ok(mutation) => mutate(state, mutation),
        Err(error) => state.write().fail(tr(error)),
    }
}
fn edit(mut state: Signal<AppState>, field: &str, value: String) {
    if !ready(state) {
        return;
    }
    let mut current = state.write();
    let editor = &mut current.timers;
    match field {
        "timer-name" => editor.draft.definition.name = value,
        "timer-category" => editor.draft.definition.category = value,
        "timer-duration" => editor.draft.duration = value,
        "timer-warning" => editor.draft.warning = value,
        "timer-removal" => editor.draft.removal = value,
        "timer-color" => editor.draft.color = value,
        "timer-tooltip" => editor.draft.definition.tooltip = value,
        "timer-start-cue" => editor.draft.definition.start_sound_data = value,
        "timer-warning-cue" => editor.draft.definition.warning_sound_data = value,
        _ => return,
    }
    editor.dirty = true;
    editor.deleting = None;
}
fn toggle(mut state: Signal<AppState>, field: &str) {
    if !ready(state) {
        return;
    }
    let mut current = state.write();
    let editor = &mut current.timers;
    let draft = &mut editor.draft.definition;
    let value = match field {
        "timer-enabled" => &mut draft.active_in_list,
        "timer-master" => &mut draft.only_master_ticks,
        "timer-category-restrict" => &mut draft.restrict_to_category,
        "timer-self" => &mut draft.restrict_to_me,
        "timer-absolute" => &mut draft.absolute_timing,
        "timer-modable" => &mut draft.modable,
        "timer-radial" => &mut draft.radial_display,
        "timer-panel1" => &mut draft.panel1_display,
        "timer-panel2" => &mut draft.panel2_display,
        _ => return,
    };
    *value = !*value;
    editor.dirty = true;
    editor.deleting = None;
}
fn text_fields(draft: &Draft) -> [(&'static str, String); 9] {
    [
        ("timer-name", draft.definition.name.clone()),
        ("timer-category", draft.definition.category.clone()),
        ("timer-duration", draft.duration.clone()),
        ("timer-warning", draft.warning.clone()),
        ("timer-removal", draft.removal.clone()),
        ("timer-color", draft.color.clone()),
        ("timer-tooltip", draft.definition.tooltip.clone()),
        ("timer-start-cue", draft.definition.start_sound_data.clone()),
        (
            "timer-warning-cue",
            draft.definition.warning_sound_data.clone(),
        ),
    ]
}
fn flags(draft: &TimerDefinition) -> [(&'static str, bool); 9] {
    [
        ("timer-enabled", draft.active_in_list),
        ("timer-master", draft.only_master_ticks),
        ("timer-category-restrict", draft.restrict_to_category),
        ("timer-self", draft.restrict_to_me),
        ("timer-absolute", draft.absolute_timing),
        ("timer-modable", draft.modable),
        ("timer-radial", draft.radial_display),
        ("timer-panel1", draft.panel1_display),
        ("timer-panel2", draft.panel2_display),
    ]
}
#[component]
pub(super) fn TimersPane() -> Element {
    let state = use_context::<Signal<AppState>>();
    use_effect(move || {
        if state.read().phase == Phase::Ready
            && !state.read().timers.loaded
            && !state.read().timers.busy
        {
            load(state, 0);
        }
    });
    rsx! {
        section { class: "space-y-6 p-6 min-[1000px]:p-8",
            h1 { class: "text-2xl font-semibold", {tr("timers")} }
            p { class: "max-w-3xl text-sm leading-7 text-muted-foreground", {tr("timer-help")} }
            if state.read().phase == Phase::Idle {
                p { {tr("timer-start-host")} }
            }
            if state.read().timers.busy {
                p { role: "status", {tr("timer-working")} }
            }
            TimerList {}
            snippet::Panel {}
            config_batch::Panel {}
            TimerForm {}
        }
    }
}
#[component]
fn TimerList() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let blocked = !ready(state);
    let dirty = state.read().timers.dirty;
    let page = state.read().timers.page.clone();
    let offset = state.read().timers.offset;
    let total = page.as_ref().map_or(0, |page| page.total);
    let revision = page.as_ref().map(|page| page.revision);
    let pending = state.read().timers.deleting.clone();
    rsx! {
        Accordion {
            default_open: true,
            class: "rounded-md border border-border".to_owned(),
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! {
                {tr("timer-definitions")}
            },
            section { class: "space-y-4 p-4", "aria-label": tr("timer-definitions"),
                div { class: "flex flex-wrap gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked,
                        onclick: move |_| load(state, offset),
                        {tr("timer-refresh")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked || dirty,
                        onclick: move |_| state.write().timers.new_draft(),
                        {tr("timer-new")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked || dirty || revision.is_none(),
                        onclick: move |_| {
                            if let Some(revision) = revision {
                                mutate(state, Mutation::Reload(revision));
                            }
                        },
                        {tr("timer-reload")}
                    }
                }
                if let Some(page) = page {
                    p { class: "text-sm text-muted-foreground",
                        {tr("timer-retained-frames")}
                        " {page.retained_frames}"
                    }
                    if page.items.is_empty() {
                        p { {tr("timer-empty")} }
                    }
                    for item in page.items {
                        {
                            let selected = state.read().timers.key.as_ref() == Some(&item.key);
                            let select_item = item.clone();
                            let delete_key = item.key.clone();
                            rsx! {
                                div {
                                    key: "{item.key}",
                                    class: "flex items-center justify-between gap-3 rounded-md border border-border p-3",
                                    Button {
                                        variant: ButtonVariant::Ghost,
                                        class: "min-w-0 justify-start truncate".to_owned(),
                                        aria_pressed: selected,
                                        disabled: blocked || dirty,
                                        onclick: move |_| {
                                            state.write().timers.select(select_item.clone(), page.revision);
                                        },
                                        "{item.definition.category} / {item.definition.name}"
                                    }
                                    Button {
                                        variant: ButtonVariant::Outline,
                                        disabled: blocked || dirty,
                                        onclick: move |_| state.write().timers.deleting = Some((delete_key.clone(), page.revision)),
                                        {tr("timer-delete")}
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some((key, revision)) = pending {
                    div {
                        class: "space-y-2 rounded-md border border-border p-3",
                        role: "group",
                        "aria-label": tr("timer-delete-confirm"),
                        p {
                            {tr("timer-delete-help")}
                            " {key}"
                        }
                        Button {
                            disabled: blocked || dirty,
                            onclick: move |_| mutate(state, Mutation::Delete(revision, key.clone())),
                            {tr("timer-delete-confirm")}
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: blocked,
                            onclick: move |_| state.write().timers.deleting = None,
                            {tr("timer-cancel")}
                        }
                    }
                }
                div { class: "flex gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked || offset == 0,
                        onclick: move |_| load(state, offset.saturating_sub(PAGE)),
                        {tr("previous")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked || offset + PAGE >= total,
                        onclick: move |_| load(state, offset + PAGE),
                        {tr("next")}
                    }
                    p { class: "self-center text-xs text-muted-foreground", "{offset} / {total}" }
                }
            }
        }
    }
}
fn conflict(state: Signal<AppState>) -> Option<(TimerRevision, Option<TimerItem>)> {
    let current = state.read();
    let editor = &current.timers;
    let page = editor.page.as_ref()?;
    let key = editor.key.as_ref()?;
    if editor.revision == Some(page.revision) {
        return None;
    }
    Some((
        page.revision,
        page.items.iter().find(|item| &item.key == key).cloned(),
    ))
}
#[component]
fn TimerForm() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let blocked = !ready(state);
    let draft = state.read().timers.draft.clone();
    let dirty = state.read().timers.dirty;
    let revision = state.read().timers.revision;
    let latest = conflict(state);
    let stale = latest.is_some();
    let save_blocked = blocked || !dirty || revision.is_none() || stale;
    rsx! {
        Accordion {
            default_open: true,
            class: "rounded-md border border-border".to_owned(),
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! {
                {tr("timer-editor")}
            },
            section { class: "space-y-4 p-4", "aria-label": tr("timer-editor"),
                for (id, value) in text_fields(&draft) {
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
                div { class: "flex flex-wrap gap-2",
                    for (field, pressed) in flags(&draft.definition) {
                        Button {
                            variant: ButtonVariant::Outline,
                            aria_pressed: pressed,
                            disabled: blocked,
                            onclick: move |_| toggle(state, field),
                            {tr(field)}
                        }
                    }
                }
                if let Some((revision, item)) = latest {
                    div { class: "space-y-3 rounded-md border border-border p-3",
                        p { class: "text-sm", {tr("timer-conflict-help")} }
                        if let Some(item) = item {
                            DefinitionSummary { definition: item.definition }
                            Button {
                                variant: ButtonVariant::Outline,
                                disabled: blocked || !dirty,
                                onclick: move |_| {
                                    state.write().timers.rebase(&item.key, revision);
                                },
                                {tr("timer-rebase")}
                            }
                        } else {
                            p { {tr("timer-conflict-hidden")} }
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: blocked || !dirty,
                            onclick: move |_| {
                                state.write().timers.keep_as_new(revision);
                            },
                            {tr("timer-keep-new")}
                        }
                    }
                }
                p { class: "text-xs leading-6 text-muted-foreground", {tr("timer-field-help")} }
                div { class: "flex gap-2",
                    Button { disabled: save_blocked, onclick: move |_| save(state), {tr("timer-save")} }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked,
                        onclick: move |_| state.write().timers.new_draft(),
                        {tr("timer-discard")}
                    }
                }
            }
        }
    }
}
#[component]
fn DefinitionSummary(definition: TimerDefinition) -> Element {
    rsx! {
        dl { class: "space-y-1 text-xs",
            for (label, value) in text_fields(&Draft::from(definition.clone())) {
                dt { class: "font-semibold", {tr(label)} }
                dd { class: "break-words whitespace-pre-wrap", "{value}" }
            }
            for (label, value) in flags(&definition) {
                dt { class: "font-semibold", {tr(label)} }
                dd { {tr(if value { "timer-yes" } else { "timer-no" })} }
            }
        }
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) async fn test_contract(
    host: &aftercast::Host,
    ui: &mut AppState,
    root: &camino::Utf8Path,
) {
    tests::contract(host, ui, root).await;
}
