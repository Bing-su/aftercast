use super::super::tr;
use super::*;
use crate::components::{Accordion, Badge, Button, ButtonVariant, Input, Label};
use dioxus::{core::spawn_forever, prelude::*};

fn launch(mut state: Signal<AppState>, view: Token, action: SourceAction) {
    let Some(handle) = state.read().handle() else {
        return;
    };
    let work = state.write().begin_source_work(view, action, handle);
    let work = match work {
        Ok(Some(work)) => work,
        Ok(None) => return,
        Err(key) => {
            state.write().fail(tr(key));
            return;
        }
    };
    spawn_forever(run_work(state, work));
}
async fn run_work(mut state: Signal<AppState>, work: Work) {
    let token = work.token;
    let action = work.action.clone();
    let selection = work.selection.clone();
    // Application ownership survives view unmount; never drop an admitted resource with its panel.
    let mut output = perform(work, move |progress| {
        if state.peek().history_source_view() == token {
            state.write().history_sources.progress = Some(progress);
        }
    })
    .await;
    if let Some(job) = output.job.take() {
        match super::super::wait_import(state, job, token.session).await {
            Ok(report) => {
                if state.peek().history_source_view() == token {
                    state.write().message = format!(
                        "{}: {} / {}",
                        tr("reparse-complete"),
                        report.records_dispatched,
                        report.lines_read
                    );
                    state.write().companions.excluded = report.companion_heads_excluded;
                }
            }
            Err(aftercast::Error::Cancelled) => output.canceled = true,
            Err(error) => output.error = Some(join_error(output.error, error.to_string())),
        }
        if let Err(error) = output.owned.release().await {
            output.error = Some(join_error(output.error, error));
        }
    }
    if state.peek().history_source_view() != token {
        output.canceled = true;
        if let Err(error) = output.owned.release().await {
            output.error = Some(join_error(output.error, error));
        }
    }
    let stale = state
        .write()
        .finish_source_work(token, &action, selection, output);
    if let Some(mut owned) = stale {
        let _cleanup = owned.release().await;
    }
    if matches!(action, SourceAction::Replay) && state.peek().revision.session == token.session {
        super::super::refresh(state, 0);
    }
}
pub(in crate::app) fn choose(state: Signal<AppState>, view: Token, selection: Selection) {
    launch(state, view, SourceAction::History(Box::new(selection)));
}
pub(in crate::app) fn leave(mut state: Signal<AppState>) {
    let release = state.write().leave_source_view();
    if release {
        let view = state.read().history_source_view();
        launch(state, view, SourceAction::Release);
    }
}
fn edit(mut state: Signal<AppState>, view: Token, field: &str, value: String) {
    if state.read().history_source_view() != view
        || !state.read().source_idle()
        || state.read().history_sources.holds_resources()
    {
        return;
    }
    let mut current = state.write();
    let draft = &mut current.history_sources.draft;
    match field {
        "path" => draft.path = value,
        "root" => draft.root = value,
        "filter" => draft.filter = value,
        "limit" => draft.limit = value,
        _ => {}
    }
}
fn text_edit(mut state: Signal<AppState>, view: Token, name: &str, field: &str, value: &str) {
    if state.read().history_source_view() != view
        || !state.read().source_idle()
        || state.read().history_sources.ready
    {
        return;
    }
    let mut current = state.write();
    let target = if name.is_empty() {
        Some(&mut current.history_sources.draft.text)
    } else {
        current
            .history_sources
            .choices
            .iter_mut()
            .find(|choice| choice.name == name)
            .map(|choice| &mut choice.text)
    };
    if let Some(target) = target
        && let Err(key) = target.change(field, value)
    {
        current.fail(tr(key));
    }
}
#[component]
pub(in crate::app) fn Panel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let view = current.history_source_view();
    let source = &current.history_sources;
    let idle = current.source_idle();
    let editable = idle && !source.holds_resources();
    let draft = source.draft.clone();
    let chosen = source.selection.clone();
    let candidates = source.snapshot.clone();
    let path = source.path.clone();
    let choices = source.choices.clone();
    let catalog = source.catalog.clone();
    let admitted = source.admitted.clone();
    let ready = source.ready;
    let pending = source.pending.is_some();
    let held = source.holds_resources();
    let error = source.error.clone();
    let progress = source.progress;
    let text = source.draft.text.clone();
    let files = source.files.clone();
    let parser = current
        .plugins
        .iter()
        .any(|plugin| plugin.info.state == aftercast::PluginState::Initialized);
    drop(current);
    rsx! {
        Accordion {
            default_open: true,
            class: "rounded-md border border-border".to_owned(),
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! {
                {tr("history-source-title")}
            },
            section {
                class: "space-y-4 p-4",
                "aria-label": tr("history-source-title"),
                p { class: "max-w-prose text-sm leading-7 text-muted-foreground",
                    {tr("history-source-help")}
                }
                if let Some(chosen) = chosen {
                    p { class: "text-sm font-medium", "{chosen.entry.title} · {chosen.entry.zone}" }
                    p { class: "text-xs text-muted-foreground",
                        {super::super::encounter_context::format_time(chosen.entry.start)}
                        " — "
                        {super::super::encounter_context::format_time(chosen.entry.end)}
                    }
                } else {
                    p { class: "text-sm text-muted-foreground",
                        {tr("history-source-select-history")}
                    }
                }
                for (field, label, value) in [
                    ("path", "history-source-path", draft.path),
                    ("root", "history-source-root", draft.root),
                    ("filter", "history-source-filter", draft.filter),
                    ("limit", "history-source-limit", draft.limit),
                ]
                {
                    div { class: "space-y-1",
                        Label { html_for: format!("history-source-{field}"), {tr(label)} }
                        Input {
                            id: format!("history-source-{field}"),
                            value,
                            disabled: !editable,
                            oninput: move |event: FormEvent| edit(state, view, field, event.value()),
                        }
                    }
                }
                p { class: "max-w-prose text-xs leading-6 text-muted-foreground",
                    {tr("history-source-budget-help")}
                }
                p { class: "max-w-prose text-xs leading-6 text-muted-foreground",
                    {tr("import-text-help")}
                }
                div { class: "flex flex-wrap gap-2",
                    Button {
                        disabled: !editable || state.read().history_sources.selection.is_none(),
                        onclick: move |_| launch(state, view, SourceAction::Discover),
                        {tr("history-source-discover")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: !idle || !held,
                        onclick: move |_| launch(state, view, SourceAction::Release),
                        {tr("history-source-release")}
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: !pending,
                        onclick: move |_| {
                            if state.read().history_source_view() == view {
                                state.write().cancel_source_work();
                            }
                        },
                        {tr("history-source-cancel")}
                    }
                }
                if pending {
                    p { role: "status", {tr("history-source-working")} }
                }
                if let Some(error) = error {
                    p {
                        role: "alert",
                        class: "break-words text-sm text-ctp-red",
                        "{error}"
                    }
                }
                if let Some(snapshot) = candidates {
                    p { class: "text-xs text-muted-foreground",
                        {tr("history-source-metadata-only")}
                    }
                    if snapshot.ambiguous {
                        p { class: "text-sm", {tr("history-source-ambiguous")} }
                    }
                    if snapshot.candidates.is_empty() {
                        p { {tr("history-source-empty")} }
                    }
                    for diagnostic in snapshot.diagnostics {
                        p { class: "break-words text-sm text-ctp-red",
                            "{diagnostic.path}: {diagnostic.message}"
                        }
                    }
                    div { class: "max-h-96 space-y-3 overflow-auto",
                        for candidate in snapshot.candidates {
                            div {
                                key: "{candidate.path}",
                                class: "space-y-1 border-b border-border pb-3",
                                p { class: "break-all text-sm", "{candidate.path}" }
                                p { class: "text-xs text-muted-foreground",
                                    {
                                        tr(
                                            match candidate.state {
                                                HistorySourceState::Available => "history-source-available",
                                                HistorySourceState::Missing => "history-source-missing",
                                                HistorySourceState::Unavailable => "history-source-unavailable",
                                            },
                                        )
                                    }
                                    " · "
                                    {
                                        tr(
                                            match candidate.change {
                                                aftercast::HistorySourceChange::New => "history-source-new",
                                                aftercast::HistorySourceChange::MetadataUnchanged => {
                                                    "history-source-unchanged"
                                                }
                                                aftercast::HistorySourceChange::MetadataChanged => {
                                                    "history-source-changed"
                                                }
                                                aftercast::HistorySourceChange::Replaced => "history-source-replaced",
                                                aftercast::HistorySourceChange::Moved => "history-source-moved",
                                            },
                                        )
                                    }
                                    " · "
                                    {
                                        candidate
                                            .length
                                            .map_or_else(|| tr("source-unavailable"), |bytes| format!("{bytes} bytes"))
                                    }
                                }
                                for alias in candidate.aliases {
                                    p { class: "break-all text-xs text-muted-foreground",
                                        "{alias}"
                                    }
                                }
                                if !candidate.message.is_empty() {
                                    p { class: "break-words text-xs text-ctp-red",
                                        "{candidate.message}"
                                    }
                                }
                                Button {
                                    variant: ButtonVariant::Outline,
                                    disabled: !idle || candidate.state != HistorySourceState::Available,
                                    aria_pressed: path.as_ref() == Some(&candidate.path),
                                    onclick: move |_| launch(state, view, SourceAction::Path(candidate.path.clone())),
                                    {tr("history-source-select-file")}
                                }
                            }
                        }
                    }
                }
                if let Some(path) = path {
                    p { class: "break-all text-sm font-medium", "{path}" }
                    TextOptions {
                        prefix: "history-main".to_owned(),
                        draft: text,
                        disabled: !idle || ready,
                        onchange: move |(field, value): (String, String)| text_edit(state, view, "", &field, &value),
                    }
                    if let Some(catalog) = catalog {
                        h3 { class: "text-sm font-semibold", {tr("sources-draft-title")} }
                        for candidate in catalog.candidates {
                            {
                                let selected = choices
                                    .iter()
                                    .find(|choice| choice.name == candidate.name)
                                    .cloned();
                                let name = candidate.name.clone();
                                let label = name.clone();
                                rsx! {
                                    div { class: "space-y-2 border-b border-border pb-3",
                                        p { class: "break-all text-xs text-muted-foreground", "{candidate.path}" }
                                        Button {
                                            variant: ButtonVariant::Outline,
                                            disabled: !idle || ready,
                                            aria_pressed: selected.is_some(),
                                            onclick: move |_| {
                                                if state.read().history_source_view() == view && state.read().source_idle()
                                                    && !state.read().history_sources.ready
                                                {
                                                    let mut current = state.write();
                                                    let choices = &mut current.history_sources.choices;
                                                    if let Some(index) = choices.iter().position(|choice| choice.name == name) {
                                                        choices.remove(index);
                                                    } else {
                                                        choices
                                                            .push(Choice {
                                                                name: name.clone(),
                                                                text: Default::default(),
                                                            });
                                                    }
                                                }
                                            },
                                            "{label}"
                                        }
                                        if let Some(selected) = selected {
                                            TextOptions {
                                                prefix: format!("history-side-{}", selected.name),
                                                draft: selected.text,
                                                disabled: !idle || ready,
                                                onchange: move |(field, value): (String, String)| text_edit(
                                                    state,
                                                    view,
                                                    &candidate.name,
                                                    &field,
                                                    &value,
                                                ),
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "flex flex-wrap gap-2",
                        Button {
                            disabled: !idle || ready,
                            onclick: move |_| launch(state, view, SourceAction::Capture),
                            {tr("history-source-capture")}
                        }
                        Button {
                            disabled: !idle || !ready || !parser,
                            onclick: move |_| launch(state, view, SourceAction::Replay),
                            {tr("history-source-reparse")}
                        }
                    }
                }
                if let Some(progress) = progress {
                    p { role: "status", class: "text-xs tabular-nums",
                        "{progress.bytes_copied} / {progress.total_bytes} bytes · {progress.files_copied} / {progress.files_total}"
                    }
                }
                if let Some(admitted) = admitted {
                    Badge { {tr(if ready { "history-source-ready" } else { "history-source-request" })} }
                    p { class: "break-all text-xs", "{admitted.path} · {admitted.bounds}" }
                    p { class: "text-xs", {format!("{} bytes", admitted.options.maximum_bytes)} }
                    super::super::companions::Codec { text: admitted.options.text, companion: false }
                    for source in admitted.options.companions {
                        p { class: "text-xs", "{source.name}" }
                        super::super::companions::Codec { text: source.text, companion: true }
                    }
                }
                for file in files {
                    p { class: "break-all text-xs tabular-nums",
                        "{file.path} · {file.length} bytes · SHA-256 {file.sha256}"
                    }
                }
                if ready {
                    p { class: "max-w-prose text-xs leading-6 text-muted-foreground",
                        {tr("history-source-image-help")}
                    }
                }
            }
        }
    }
}
#[component]
fn TextOptions(
    prefix: String,
    draft: import_text::Draft,
    disabled: bool,
    onchange: EventHandler<(String, String)>,
) -> Element {
    let encoding_id = format!("{prefix}-encoding");
    let policy_id = format!("{prefix}-policy");
    rsx! {
        div { class: "flex flex-wrap items-end gap-3",
            div { class: "space-y-1",
                Label { html_for: encoding_id.clone(), {tr("import-text-encoding")} }
                select {
                    id: encoding_id,
                    class: "h-9 max-w-full rounded-md border border-border bg-background px-2 text-sm",
                    value: draft.key(),
                    disabled,
                    onchange: move |event: FormEvent| onchange.call(("encoding".into(), event.value())),
                    option { value: "plugin", {tr("import-text-plugin")} }
                    for (key, _, label) in import_text::ENCODINGS {
                        option { value: key, "{label}" }
                    }
                }
            }
            if draft.encoding.is_some() {
                div { class: "space-y-1",
                    Label { html_for: policy_id.clone(), {tr("import-text-invalid")} }
                    select {
                        id: policy_id,
                        class: "h-9 max-w-full rounded-md border border-border bg-background px-2 text-sm",
                        disabled,
                        value: if draft.invalid_bytes == aftercast::ImportInvalidBytes::Strict { "strict" } else { "replace" },
                        onchange: move |event: FormEvent| onchange.call(("invalid".into(), event.value())),
                        option { value: "strict", {tr("import-text-strict")} }
                        option { value: "replace", {tr("import-text-replace")} }
                    }
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled,
                    aria_pressed: draft.detect_bom,
                    onclick: move |_| {
                        onchange
                            .call(("bom".into(), if draft.detect_bom { "false" } else { "true" }.into()))
                    },
                    {tr("import-text-bom")}
                }
            }
        }
    }
}
