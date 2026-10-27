use super::*;
use crate::components::{Button, ButtonVariant, Label};
fn edit(mut state: Signal<AppState>, view: Token, field: &str, value: &str) {
    let result = state.write().edit_import_text(view, field, value);
    if let Err(key) = result {
        state.write().fail(tr(key));
    }
}
#[component]
pub(super) fn Controls() -> Element {
    let state = use_context::<Signal<AppState>>();
    let current = state.read();
    let view = current.text_view();
    let draft = current.import_text.draft.clone();
    let blocked = !current.can_edit_import_text();
    let implicit = draft.encoding.is_none();
    let pending = current
        .import_text
        .full
        .as_ref()
        .is_some_and(|request| request.path.is_none());
    drop(current);
    rsx! {
        section {
            class: "my-5 space-y-3 rounded-md border border-border p-4",
            "aria-label": tr("import-text-title"),
            h2 { class: "text-sm font-semibold", {tr("import-text-title")} }
            p { class: "text-sm leading-6 text-muted-foreground", {tr("import-text-help")} }
            div { class: "flex flex-wrap items-end gap-4",
                div { class: "space-y-1",
                    Label { html_for: "import-encoding", {tr("import-text-encoding")} }
                    select {
                        id: "import-encoding",
                        class: "h-9 max-w-full rounded-md border border-border bg-background px-2 text-sm",
                        value: draft.key(),
                        disabled: blocked,
                        onchange: move |event: FormEvent| edit(state, view, "encoding", &event.value()),
                        option { value: "plugin", {tr("import-text-plugin")} }
                        for (key, _, label) in ENCODINGS {
                            option { value: key, "{label}" }
                        }
                    }
                }
                if !implicit {
                    div { class: "space-y-1",
                        Label { html_for: "import-byte-policy", {tr("import-text-invalid")} }
                        select {
                            id: "import-byte-policy",
                            class: "h-9 max-w-full rounded-md border border-border bg-background px-2 text-sm",
                            value: if draft.invalid_bytes == ImportInvalidBytes::Strict { "strict" } else { "replace" },
                            disabled: blocked || implicit,
                            onchange: move |event: FormEvent| edit(state, view, "invalid", &event.value()),
                            option { value: "strict", {tr("import-text-strict")} }
                            option { value: "replace", {tr("import-text-replace")} }
                        }
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked || implicit,
                        aria_pressed: draft.detect_bom,
                        onclick: move |_| edit(state, view, "bom", if draft.detect_bom { "false" } else { "true" }),
                        {tr("import-text-bom")}
                    }
                }
            }
            if implicit {
                p { class: "text-xs leading-6 text-muted-foreground",
                    {tr("import-text-plugin-help")}
                }
            } else {
                p { class: "text-xs leading-6 text-muted-foreground", {tr("import-text-bom-help")} }
                if draft.invalid_bytes == ImportInvalidBytes::Replace {
                    p { class: "text-xs leading-6 text-muted-foreground",
                        {tr("import-text-replace-help")}
                    }
                }
            }
            p { class: "text-xs leading-6 text-muted-foreground", {tr("import-text-session-help")} }
            if pending {
                p { role: "status", class: "text-sm", {tr("import-text-picking")} }
            }
        }
    }
}
