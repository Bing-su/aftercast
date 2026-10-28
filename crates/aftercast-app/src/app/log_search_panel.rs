use super::log_window::{ContextEndpoint, guard};
use crate::components::{Accordion, Button, ButtonVariant, Input};
use aftercast::{
    ActionSnapshot, EncounterLog,
    log_search::{self, CopyScope, LogEntry},
};
use dioxus::prelude::*;
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct Endpoint(pub ContextEndpoint);
impl PartialEq for Endpoint {
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id
            && self.0.generation == other.0.generation
            && Arc::ptr_eq(&self.0.lifetime, &other.0.lifetime)
    }
}
#[derive(Clone)]
pub(super) struct Frozen {
    pub(super) snapshot: ActionSnapshot,
    pub(super) rows: Vec<EncounterLog>,
    dropped: u64,
}
#[derive(Default)]
pub(super) struct State {
    version: u64,
    busy: bool,
    frozen: Option<Arc<Frozen>>,
    matches: Vec<u32>,
    cursor: Option<u32>,
    selected: Vec<u32>,
    only: bool,
    filter: Option<i32>,
    page: usize,
    error: Option<String>,
    copy: Option<String>,
}
impl State {
    pub(super) fn start(&mut self) -> u64 {
        self.version = self.version.wrapping_add(1);
        self.busy = true;
        self.error = None;
        self.version
    }
    fn reset_view(&mut self) {
        self.frozen = None;
        self.matches.clear();
        self.selected.clear();
        self.cursor = None;
        self.copy = None;
        self.filter = None;
        self.only = false;
        self.page = 0;
    }
    fn begin_capture(&mut self) -> u64 {
        self.reset_view();
        self.start()
    }
    fn invalidate(&mut self) {
        self.cancel();
        self.reset_view();
        self.error = Some("Session ended.".into());
    }
    pub(super) fn cancel(&mut self) {
        self.version = self.version.wrapping_add(1);
        self.busy = false;
    }
    pub(super) fn current(&self, version: u64) -> bool {
        self.busy && self.version == version
    }
}
fn append_rows(
    rows: &mut Vec<EncounterLog>,
    bytes: &mut usize,
    items: Vec<EncounterLog>,
) -> Result<(), String> {
    for row in items {
        *bytes = bytes.saturating_add(row.text.len());
        if *bytes > 16 * 1024 * 1024 {
            return Err("Snapshot text exceeds 16 MiB.".into());
        }
        rows.push(row);
    }
    Ok(())
}
pub(super) async fn capture(endpoint: &ContextEndpoint) -> Result<Option<Frozen>, String> {
    guard(endpoint, async {
        let Some(snapshot) = endpoint
            .handle
            .capture_log_snapshot(endpoint.id)
            .await
            .map_err(|e| e.to_string())?
        else {
            return Ok(None);
        };
        let mut rows = Vec::new();
        let mut offset = 0;
        let mut total = None;
        let mut bytes = 0usize;
        let dropped = loop {
            let page = endpoint
                .handle
                .action_snapshot_logs(snapshot, offset, 256)
                .await
                .map_err(|e| e.to_string())?;
            if total.is_some_and(|value| value != page.page.total) {
                return Err("Snapshot log count changed.".into());
            }
            total = Some(page.page.total);
            if page.page.items.is_empty() && offset < page.page.total {
                return Err("Snapshot log page ended early.".into());
            }
            append_rows(&mut rows, &mut bytes, page.page.items)?;
            offset = u32::try_from(rows.len()).map_err(|e| e.to_string())?;
            if offset >= page.page.total {
                break page.dropped;
            }
        };
        Ok(Some(Frozen {
            snapshot,
            rows,
            dropped,
        }))
    })
    .await
}
fn start_capture(mut state: Signal<State>, endpoint: ContextEndpoint) {
    let version = {
        let mut value = state.write();
        value.begin_capture()
    };
    spawn(async move {
        let result = capture(&endpoint).await;
        if !endpoint.valid() || !state.peek().current(version) {
            return;
        }
        let mut value = state.write();
        value.busy = false;
        match result {
            Ok(Some(data)) => value.frozen = Some(Arc::new(data)),
            Ok(None) => value.error = Some("Encounter is unavailable.".into()),
            Err(error) => value.error = Some(error),
        }
    });
}
fn start_search(
    mut state: Signal<State>,
    endpoint: ContextEndpoint,
    pattern: String,
    regex: bool,
    filter: Option<i32>,
) {
    let (version, frozen) = {
        let mut value = state.write();
        let Some(frozen) = value.frozen.clone() else {
            return;
        };
        (value.start(), frozen)
    };
    spawn(async move {
        let result = guard(&endpoint, async {
            endpoint
                .handle
                .search_snapshot_logs(frozen.snapshot, &pattern, regex, filter)
                .await
                .map_err(|e| e.to_string())
        })
        .await;
        if !endpoint.valid() || !state.peek().current(version) {
            return;
        }
        let mut value = state.write();
        value.busy = false;
        match result {
            Ok(indices)
                if indices
                    .iter()
                    .all(|index| (*index as usize) < frozen.rows.len()) =>
            {
                value.matches = indices;
                value.cursor = None;
                value.only = false;
                value.filter = filter;
                value.page = 0;
                value.selected.clear();
                value.copy = None;
            }
            Ok(_) => value.error = Some("Invalid search result index.".into()),
            Err(error) => value.error = Some(error),
        }
    });
}
#[component]
pub(super) fn SearchPanel(endpoint: Endpoint) -> Element {
    let mut state = use_signal(State::default);
    let lifecycle = endpoint.0.clone();
    use_future(move || {
        let endpoint = lifecycle.clone();
        async move {
            while endpoint.valid() {
                dioxus_sdk::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            state.write().invalidate();
        }
    });
    let mut pattern = use_signal(String::new);
    let mut type_text = use_signal(String::new);
    let current = state.read();
    let busy = current.busy;
    let frozen = current.frozen.clone();
    let matches = current.matches.clone();
    let selected = current.selected.clone();
    let only = current.only;
    let filter = current.filter;
    let page = current.page;
    let cursor = current.cursor;
    let error = current.error.clone();
    let copy = current.copy.clone();
    drop(current);
    let mut visible: Vec<u32> = frozen
        .as_ref()
        .map(|data| {
            data.rows
                .iter()
                .enumerate()
                .filter(|(index, row)| {
                    filter.is_none_or(|kind| kind == row.log_type)
                        && (!only || matches.binary_search(&(*index as u32)).is_ok())
                })
                .map(|(index, _)| index as u32)
                .collect()
        })
        .unwrap_or_default();
    let total = visible.len();
    let start = page.saturating_mul(50);
    visible = visible.into_iter().skip(start).take(50).collect();
    let source = endpoint.0.clone();
    rsx! {
        Accordion {
            class: "mt-5 border border-border rounded-md".to_owned(),
            default_open: false,
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! { "Search encounter logs" },
            div { class: "space-y-3 p-4",
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: busy,
                    onclick: move |_| start_capture(state, source.clone()),
                    "Capture logs"
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: !busy,
                    onclick: move |_| state.write().cancel(),
                    "Cancel"
                }
                if busy {
                    p { role: "status", "Loading or searching…" }
                }
                if let Some(error) = error {
                    p { role: "alert", "{error}" }
                }
                if let Some(data) = frozen {
                    p {
                        "{data.rows.len()} retained; {data.dropped} omitted. Partial imports may be incomplete."
                    }
                    Input {
                        id: "log-search-pattern".to_owned(),
                        value: pattern(),
                        placeholder: "Search text or .NET regex".to_owned(),
                        oninput: move |event: FormEvent| pattern.set(event.value()),
                    }
                    Input {
                        id: "log-search-type".to_owned(),
                        value: type_text(),
                        placeholder: "Optional log type number".to_owned(),
                        oninput: move |event: FormEvent| type_text.set(event.value()),
                    }
                    for (regex, label) in [(false, "Search text"), (true, "Search regex")] {
                        {
                            let endpoint = endpoint.0.clone();
                            rsx! {
                                Button {
                                    variant: ButtonVariant::Outline,
                                    disabled: busy,
                                    onclick: move |_| {
                                        let text = type_text();
                                        let filter = if text.trim().is_empty() {
                                            None
                                        } else {
                                            match text.parse::<i32>() {
                                                Ok(value) => Some(value),
                                                Err(_) => {
                                                    state.write().error = Some("Invalid log type.".into());
                                                    return;
                                                }
                                            }
                                        };
                                        start_search(state, endpoint.clone(), pattern(), regex, filter);
                                    },
                                    "{label}"
                                }
                            }
                        }
                    }
                    p { "{matches.len()} matches; {total} visible" }
                    Button {
                        variant: ButtonVariant::Outline,
                        aria_pressed: only,
                        onclick: move |_| {
                            let mut value = state.write();
                            value.only = !value.only;
                            value.page = 0;
                        },
                        "Results only"
                    }
                    for (forward, label) in [(false, "Previous match"), (true, "Next match")] {
                        {
                            let data = data.clone();
                            rsx! {
                                Button {
                                    variant: ButtonVariant::Outline,
                                    onclick: move |_| {
                                        let mut value = state.write();
                                        if let Some(index) = log_search::adjacent_match(
                                            &value.matches,
                                            value.cursor,
                                            forward,
                                        ) {
                                            value.cursor = Some(index);
                                            let position = data
                                                .rows
                                                .iter()
                                                .enumerate()
                                                .filter(|(idx, row)| {
                                                    value.filter.is_none_or(|kind| kind == row.log_type)
                                                        && (!value.only
                                                            || value.matches.binary_search(&(*idx as u32)).is_ok())
                                                })
                                                .position(|(idx, _)| idx as u32 == index);
                                            if let Some(position) = position {
                                                value.page = position / 50;
                                            }
                                        }
                                    },
                                    "{label}"
                                }
                            }
                        }
                    }
                    div { class: "space-y-2",
                        for index in visible {
                            if let Some(row) = data.rows.get(index as usize) {
                                {
                                    let text = row.text.clone();
                                    let active = cursor == Some(index);
                                    let marked = matches.binary_search(&index).is_ok();
                                    let checked = selected.contains(&index);
                                    rsx! {
                                        Button {
                                            variant: ButtonVariant::Outline,
                                            aria_pressed: checked,
                                            class: if active || marked { "bg-accent" } else { "" },
                                            onclick: move |_| {
                                                let mut value = state.write();
                                                if let Some(pos) = value.selected.iter().position(|entry| *entry == index) {
                                                    value.selected.remove(pos);
                                                } else {
                                                    value.selected.push(index);
                                                }
                                                value.cursor = Some(index);
                                            },
                                            "#{index} · {text}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: page == 0,
                        onclick: move |_| state.write().page = page.saturating_sub(1),
                        "Previous page"
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: start.saturating_add(50) >= total,
                        onclick: move |_| state.write().page = page.saturating_add(1),
                        "Next page"
                    }
                    for (scope, label) in [
                        (CopyScope::All, "Prepare all text"),
                        (CopyScope::Matches, "Prepare result text"),
                        (CopyScope::Selected, "Prepare selected text"),
                        (CopyScope::Combat, "Prepare combat text"),
                    ]
                    {
                        {
                            let data = data.clone();
                            let matches = matches.clone();
                            let selected = selected.clone();
                            rsx! {
                                Button {
                                    variant: ButtonVariant::Outline,
                                    onclick: move |_| {
                                        let rows = data
                                            .rows
                                            .iter()
                                            .enumerate()
                                            .map(|(index, row)| LogEntry {
                                                index: index as u32,
                                                kind: row.log_type,
                                                text: row.text.clone(),
                                            })
                                            .collect::<Vec<_>>();
                                        let mut value = state.write();
                                        match log_search::copy_text(
                                            &rows,
                                            scope,
                                            &matches,
                                            &selected,
                                            if cfg!(windows) { "\r\n" } else { "\n" },
                                        ) {
                                            Ok(text) => value.copy = Some(text),
                                            Err(error) => value.error = Some(error.into()),
                                        }
                                    },
                                    "{label}"
                                }
                            }
                        }
                    }
                    if let Some(copy) = copy {
                        {
                            let preview: String = copy.chars().take(16_384).collect();
                            let bytes = copy.len();
                            rsx! {
                                p { "Prepared {bytes} bytes; preview shows at most 16,384 characters." }
                                pre { class: "max-h-48 overflow-auto whitespace-pre-wrap", "{preview}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    #[test]
    fn new_capture_and_session_invalidation_clear_view_and_reject_pending_results() {
        let mut state = State {
            only: true,
            filter: Some(42),
            page: 3,
            matches: vec![7],
            selected: vec![7],
            cursor: Some(7),
            copy: Some("old".into()),
            ..State::default()
        };
        let first = state.begin_capture();
        assert!(
            !state.only
                && state.filter.is_none()
                && state.page == 0
                && state.matches.is_empty()
                && state.selected.is_empty()
                && state.cursor.is_none()
                && state.copy.is_none()
        );
        assert!(state.current(first));
        state.invalidate();
        assert!(!state.current(first) && state.error.as_deref() == Some("Session ended."));
    }
}
