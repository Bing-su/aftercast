use super::*;
use crate::components::{Accordion, Button, ButtonVariant, Input, Label};

#[component]
pub(super) fn Panel() -> Element {
    let state = use_context::<Signal<AppState>>();
    let current = state.read();
    let token = current.source_view();
    let blocked = !current.can_edit_sources();
    let ready = current.phase == Phase::Ready && !blocked;
    let empty_path = current.import_path.trim().is_empty();
    let candidates = current
        .companions
        .catalog
        .as_ref()
        .map(|catalog| catalog.candidates.clone());
    let choices = current.companions.choices.clone();
    let live = current.companions.live.clone();
    let active_import = current.source_import_snapshot();
    let error = current.companions.error.clone();
    let wait = current.companions.wait.clone();
    let zone = current.companions.zone;
    let pending = current.companions.pending.is_some();
    let excluded = current.companions.excluded;
    drop(current);
    rsx! {
        Accordion {
            class: "my-5 rounded-md border border-border".to_owned(),
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            default_open: true,
            header: rsx! {
                {tr("sources-title")}
            },
            section { class: "space-y-4 p-4", "aria-label": tr("sources-title"),
                p { class: "max-w-prose text-sm leading-7 text-muted-foreground",
                    {tr("sources-help")}
                }
                h3 { class: "text-sm font-semibold", {tr("sources-draft-title")} }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: !ready || empty_path,
                    onclick: move |_| catalog(state, token),
                    {tr("sources-catalog")}
                }
                if pending {
                    p { role: "status", class: "text-sm", {tr("sources-pending")} }
                }
                if let Some(error) = error {
                    p {
                        role: "alert",
                        class: "break-words text-sm text-ctp-red",
                        "{error}"
                    }
                }
                if let Some(candidates) = candidates {
                    if candidates.is_empty() {
                        p { class: "text-sm text-muted-foreground", {tr("sources-empty")} }
                    }
                    div { class: "max-h-96 space-y-3 overflow-auto",
                        for candidate in candidates {
                            CandidateRow {
                                key: "{candidate.name}",
                                name: candidate.name.clone(),
                                path: candidate.path.to_string(),
                                draft: choices
                                    .iter()
                                    .find(|choice| choice.name == candidate.name)
                                    .map(|choice| choice.text.clone()),
                                token,
                                blocked,
                            }
                        }
                    }
                }
                p { class: "text-xs leading-6 text-muted-foreground",
                    {tr("sources-selection-help")}
                }
                p { class: "text-xs tabular-nums",
                    {tr("sources-excluded")}
                    " {excluded}"
                }
                div { class: "border-t border-border pt-4 space-y-3",
                    h3 { class: "text-sm font-semibold", {tr("sources-live-title")} }
                    p { class: "max-w-prose text-xs leading-6 text-muted-foreground",
                        {tr("sources-live-help")}
                    }
                    Label { html_for: "source-clock-wait", {tr("sources-clock")} }
                    Input {
                        id: "source-clock-wait".to_owned(),
                        value: wait,
                        placeholder: "30000".to_owned(),
                        disabled: blocked,
                        oninput: move |event: FormEvent| edit(state, token, "", "wait", &event.value()),
                    }
                    div { class: "flex flex-wrap gap-3",
                        Button {
                            variant: ButtonVariant::Outline,
                            aria_pressed: zone,
                            disabled: blocked,
                            onclick: move |_| edit(state, token, "", "zone", if zone { "false" } else { "true" }),
                            {tr("sources-zone")}
                        }
                        Button {
                            disabled: !ready || empty_path,
                            onclick: move |_| open(state, token),
                            {tr(if live.is_some() { "sources-reopen" } else { "sources-open" })}
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: !ready || live.is_none(),
                            onclick: move |_| close(state, token),
                            {tr("sources-close")}
                        }
                    }
                }
                if let Some(snapshot) = active_import {
                    ImportSnapshot { snapshot }
                }
                if let Some(snapshot) = live {
                    LiveSnapshot { snapshot }
                }

            }
        }
    }
}
#[component]
fn CandidateRow(
    name: String,
    path: String,
    draft: Option<import_text::Draft>,
    token: Token,
    blocked: bool,
) -> Element {
    let state = use_context::<Signal<AppState>>();
    // Catalog names may contain spaces; encode field IDs rather than copying filenames into HTML IDs.
    let field_id = name
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let selected = draft.is_some();
    let draft = draft.unwrap_or_default();
    let implicit = draft.encoding.is_none();
    rsx! {
        div { class: "space-y-2 border-t border-border pt-3",
            Button {
                variant: ButtonVariant::Outline,
                aria_pressed: selected,
                disabled: blocked,
                onclick: {
                    let name = name.clone();
                    move |_| edit(state, token, &name, "selected", "")
                },
                "{name} · "
                {tr(if selected { "sources-selected" } else { "sources-unselected" })}
            }
            p { class: "break-words text-xs text-muted-foreground", "{path}" }
            if selected {
                div { class: "flex flex-wrap items-end gap-3",
                    Label { html_for: format!("source-encoding-{field_id}"),
                        {tr("import-text-encoding")}
                    }
                    select {
                        id: format!("source-encoding-{field_id}"),
                        class: "h-9 max-w-full rounded-md border border-border bg-background px-2 text-sm",
                        value: draft.key(),
                        disabled: blocked,
                        onchange: {
                            let name = name.clone();
                            move |event: FormEvent| edit(state, token, &name, "encoding", &event.value())
                        },
                        option { value: "plugin", {tr("sources-inherit")} }
                        for (key, _, label) in import_text::ENCODINGS {
                            option { value: key, "{label}" }
                        }
                    }
                    if !implicit {
                        Label { html_for: format!("source-invalid-{field_id}"),
                            {tr("import-text-invalid")}
                        }
                        select {
                            id: format!("source-invalid-{field_id}"),
                            class: "h-9 max-w-full rounded-md border border-border bg-background px-2 text-sm",
                            value: if draft.invalid_bytes == aftercast::ImportInvalidBytes::Strict { "strict" } else { "replace" },
                            disabled: blocked,
                            onchange: {
                                let name = name.clone();
                                move |event: FormEvent| edit(state, token, &name, "invalid", &event.value())
                            },
                            option { value: "strict", {tr("import-text-strict")} }
                            option { value: "replace", {tr("import-text-replace")} }
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: blocked,
                            aria_pressed: draft.detect_bom,
                            onclick: {
                                let name = name.clone();
                                move |_| edit(
                                    state,
                                    token,
                                    &name,
                                    "bom",
                                    if draft.detect_bom { "false" } else { "true" },
                                )
                            },
                            {tr("import-text-bom")}
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Codec(text: Option<ImportTextOptions>, companion: bool) -> Element {
    let label = text.map(|text| {
        import_text::ENCODINGS
            .iter()
            .find(|(_, encoding, _)| *encoding == text.encoding)
            .map_or("?", |(_, _, label)| *label)
    });
    rsx! {
        p { class: "text-xs leading-6 text-muted-foreground",
            if let (Some(text), Some(label)) = (text, label) {
                "{label} · "
                {
                    tr(
                        if text.invalid_bytes == aftercast::ImportInvalidBytes::Strict {
                            "import-text-strict"
                        } else {
                            "import-text-replace"
                        },
                    )
                }
                " · "
                {tr(if text.detect_bom { "sources-bom-enabled" } else { "sources-bom-disabled" })}
            } else {
                {tr(if companion { "sources-active-inherit" } else { "sources-active-plugin" })}
            }
        }
    }
}
#[component]
fn ImportSnapshot(snapshot: ActiveImport) -> Element {
    rsx! {
        section {
            class: "space-y-2 border-t border-border pt-4",
            "aria-label": tr("sources-import-snapshot"),
            h3 { class: "text-sm font-semibold", {tr("sources-import-snapshot")} }
            p { class: "break-words text-xs", "{snapshot.path}" }
            if let Some(range) = snapshot.range {
                p { class: "text-xs tabular-nums",
                    {tr("sources-range-ticks")}
                    " {range.start_ticks()} — {range.end_ticks()}"
                }
            }
            p { class: "text-xs font-semibold", {tr("sources-main")} }
            Codec { text: snapshot.text, companion: false }
            for source in snapshot.sources {
                p { class: "text-xs font-semibold", "{source.name}" }
                Codec { text: source.text, companion: true }
            }
        }
    }
}
#[component]
fn LiveSnapshot(snapshot: LiveLogSnapshot) -> Element {
    rsx! {
        section {
            class: "space-y-2 border-t border-border pt-4",
            "aria-label": tr("sources-live-snapshot"),
            h3 { class: "text-sm font-semibold", {tr("sources-live-snapshot")} }
            p { role: "status", class: "break-words text-xs",
                "{snapshot.path} · {snapshot.options.max_clock_wait.as_millis()} ms · "
                {tr("sources-zone")}
                " · "
                {tr(if snapshot.options.get_current_zone { "enabled" } else { "disabled" })}
            }
            for source in snapshot.sources {
                div { class: "space-y-1 border-t border-border py-3 text-xs",
                    p { class: "font-semibold",
                        if source.name.is_empty() {
                            {tr("sources-main")}
                        } else {
                            "{source.selected_name} / {source.name}"
                        }
                    }
                    p { class: "break-words text-muted-foreground", "{source.path}" }
                    Codec {
                        text: if source.name.is_empty() { snapshot.options.text } else { snapshot
                            .options
                            .companions
                            .iter()
                            .find(|selected| selected.name == source.selected_name)
                            .and_then(|selected| selected.text) },
                        companion: !source.name.is_empty(),
                    }
                    p { class: "tabular-nums",
                        {
                            tr(
                                match source.state {
                                    aftercast::LiveLogState::Following => "sources-following",
                                    aftercast::LiveLogState::Paused => "sources-paused",
                                    aftercast::LiveLogState::WaitingForClock => "sources-waiting",
                                    aftercast::LiveLogState::Changing => "sources-changing",
                                    aftercast::LiveLogState::Failed => "sources-failed",
                                    aftercast::LiveLogState::Stopped => "sources-stopped",
                                },
                            )
                        }
                        " · "
                        {tr("sources-generation")}
                        " {source.generation} · "
                        {tr("sources-dropped")}
                        " {source.dropped} · "
                        {tr("sources-queued")}
                        " {source.queued}"
                    }
                    if let Some(ticks) = source.pending_ticks {
                        p { class: "tabular-nums",
                            {tr("sources-clock-head")}
                            " {ticks}"
                        }
                    }
                    if !source.last_error.is_empty() {
                        p { role: "status", class: "break-words text-ctp-red",
                            "{source.last_error}"
                        }
                    }
                    if source.error_truncated {
                        p { class: "text-muted-foreground", {tr("sources-truncated")} }
                    }
                }
            }
        }
    }
}
