use super::{
    AppState,
    log_window::{ContextEndpoint, WindowAppearance, guard},
};
use crate::components::{Accordion, Button, ButtonVariant};
use aftercast::{EncounterId, TriggerBenchmarkResult, TriggerPage};
use dioxus::prelude::*;

#[derive(Default)]
pub(super) struct State {
    selected: Vec<String>,
    version: u64,
    busy: bool,
    result: Option<TriggerBenchmarkResult>,
    error: Option<String>,
}
impl State {
    pub(super) fn begin(&mut self) -> u64 {
        self.version = self.version.wrapping_add(1);
        self.busy = true;
        self.result = None;
        self.error = None;
        self.version
    }
    pub(super) fn cancel(&mut self) {
        self.version = self.version.wrapping_add(1);
        self.busy = false;
    }
    pub(super) fn current(&self, version: u64) -> bool {
        self.busy && self.version == version
    }
}
pub(super) async fn run(
    endpoint: &ContextEndpoint,
    revision: aftercast::TriggerRevision,
    keys: &[String],
) -> Result<TriggerBenchmarkResult, String> {
    guard(endpoint, async {
        let snapshot = endpoint
            .handle
            .capture_log_snapshot(endpoint.id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("Encounter is unavailable")?;
        endpoint
            .handle
            .benchmark_triggers(revision, snapshot, keys)
            .await
            .map_err(|e| e.to_string())
    })
    .await
}
#[component]
pub(super) fn BenchmarkPanel(id: EncounterId) -> Element {
    let app = use_context::<Signal<AppState>>();
    let Some(handle) = app.read().handle() else {
        return rsx! {};
    };
    let endpoint = ContextEndpoint {
        handle,
        id,
        generation: app.read().log_windows.generation(),
        lifetime: app.read().log_windows.clone(),
        appearance: WindowAppearance {
            title: "Trigger benchmark".into(),
            theme: app.read().settings.ui.theme.clone(),
            language: app.read().settings.ui.language.clone(),
        },
    };
    let page: Option<TriggerPage> = app.read().triggers.page.clone();
    let mut state = use_signal(State::default);
    let life = endpoint.clone();
    use_future(move || {
        let endpoint = life.clone();
        async move {
            while endpoint.valid() {
                dioxus_sdk::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            let mut value = state.write();
            value.cancel();
            value.selected.clear();
            value.result = None;
            value.error = Some("Session ended.".into());
        }
    });
    let busy = state.read().busy;
    let selected = state.read().selected.clone();
    let result = state.read().result.clone();
    let error = state.read().error.clone();
    rsx! {
        Accordion {
            class: "rounded-md border border-border".to_owned(),
            default_open: false,
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! { "Trigger benchmark" },
            div { class: "p-4 space-y-3",
                p {
                    "Measure selected expressions against a captured encounter with more than 1,000 retained log entries. No trigger alerts, timers or results are executed."
                }
                p {
                    "Select up to 32 triggers from the current trigger page. Changing pages preserves selection while the catalog revision remains the same."
                }
                if let Some(page) = page {
                    for item in &page.items {
                        {
                            let key = item.key.clone();
                            let checked = selected.contains(&key);
                            rsx! {
                                Button {
                                    variant: ButtonVariant::Outline,
                                    disabled: busy || (!checked && selected.len() >= 32),
                                    aria_pressed: checked,
                                    onclick: move |_| {
                                        let mut value = state.write();
                                        if let Some(index) = value.selected.iter().position(|item| item == &key) {
                                            value.selected.remove(index);
                                        } else {
                                            value.selected.push(key.clone());
                                        }
                                        value.result = None;
                                    },
                                    "{item.definition.category} / {item.definition.regex}"
                                }
                            }
                        }
                    }
                    {
                        let endpoint = endpoint.clone();
                        let revision = page.revision;
                        let keys = selected.clone();
                        rsx! {
                            Button {
                                disabled: busy || keys.is_empty(),
                                onclick: move |_| {
                                    let endpoint = endpoint.clone();
                                    let keys = keys.clone();
                                    let version = state.write().begin();
                                    spawn(async move {
                                        let answer = run(&endpoint, revision, &keys).await;
                                        if !endpoint.valid() || !state.peek().current(version) {
                                            return;
                                        }
                                        let mut value = state.write();
                                        value.busy = false;
                                        match answer {
                                            Ok(result) => value.result = Some(result),
                                            Err(error) => value.error = Some(error),
                                        }
                                    });
                                },
                                "Perform benchmark"
                            }
                        }
                    }
                } else {
                    p { "Load trigger definitions first." }
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: !busy,
                    onclick: move |_| state.write().cancel(),
                    "Cancel"
                }
                if busy {
                    p { role: "status",
                        "Measuring… Cancellation discards the result; a running match finishes or times out."
                    }
                }
                if let Some(error) = error {
                    p { role: "alert", "{error}" }
                }
                if let Some(result) = result {
                    p {
                        "{result.lines} retained lines, {result.dropped} omitted; timestamp length {result.timestamp_length}. Baseline: {result.baseline_milliseconds} ms."
                    }
                    if result.dropped > 0 {
                        p { "The retained log is incomplete." }
                    }
                    for row in result.rows {
                        {
                            let ratio = if result.baseline_milliseconds == 0 {
                                "baseline below 1 ms; ratio unavailable".to_owned()
                            } else {
                                format!(
                                    "{:.1}× baseline",
                                    row.milliseconds as f64 / result.baseline_milliseconds as f64,
                                )
                            };
                            rsx! {
                                p {
                                    "{row.category} / {row.key}: {row.milliseconds} ms ({ratio}), {row.matches} matches, restricted: {row.restricted}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
