use super::{AppState, Phase, tr};
use crate::components::{Accordion, Button, ButtonVariant, Input, Label};
use aftercast::{
    HostHandle, TriggerDefinition, TriggerItem, TriggerPage, TriggerResultCursor,
    TriggerResultPage, TriggerRevision,
};
use dioxus::{core::spawn_forever, prelude::*};
const PAGE: u32 = 16;

#[derive(Default)]
pub(super) struct EditorState {
    loaded: bool,
    results_loaded: bool,
    busy: bool,
    pub(super) page: Option<TriggerPage>,
    offset: u32,
    draft: TriggerDefinition,
    key: Option<String>,
    revision: Option<TriggerRevision>,
    dirty: bool,
    deleting: Option<(String, TriggerRevision)>,
    results: Option<TriggerResultPage>,
    cursor: Option<TriggerResultCursor>,
    previous: Vec<Option<TriggerResultCursor>>,
}
impl EditorState {
    fn select(&mut self, item: TriggerItem, revision: TriggerRevision) -> bool {
        if self.dirty {
            return false;
        }
        self.key = Some(item.key);
        self.draft = item.definition;
        self.revision = Some(revision);
        self.dirty = false;
        self.deleting = None;
        true
    }
    fn new_draft(&mut self) {
        self.key = None;
        self.draft = TriggerDefinition::default();
        self.dirty = false;
        self.deleting = None;
        self.revision = self.page.as_ref().map(|page| page.revision);
    }
    fn page(&mut self, offset: u32, page: TriggerPage) {
        // Creating cannot overwrite an existing key: refreshing can safely rebase a new draft.
        if self.key.is_none() || self.revision.is_none() && !self.dirty {
            self.revision = Some(page.revision);
        }
        self.offset = offset;
        self.page = Some(page);
    }
    fn reviewed_rebase(&mut self, key: &str, revision: TriggerRevision) -> bool {
        let reviewed = self.page.as_ref().is_some_and(|page| {
            page.revision == revision && page.items.iter().any(|item| item.key == key)
        });
        if !self.dirty || self.key.as_deref() != Some(key) || !reviewed {
            return false;
        }
        self.revision = Some(revision);
        true
    }
    fn keep_as_new(&mut self, revision: TriggerRevision) -> bool {
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
    fn remember_cursor(&mut self) {
        if self.previous.len() == 256 {
            self.previous.remove(0);
        }
        self.previous.push(self.cursor);
    }
    fn accept_results(
        &mut self,
        movement: ResultMove,
        cursor: Option<TriggerResultCursor>,
        page: TriggerResultPage,
    ) {
        match movement {
            ResultMove::First => self.previous.clear(),
            ResultMove::Next => self.remember_cursor(),
            ResultMove::Previous => {
                self.previous.pop();
            }
        }
        self.cursor = cursor;
        self.results = Some(page);
    }
    fn reset_results(&mut self) {
        self.results_loaded = false;
        self.results = None;
        self.cursor = None;
        self.previous.clear();
    }
}

#[component]
pub(super) fn TriggersPane() -> Element {
    let state = use_context::<Signal<AppState>>();
    use_effect(move || {
        let ready = {
            let current = state.read();
            current.phase == Phase::Ready && !current.triggers.busy
        };
        if !ready {
            return;
        }
        if !state.peek().triggers.loaded {
            load_list(state, 0);
        } else if !state.peek().triggers.results_loaded {
            load_results(state, ResultMove::First);
        }
    });
    rsx! {
        section { class: "space-y-6 p-6 min-[1000px]:p-8",
            h1 { class: "text-2xl font-semibold", {tr("triggers")} }
            p { class: "max-w-3xl text-sm leading-7 text-muted-foreground",
                {tr("trigger-editor-help")}
            }
            if state.read().phase == Phase::Idle {
                p { {tr("trigger-start-host")} }
            }
            if state.read().triggers.busy {
                p { role: "status", {tr("trigger-working")} }
            }
            TriggerList {}
            if let Some(detail) = state.read().detail.clone() {
                if let Some(page) = state.read().triggers.page.clone() {
                    super::trigger_benchmark::BenchmarkPanel {
                        key: "benchmark-{detail.summary.id:?}-{page.revision:?}",
                        id: detail.summary.id,
                    }
                }
            } else {
                p { "Select an encounter in Analysis to benchmark its retained logs." }
            }
            TriggerForm {}
            TriggerResults {}
        }
    }
}

#[component]
fn TriggerList() -> Element {
    let state = use_context::<Signal<AppState>>();
    let blocked = !controls_ready(state);
    let page = state.read().triggers.page.clone();
    let offset = state.read().triggers.offset;
    let dirty = state.read().triggers.dirty;
    let reload_revision = page.as_ref().map(|page| page.revision);
    let total = page.as_ref().map_or(0, |page| page.total);
    rsx! {
        Accordion {
            default_open: true,
            class: "rounded-md border border-border".to_owned(),
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! {
                {tr("trigger-definitions")}
            },
            section {
                class: "space-y-3 p-4",
                "aria-label": tr("trigger-definitions"),
                div { class: "flex flex-wrap gap-2",
                    Button { disabled: blocked, onclick: move |_| new_draft(state),
                        {tr("trigger-new")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked,
                        onclick: move |_| load_list(state, offset),
                        {tr("refresh")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked || dirty || page.is_none(),
                        onclick: move |_| {
                            if let Some(revision) = reload_revision {
                                reload(state, revision);
                            }
                        },
                        {tr("trigger-reload")}
                    }
                }
                if let Some(page) = page {
                    div { class: "flex flex-wrap gap-2",
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: blocked,
                            aria_pressed: page.original_lines,
                            onclick: move |_| options(state, page.revision, !page.original_lines, page.process_imports),
                            {tr("trigger-original-lines")}
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: blocked,
                            aria_pressed: page.process_imports,
                            onclick: move |_| options(state, page.revision, page.original_lines, !page.process_imports),
                            {tr("trigger-imports")}
                        }
                    }
                    if page.items.is_empty() {
                        p { {tr("trigger-empty")} }
                    }
                    ul {
                        id: "trigger-definitions-list",
                        class: "divide-y divide-border",
                        for item in page.items {
                            li { class: "flex items-center gap-3 py-3",
                                Button {
                                    variant: ButtonVariant::Ghost,
                                    class: "h-auto min-w-0 flex-1 flex-col items-start whitespace-normal text-left".to_owned(),
                                    disabled: blocked || dirty,
                                    onclick: {
                                        let item = item.clone();
                                        move |_| select(state, item.clone(), page.revision)
                                    },
                                    strong { "{item.definition.category}" }
                                    code { class: "break-all", "{item.definition.regex}" }
                                    span { class: "text-xs text-muted-foreground",
                                        {tr(if item.definition.active { "enabled" } else { "disabled" })}
                                    }
                                }
                                Button {
                                    variant: ButtonVariant::Outline,
                                    disabled: blocked,
                                    onclick: move |_| confirm_delete(state, item.key.clone(), page.revision),
                                    {tr("remove")}
                                }
                            }
                        }
                    }
                }
                if let Some((key, revision)) = state.read().triggers.deleting.clone() {
                    div { class: "space-y-2 rounded-md border border-border p-3",
                        p { class: "break-all",
                            {tr("trigger-delete-confirm")}
                            " {key}"
                        }
                        Button {
                            disabled: blocked,
                            onclick: {
                                let key = key.clone();
                                move |_| mutate(state, Mutation::Delete(revision, key.clone()))
                            },
                            {tr("trigger-delete")}
                        }
                        Button {
                            variant: ButtonVariant::Ghost,
                            disabled: blocked,
                            onclick: move |_| cancel_delete(state),
                            {tr("cancel")}
                        }
                    }
                }
                div { class: "flex items-center gap-3 text-xs text-muted-foreground",
                    Button {
                        variant: ButtonVariant::Ghost,
                        disabled: blocked || offset == 0,
                        onclick: move |_| load_list(state, offset.saturating_sub(PAGE)),
                        {tr("previous")}
                    }
                    span {
                        "{offset.saturating_add(1).min(total)}–{offset.saturating_add(PAGE).min(total)} / {total}"
                    }
                    Button {
                        variant: ButtonVariant::Ghost,
                        disabled: blocked || offset.saturating_add(PAGE) >= total,
                        onclick: move |_| load_list(state, offset + PAGE),
                        {tr("next")}
                    }
                }
            }
        }
    }
}

#[component]
fn TriggerForm() -> Element {
    let state = use_context::<Signal<AppState>>();
    let draft = state.read().triggers.draft.clone();
    let blocked = !controls_ready(state) || state.read().triggers.page.is_none();
    let save_blocked =
        blocked || !state.read().triggers.dirty || state.read().triggers.revision.is_none();
    rsx! {
        Accordion {
            default_open: true,
            class: "rounded-md border border-border".to_owned(),
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! {
                {tr("trigger-edit")}
            },
            section { class: "space-y-4 p-4", "aria-label": tr("trigger-edit"),
                for (id, label, value) in [
                    ("trigger-category", "trigger-category-label", draft.category.clone()),
                    ("trigger-regex", "trigger-regex-label", draft.regex.clone()),
                    ("trigger-sound", "trigger-sound-label", draft.sound_data.clone()),
                    ("trigger-timer-name", "trigger-timer-label", draft.timer_name.clone()),
                ]
                {
                    div { class: "space-y-1",
                        Label { html_for: id.to_owned(), {tr(label)} }
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
                    "aria-label": tr("trigger-sound-type"),
                    for (sound, label) in [(0, "trigger-none"), (1, "trigger-beep"), (2, "trigger-wav"), (3, "trigger-tts")] {
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: blocked,
                            aria_pressed: draft.sound_type == sound,
                            onclick: move |_| sound_type(state, sound),
                            {tr(label)}
                        }
                    }
                }
                div { class: "flex flex-wrap gap-2",
                    for (field, label, pressed) in [
                        ("active", "trigger-active", draft.active),
                        ("restrict", "trigger-restrict", draft.restrict_to_category_zone),
                        ("timer", "trigger-timer", draft.timer),
                        ("tabbed", "trigger-tabbed", draft.tabbed),
                    ]
                    {
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: blocked,
                            aria_pressed: pressed,
                            onclick: move |_| toggle(state, field),
                            {tr(label)}
                        }
                    }
                }
                if let Some((revision, latest)) = conflict(state) {
                    div { class: "space-y-3 rounded-md border border-border p-3",
                        p { class: "text-sm", {tr("trigger-conflict-help")} }
                        if let Some(item) = latest {
                            DefinitionSummary { definition: item.definition }
                            Button {
                                variant: ButtonVariant::Outline,
                                disabled: blocked,
                                onclick: move |_| rebase(state, &item.key, revision),
                                {tr("trigger-rebase")}
                            }
                        } else {
                            p { class: "text-sm text-muted-foreground",
                                {tr("trigger-conflict-not-visible")}
                            }
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: blocked,
                            onclick: move |_| keep_as_new(state, revision),
                            {tr("trigger-keep-as-new")}
                        }
                    }
                }
                p { class: "text-xs text-muted-foreground", {tr("trigger-draft-help")} }
                div { class: "flex gap-2",
                    Button {
                        id: "save-trigger",
                        disabled: save_blocked,
                        onclick: move |_| save(state),
                        {tr("trigger-save")}
                    }
                    Button {
                        variant: ButtonVariant::Ghost,
                        disabled: blocked,
                        onclick: move |_| new_draft(state),
                        {tr("trigger-discard")}
                    }
                }
            }
        }
    }
}

#[component]
fn TriggerResults() -> Element {
    let state = use_context::<Signal<AppState>>();
    let blocked = !controls_ready(state);
    let results = state.read().triggers.results.clone();
    let can_back = !state.read().triggers.previous.is_empty();
    let can_next = results.as_ref().is_some_and(|page| {
        page.items
            .last()
            .is_some_and(|item| item.sequence < page.latest_sequence)
    });
    rsx! {
        Accordion {
            default_open: true,
            class: "rounded-md border border-border".to_owned(),
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! {
                {tr("trigger-results")}
            },
            section { class: "space-y-3 p-4", "aria-label": tr("trigger-results"),
                div { class: "flex flex-wrap gap-2",
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked,
                        onclick: move |_| load_results(state, ResultMove::First),
                        {tr("trigger-refresh-results")}
                    }
                    Button {
                        variant: ButtonVariant::Ghost,
                        disabled: blocked,
                        onclick: move |_| mutate(state, Mutation::Clear),
                        {tr("trigger-clear-results")}
                    }
                    Button {
                        variant: ButtonVariant::Ghost,
                        disabled: blocked || !can_back,
                        onclick: move |_| load_results(state, ResultMove::Previous),
                        {tr("previous")}
                    }
                    Button {
                        variant: ButtonVariant::Ghost,
                        disabled: blocked || !can_next,
                        onclick: move |_| load_results(state, ResultMove::Next),
                        {tr("next")}
                    }
                }
                if let Some(page) = results {
                    p {
                        role: "status",
                        class: "text-xs text-muted-foreground",
                        {tr("trigger-result-summary")}
                        " {page.latest_sequence} · "
                        {tr("omitted-logs")}
                        " {page.omitted}"
                    }
                    if page.items.is_empty() {
                        p { {tr("trigger-no-results")} }
                    }
                    ol { class: "divide-y divide-border",
                        for item in page.items {
                            li { class: "space-y-2 py-3",
                                p { class: "text-sm font-semibold break-all",
                                    "{item.category} / {item.tab_name}"
                                }
                                p { class: "text-xs text-muted-foreground tabular-nums",
                                    "#{item.sequence} · "
                                    {result_time(item.time_ticks)}
                                    " · #{item.time_sorter} · "
                                    {tr(if item.is_import { "trigger-import-result" } else { "trigger-live-result" })}
                                }
                                pre { class: "whitespace-pre-wrap break-all text-sm",
                                    "{item.line}"
                                }
                                dl { class: "text-xs text-muted-foreground",
                                    for capture in item.captures {
                                        dt { "{capture.name}" }
                                        dd { class: "ml-3 whitespace-pre-wrap break-all",
                                            "{capture.value}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
fn result_time(ticks: u64) -> String {
    super::encounter_context::format_time(aftercast::LogTimestamp {
        ticks,
        utc_ticks: None,
        kind: aftercast::LogTimeKind::Unspecified,
    })
}
fn controls_ready(state: Signal<AppState>) -> bool {
    let current = state.read();
    current.phase == Phase::Ready && !current.triggers.busy
}
fn edit(mut state: Signal<AppState>, field: &str, value: String) {
    if !controls_ready(state) {
        return;
    }
    let mut current = state.write();
    let editor = &mut current.triggers;
    match field {
        "trigger-category" => editor.draft.category = value,
        "trigger-regex" => editor.draft.regex = value,
        "trigger-sound" => editor.draft.sound_data = value,
        "trigger-timer-name" => editor.draft.timer_name = value,
        _ => return,
    }
    editor.dirty = true;
}
fn sound_type(mut state: Signal<AppState>, value: i32) {
    if controls_ready(state) {
        let mut current = state.write();
        current.triggers.draft.sound_type = value;
        current.triggers.dirty = true;
    }
}
fn toggle(mut state: Signal<AppState>, field: &str) {
    if !controls_ready(state) {
        return;
    }
    let mut current = state.write();
    let editor = &mut current.triggers;
    match field {
        "active" => editor.draft.active = !editor.draft.active,
        "restrict" => {
            editor.draft.restrict_to_category_zone = !editor.draft.restrict_to_category_zone
        }
        "timer" => editor.draft.timer = !editor.draft.timer,
        "tabbed" => editor.draft.tabbed = !editor.draft.tabbed,
        _ => return,
    }
    editor.dirty = true;
}
struct Request {
    handle: HostHandle,
    session: u64,
}
impl AppState {
    fn begin_trigger_request(&mut self, mutation: bool) -> Option<u64> {
        if self.phase != Phase::Ready || self.triggers.busy {
            return None;
        }
        self.triggers.busy = true;
        if mutation {
            self.phase = Phase::UpdatingTriggers;
            self.error = None;
        }
        Some(self.revision.session)
    }
    fn finish_trigger_request(&mut self, session: u64) -> bool {
        if self.revision.session != session {
            return false;
        }
        self.triggers.busy = false;
        if self.phase == Phase::UpdatingTriggers {
            self.phase = Phase::Ready;
        }
        true
    }
    fn receive_trigger_ack(
        &mut self,
        session: u64,
        mutation: &Mutation,
        result: Result<Option<TriggerRevision>, aftercast::Error>,
        message: String,
    ) -> bool {
        if self.revision.session != session {
            return false;
        }
        match result {
            Ok(revision) => {
                mutation.apply(&mut self.triggers, revision);
                self.message = message;
                true
            }
            Err(error) => {
                self.finish_trigger_request(session);
                self.fail(error);
                false
            }
        }
    }
}
fn begin(mut state: Signal<AppState>, mutation: bool) -> Option<Request> {
    let mut current = state.write();
    let handle = current.handle()?;
    let session = current.begin_trigger_request(mutation)?;
    Some(Request { handle, session })
}
fn finish(mut state: Signal<AppState>, session: u64) -> bool {
    state.write().finish_trigger_request(session)
}
async fn read_list(
    handle: &HostHandle,
    offset: u32,
) -> Result<(u32, TriggerPage), aftercast::Error> {
    let page = handle.triggers(offset, PAGE).await?;
    if page.items.is_empty() && offset > 0 && page.total > 0 {
        let offset = (page.total - 1) / PAGE * PAGE;
        return Ok((offset, handle.triggers(offset, PAGE).await?));
    }
    Ok((offset, page))
}
fn load_list(mut state: Signal<AppState>, offset: u32) {
    let Some(request) = begin(state, false) else {
        return;
    };
    state.write().triggers.loaded = true;
    spawn_forever(async move {
        let result = read_list(&request.handle, offset).await;
        if !finish(state, request.session) {
            return;
        }
        match result {
            Ok((offset, page)) => state.write().triggers.page(offset, page),
            Err(error) => state.write().fail(error),
        }
    });
}
#[derive(Clone, Copy)]
enum ResultMove {
    First,
    Next,
    Previous,
}
fn load_results(mut state: Signal<AppState>, movement: ResultMove) {
    let cursor = {
        let current = state.read();
        let editor = &current.triggers;
        match movement {
            ResultMove::First => None,
            ResultMove::Next => editor.results.as_ref().map(|page| page.next_cursor),
            ResultMove::Previous => editor.previous.last().copied().flatten(),
        }
    };
    let Some(request) = begin(state, false) else {
        return;
    };
    state.write().triggers.results_loaded = true;
    spawn_forever(async move {
        let result = request.handle.trigger_results(cursor, PAGE).await;
        if !finish(state, request.session) {
            return;
        }
        match result {
            Ok(page) => state
                .write()
                .triggers
                .accept_results(movement, cursor, page),
            Err(error) => state.write().fail(error),
        }
    });
}

enum Mutation {
    Save(TriggerRevision, Option<String>, TriggerDefinition),
    Delete(TriggerRevision, String),
    Options(TriggerRevision, bool, bool),
    Reload(TriggerRevision),
    Clear,
}
impl Mutation {
    fn message(&self) -> String {
        tr(match self {
            Self::Save(..) => "trigger-saved",
            Self::Delete(..) => "trigger-deleted",
            Self::Options(..) => "trigger-options-saved",
            Self::Reload(..) => "trigger-reloaded",
            Self::Clear => "trigger-results-cleared",
        })
    }
    async fn execute(
        &self,
        handle: &HostHandle,
    ) -> Result<Option<TriggerRevision>, aftercast::Error> {
        match self {
            Self::Save(revision, key, definition) => handle
                .save_trigger(*revision, key.as_deref(), definition)
                .await
                .map(Some),
            Self::Delete(revision, key) => handle.delete_trigger(*revision, key).await.map(Some),
            Self::Options(revision, original, imports) => handle
                .save_trigger_options(*revision, *original, *imports)
                .await
                .map(Some),
            Self::Reload(revision) => handle.reload_triggers(*revision).await.map(Some),
            Self::Clear => handle.clear_trigger_results().await.map(|()| None),
        }
    }
    fn apply(&self, editor: &mut EditorState, revision: Option<TriggerRevision>) {
        match self {
            Self::Save(_, _, definition) => {
                editor.draft = definition.clone();
                editor.key = Some(format!("{}|{}", definition.category, definition.regex));
                editor.dirty = false;
                editor.revision = revision;
                editor.deleting = None;
            }
            Self::Delete(previous, key) => {
                if editor.key.as_ref() == Some(key) {
                    editor.new_draft();
                    editor.revision = revision;
                } else if editor.revision == Some(*previous) {
                    editor.revision = revision;
                }
                editor.deleting = None;
            }
            Self::Options(previous, ..) => {
                if editor.revision == Some(*previous) {
                    editor.revision = revision;
                }
            }
            Self::Reload(_) => {
                editor.new_draft();
                editor.revision = revision;
            }
            Self::Clear => {}
        }
        if !matches!(self, Self::Options(..)) {
            editor.reset_results();
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
        // Acknowledged mutations stay acknowledged even if the subsequent refresh fails.
        if !state
            .write()
            .receive_trigger_ack(request.session, &mutation, result, message)
        {
            return;
        }
        if !matches!(mutation, Mutation::Clear) {
            let offset = state.read().triggers.offset;
            let refreshed = read_list(&request.handle, offset).await;
            if state.read().revision.session != request.session {
                return;
            }
            match refreshed {
                Ok((offset, page)) => state.write().triggers.page(offset, page),
                Err(error) => state.write().fail(error),
            }
        }
        finish(state, request.session);
    });
}
fn save(state: Signal<AppState>) {
    let mutation = {
        let current = state.read();
        let editor = &current.triggers;
        let Some(revision) = editor.revision else {
            return;
        };
        if !editor.dirty {
            return;
        }
        let mut draft = editor.draft.clone();
        if draft.category.is_empty() {
            draft.category = " General".into();
        }
        Mutation::Save(revision, editor.key.clone(), draft)
    };
    mutate(state, mutation);
}
fn options(state: Signal<AppState>, revision: TriggerRevision, original: bool, imports: bool) {
    mutate(state, Mutation::Options(revision, original, imports));
}
fn reload(state: Signal<AppState>, revision: TriggerRevision) {
    if !state.read().triggers.dirty {
        mutate(state, Mutation::Reload(revision));
    }
}
fn new_draft(mut state: Signal<AppState>) {
    if controls_ready(state) {
        state.write().triggers.new_draft();
    }
}
fn select(mut state: Signal<AppState>, item: TriggerItem, revision: TriggerRevision) {
    if controls_ready(state) {
        state.write().triggers.select(item, revision);
    }
}
fn confirm_delete(mut state: Signal<AppState>, key: String, revision: TriggerRevision) {
    if controls_ready(state) {
        state.write().triggers.deleting = Some((key, revision));
    }
}
fn cancel_delete(mut state: Signal<AppState>) {
    if controls_ready(state) {
        state.write().triggers.deleting = None;
    }
}

fn conflict(state: Signal<AppState>) -> Option<(TriggerRevision, Option<TriggerItem>)> {
    let current = state.read();
    let editor = &current.triggers;
    let page = editor.page.as_ref()?;
    let key = editor.key.as_ref()?;
    if !editor.dirty || editor.revision == Some(page.revision) {
        return None;
    }
    Some((
        page.revision,
        page.items.iter().find(|item| &item.key == key).cloned(),
    ))
}
fn rebase(mut state: Signal<AppState>, key: &str, revision: TriggerRevision) {
    if controls_ready(state) {
        state.write().triggers.reviewed_rebase(key, revision);
    }
}
fn keep_as_new(mut state: Signal<AppState>, revision: TriggerRevision) {
    if controls_ready(state) {
        state.write().triggers.keep_as_new(revision);
    }
}
#[component]
fn DefinitionSummary(definition: TriggerDefinition) -> Element {
    let sound = match definition.sound_type {
        0 => "trigger-none",
        1 => "trigger-beep",
        2 => "trigger-wav",
        3 => "trigger-tts",
        _ => "trigger-unknown-sound",
    };
    rsx! {
        h2 { class: "font-semibold", {tr("trigger-latest-saved")} }
        dl { class: "space-y-2 text-xs",
            for (label, value) in [
                ("trigger-category-label", definition.category),
                ("trigger-regex-label", definition.regex),
                ("trigger-sound-type", tr(sound)),
                ("trigger-sound-label", definition.sound_data),
                ("trigger-timer-label", definition.timer_name),
                ("trigger-active", tr(if definition.active { "enabled" } else { "disabled" })),
                (
                    "trigger-restrict",
                    tr(if definition.restrict_to_category_zone { "enabled" } else { "disabled" }),
                ),
                ("trigger-timer", tr(if definition.timer { "enabled" } else { "disabled" })),
                ("trigger-tabbed", tr(if definition.tabbed { "enabled" } else { "disabled" })),
            ]
            {
                dt { class: "font-medium", {tr(label)} }
                dd { class: "whitespace-pre-wrap break-all text-muted-foreground",
                    "{value}"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
