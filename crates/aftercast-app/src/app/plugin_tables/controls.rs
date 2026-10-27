use super::*;
use crate::components::{Accordion, Badge, Button, ButtonVariant};
pub(super) fn toggle(preferences: &mut Preferences, key: &str) {
    if let Some(index) = preferences.visible.iter().position(|value| value == key) {
        preferences.visible.remove(index);
    } else {
        preferences.visible.push(key.into());
    }
}
pub(super) fn move_key(preferences: &mut Preferences, key: &str, up: bool) {
    let Some(index) = preferences.order.iter().position(|value| value == key) else {
        return;
    };
    let destination = if up {
        index.checked_sub(1)
    } else {
        index
            .checked_add(1)
            .filter(|next| *next < preferences.order.len())
    };
    if let Some(destination) = destination {
        preferences.order.swap(index, destination);
    }
}
fn context(scope: &TableScope) -> String {
    match scope {
        TableScope::Encounters => tr("table-all-encounters"),
        TableScope::Combatants(_) => tr("table-retained-encounter"),
        TableScope::Categories { combatant, .. } => combatant.clone(),
        TableScope::Attacks {
            combatant,
            category,
            ..
        } => format!("{combatant} / {category}"),
        TableScope::Actions {
            combatant,
            category,
            attack,
            ..
        } => format!("{combatant} / {category} / {attack}"),
    }
}
#[component]
fn Choice(
    view: View,
    column: aftercast::TableColumn,
    checked: bool,
    first: bool,
    last: bool,
    disabled: bool,
) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let toggle_key = column.key.clone();
    let up_key = column.key.clone();
    let down_key = column.key.clone();
    rsx! {
        div { class: "flex flex-wrap items-center gap-2 border-b border-border py-2 text-sm",
            Button {
                variant: ButtonVariant::Outline,
                disabled,
                aria_pressed: checked,
                onclick: move |_| {
                    state.write().edit_table(view, |draft| toggle(draft, &toggle_key));
                },
                {tr(if checked { "table-visible" } else { "table-hidden" })}
            }
            span { class: "min-w-0 flex-1 break-words", "{column.label} [{column.key}]" }
            if !column.can_read {
                Badge { {tr("table-unreadable")} }
            }
            Button {
                variant: ButtonVariant::Ghost,
                disabled: disabled || first,
                aria_label: Some(format!("{} {}", tr("table-up"), column.key)),
                onclick: move |_| {
                    state.write().edit_table(view, |draft| move_key(draft, &up_key, true));
                },
                {tr("table-up")}
            }
            Button {
                variant: ButtonVariant::Ghost,
                disabled: disabled || last,
                aria_label: Some(format!("{} {}", tr("table-down"), column.key)),
                onclick: move |_| {
                    state.write().edit_table(view, |draft| move_key(draft, &down_key, false));
                },
                {tr("table-down")}
            }
        }
    }
}
#[component]
fn SortControls(view: View, catalog: TableCatalog, draft: Preferences, disabled: bool) -> Element {
    let mut state = use_context::<Signal<AppState>>();
    rsx! {
        div { class: "flex flex-wrap items-center gap-3 py-3",
            for (secondary, id, label) in [
                (false, "plugin-primary", "table-primary"),
                (true, "plugin-secondary", "table-secondary"),
            ]
            {
                div { class: "space-y-1",
                    crate::components::Label { html_for: id.to_owned(), {tr(label)} }
                    select {
                        id,
                        class: "h-9 max-w-full rounded-md border border-border bg-background px-2 text-sm",
                        disabled: disabled || secondary && draft.primary.is_none(),
                        value: if secondary { draft.secondary.clone().unwrap_or_default() } else { draft.primary.clone().unwrap_or_default() },
                        onchange: move |event: FormEvent| {
                            let key = event.value();
                            state
                                .write()
                                .edit_table(
                                    view,
                                    |draft| {
                                        let key = (!key.is_empty()).then_some(key);
                                        if secondary {
                                            draft.secondary = key;
                                        } else {
                                            draft.primary = key;
                                            if draft.primary.is_none() {
                                                draft.secondary = None;
                                            }
                                        }
                                    },
                                );
                        },
                        option { value: "", {tr("unsorted")} }
                        for column in catalog.columns.iter().filter(|column| column.can_sort) {
                            option { value: column.key.clone(), "{column.label} [{column.key}]" }
                        }
                    }
                }
            }
            Button {
                variant: ButtonVariant::Outline,
                disabled,
                aria_pressed: draft.descending,
                onclick: move |_| {
                    state.write().edit_table(view, |draft| draft.descending = !draft.descending);
                },
                {tr(if draft.descending { "sort-descending" } else { "sort-ascending" })}
            }
        }
    }
}
#[component]
pub(super) fn Panel() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let view = view(&current);
    let active_scope = scope(&current);
    let frame = current
        .plugin_tables
        .frame
        .as_ref()
        .filter(|frame| target_matches(&current, &frame.token))
        .cloned();
    let editor = current
        .plugin_tables
        .editors
        .get(level_key(active_scope.level()))
        .cloned();
    let blocked = !current_view(&current, view);
    let idle = idle(&current);
    let error = current.plugin_tables.error.clone();
    let save_error = current.plugin_tables.save_error.clone();
    let message = current.plugin_tables.message.clone();
    let busy = current.plugin_tables.pending.is_some();
    let dirty = editor
        .as_ref()
        .is_some_and(|editor| editor.draft != editor.applied);
    let file_available = current.file.is_some();
    let history = !current.plugin_tables.history.is_empty();
    drop(current);
    rsx! {
        section { class: "my-5 space-y-4", "aria-label": tr("table-plugin-title"),
            div { class: "flex flex-wrap items-center gap-3",
                h2 { class: "text-sm font-semibold",
                    {tr("table-plugin-title")}
                    " · "
                    {tr(&format!("table-level-{}", level_key(active_scope.level())))}
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: !idle,
                    onclick: move |_| {
                        if view == super::view(&state.read()) {
                            load(state, Mode::Refresh, 0);
                        }
                    },
                    {tr("table-refresh")}
                }
                Button {
                    variant: ButtonVariant::Ghost,
                    disabled: blocked || !history,
                    onclick: move |_| {
                        state.write().navigate_table(view, None);
                    },
                    {tr("table-back")}
                }
            }
            p { class: "break-words text-xs text-muted-foreground", {context(&active_scope)} }
            if busy {
                p { role: "status", {tr("reading-tables")} }
            }
            if let Some(error) = error {
                p { role: "alert", class: "text-sm text-destructive", "{error}" }
            }
            if let Some(error) = save_error {
                p { role: "alert", class: "text-sm text-destructive",
                    {tr("table-save-failed")}
                    " {error}"
                }
            }
            if !message.is_empty() {
                p { role: "status", "{message}" }
            }
            if let (Some(frame), Some(editor)) = (frame, editor) {
                for notice in &frame.notices {
                    p { class: "text-xs text-muted-foreground",
                        {tr(notice.kind)}
                        ": "
                        {notice.keys.join(", ")}
                    }
                }
                if frame.catalog.columns.is_empty() {
                    p { class: "text-sm text-muted-foreground", {tr("table-no-registrations")} }
                }
                Accordion {
                    default_open: false,
                    class: "rounded-md border border-border".to_owned(),
                    trigger_class: "px-4 py-3 font-semibold".to_owned(),
                    header: rsx! {
                        {tr("table-configure")}
                    },
                    div { class: "space-y-3 p-4",
                        p { class: "text-xs leading-6 text-muted-foreground",
                            {tr("table-settings-help")}
                        }
                        for (index, key) in editor.draft.order.iter().enumerate() {
                            if let Some(column) = frame.catalog.columns.iter().find(|column| &column.key == key) {
                                Choice {
                                    key: "{key}",
                                    view,
                                    column: column.clone(),
                                    checked: editor.draft.visible.contains(key),
                                    first: index == 0,
                                    last: index + 1 == editor.draft.order.len(),
                                    disabled: blocked,
                                }
                            }
                        }
                        SortControls {
                            view,
                            catalog: frame.catalog.clone(),
                            draft: editor.draft.clone(),
                            disabled: blocked,
                        }
                        div { class: "flex flex-wrap gap-2",
                            Button {
                                disabled: blocked || !dirty || editor.draft.validate().is_err(),
                                onclick: move |_| {
                                    if current_view(&state.read(), view) {
                                        load(state, Mode::Apply, 0);
                                    }
                                },
                                {tr("table-apply")}
                            }
                            Button {
                                variant: ButtonVariant::Outline,
                                disabled: blocked || !dirty,
                                onclick: move |_| {
                                    let applied = state
                                        .read()
                                        .plugin_tables
                                        .editors
                                        .get(level_key(scope(&state.read()).level()))
                                        .map(|editor| editor.applied.clone());
                                    if let Some(applied) = applied {
                                        state.write().edit_table(view, |draft| *draft = applied);
                                    }
                                },
                                {tr("table-discard")}
                            }
                            Button {
                                variant: ButtonVariant::Outline,
                                disabled: blocked || dirty,
                                onclick: move |_| {
                                    if current_view(&state.read(), view) {
                                        load(state, Mode::Saved, 0);
                                    }
                                },
                                {tr("table-use-saved")}
                            }
                            Button {
                                variant: ButtonVariant::Outline,
                                disabled: blocked || dirty || !file_available || frame.catalog.columns.is_empty()
                                    || editor.applied.validate().is_err()
                                    || frame.page.as_ref().is_some_and(|page| page.sort_error.is_some()),
                                onclick: move |_| save(state, view),
                                {tr("table-save")}
                            }
                        }
                        if dirty {
                            p { class: "text-xs text-muted-foreground", {tr("table-draft-help")} }
                        }
                    }
                }
                if frame.missing {
                    p { role: "alert", {tr("table-missing-scope")} }
                }
                if editor.applied.selected().is_empty() && !frame.catalog.columns.is_empty() {
                    p { {tr("table-no-visible")} }
                }
                if let Some(page) = frame.page {
                    if let Some(error) = page.sort_error {
                        p { role: "alert", class: "text-sm text-destructive",
                            {tr("table-sort-failed")}
                            " {error:?}"
                        }
                    }
                    div {
                        render::Grid {
                            key: "{view.session}-{view.generation}-{state.read().settings.ui.language}-{state.read().settings.ui.theme}",
                            page: page.clone(),
                            level: frame.catalog.level,
                            view,
                        }
                    }
                    div { class: "flex items-center gap-3 text-xs text-muted-foreground",
                        Button {
                            variant: ButtonVariant::Ghost,
                            disabled: blocked || frame.offset == 0,
                            onclick: move |_| {
                                if current_view(&state.read(), view) {
                                    load(state, Mode::Refresh, frame.offset.saturating_sub(PAGE));
                                }
                            },
                            {tr("previous")}
                        }
                        span { "{frame.offset} / {page.total}" }
                        Button {
                            variant: ButtonVariant::Ghost,
                            disabled: blocked || frame.offset.saturating_add(PAGE) >= page.total,
                            onclick: move |_| {
                                if current_view(&state.read(), view) {
                                    load(state, Mode::Refresh, frame.offset.saturating_add(PAGE));
                                }
                            },
                            {tr("next")}
                        }
                    }
                }
            }
        }
    }
}
