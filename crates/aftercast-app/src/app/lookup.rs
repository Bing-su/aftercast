use super::{
    AppState,
    log_window::{ContextEndpoint, WindowAppearance, guard},
};
use crate::components::{Accordion, Button, ButtonVariant};
use aftercast::{
    ActionSnapshot, ClassifiedAction, EncounterId, SourceLogLink, TablePage, TableQuery,
    TableScope, TableSort,
};
use dioxus::prelude::*;
use std::collections::BTreeSet;

#[derive(Clone)]
pub(super) struct Lookup {
    pub(super) snapshot: ActionSnapshot,
    pub(super) actions: Vec<ClassifiedAction>,
    dropped_logs: u64,
}

pub(super) async fn load(
    endpoint: &ContextEndpoint,
    combatant: &str,
    category: &str,
    outgoing: bool,
) -> Result<Option<Lookup>, String> {
    guard(endpoint, async {
        let Some(snapshot) = endpoint
            .handle
            .capture_category_actions(endpoint.id, combatant, category, outgoing)
            .await
            .map_err(|e| e.to_string())?
        else {
            return Ok(None);
        };
        let mut actions = Vec::with_capacity(snapshot.total as usize);
        while actions.len() < snapshot.total as usize {
            let page = endpoint
                .handle
                .action_snapshot_page(snapshot, actions.len() as u32, 256)
                .await
                .map_err(|e| e.to_string())?;
            if page.items.is_empty() {
                return Err("Category snapshot ended early".into());
            }
            actions.extend(page.items);
        }
        let dropped_logs = endpoint
            .handle
            .action_snapshot_logs(snapshot, 0, 1)
            .await
            .map_err(|e| e.to_string())?
            .dropped;
        Ok(Some(Lookup {
            snapshot,
            actions,
            dropped_logs,
        }))
    })
    .await
}

pub(super) fn peer(row: &ClassifiedAction, outgoing: bool) -> &str {
    if outgoing {
        &row.action.victim
    } else {
        &row.action.attacker
    }
}

// ACT adds every row to the aggregate and then adds it to its skill. An action
// literally named All is therefore added twice to that same aggregate.
pub(super) fn matched_indices(
    actions: &[ClassifiedAction],
    name: &str,
    outgoing: bool,
    skill: Option<&str>,
) -> Vec<usize> {
    let aggregate = skill.is_none() || skill == Some("All");
    let mut indices = Vec::new();
    for (index, row) in actions.iter().enumerate() {
        if peer(row, outgoing) != name {
            continue;
        }
        if aggregate || skill == Some(row.action.attack.as_str()) {
            indices.push(index);
        }
        if aggregate && row.action.attack == "All" {
            indices.push(index);
        }
    }
    indices.sort_by_key(|&index| {
        std::cmp::Reverse(
            actions
                .get(index)
                .map(|row| (row.action.time_sorter, row.action.time.ticks)),
        )
    });
    indices
}

#[component]
pub(super) fn LookupPanel(
    id: EncounterId,
    combatant: String,
    category: String,
    outgoing: bool,
    partial: bool,
) -> Element {
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
            title: category.clone(),
            theme: app.read().settings.ui.theme.clone(),
            language: app.read().settings.ui.language.clone(),
        },
    };
    let mut selected_peer = use_signal(|| None::<String>);
    let mut selected_attack = use_signal(|| None::<(String, bool)>);
    let mut selected_row = use_signal(|| None::<usize>);
    let mut page = use_signal(|| 0usize);
    let mut enabled = use_signal(|| false);
    let request_endpoint = endpoint.clone();
    let request_actor = combatant.clone();
    let request_category = category.clone();
    let mut data = use_resource(move || {
        let endpoint = request_endpoint.clone();
        let combatant = request_actor.clone();
        let category = request_category.clone();
        let active = enabled();
        async move {
            if active {
                load(&endpoint, &combatant, &category, outgoing).await
            } else {
                Ok(None)
            }
        }
    });
    let pending = matches!(*data.state().read(), UseResourceState::Pending);
    let current = if pending { None } else { data.read().clone() };
    let log_endpoint = endpoint.clone();
    let log = use_resource(move || {
        let endpoint = log_endpoint.clone();
        let current = data.read().clone();
        let index = selected_row();
        async move {
            let Some(Ok(Some(report))) = current else {
                return Ok::<Option<String>, String>(None);
            };
            let Some(index) = index else {
                return Ok(None);
            };
            let Some(row) = report.actions.get(index) else {
                return Ok(None);
            };
            let SourceLogLink::Retained(offset) = row.action.source else {
                return Ok(Some("Original log is unavailable or ambiguous.".into()));
            };
            guard(&endpoint, async {
                let page = endpoint
                    .handle
                    .action_snapshot_logs(report.snapshot, offset, 1)
                    .await
                    .map_err(|e| e.to_string())?;
                let entry = page
                    .page
                    .items
                    .into_iter()
                    .next()
                    .ok_or("Original log unavailable")?;
                if entry.source != SourceLogLink::Retained(offset)
                    || entry.time_sorter != row.action.time_sorter
                {
                    return Err("Original log identity changed".into());
                }
                Ok(Some(entry.text))
            })
            .await
        }
    });
    rsx! {
        Accordion {
            class: "mt-5 border border-border rounded-md".to_owned(),
            default_open: false,
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! { "Lookup by combatant — {combatant} / {category}" },
            div { class: "p-4 space-y-3",
                if partial {
                    p { "Import is incomplete or still running; actions may be missing." }
                }
                Button {
                    variant: ButtonVariant::Outline,
                    disabled: pending && enabled(),
                    onclick: move |_| {
                        selected_peer.set(None);
                        selected_attack.set(None);
                        selected_row.set(None);
                        page.set(0);
                        if enabled() {
                            data.restart();
                        } else {
                            enabled.set(true);
                        }
                    },
                    "Load / refresh"
                }
                if pending && enabled() {
                    Button {
                        variant: ButtonVariant::Outline,
                        onclick: move |_| {
                            selected_peer.set(None);
                            selected_attack.set(None);
                            selected_row.set(None);
                            page.set(0);
                            data.cancel();
                        },
                        "Cancel"
                    }
                }
                if !enabled() {
                    p { "Load the selected category to inspect its combatants." }
                } else if matches!(*data.state().read(), UseResourceState::Stopped) {
                    p { "Category lookup canceled." }
                } else {
                    match current {
                        None => rsx! {
                            p { "Loading category snapshot…" }
                        },
                        Some(Err(error)) => rsx! {
                            p { role: "alert", "{error}" }
                        },
                        Some(Ok(None)) => rsx! {
                            p { "Selected category is unavailable." }
                        },
                        Some(Ok(Some(report))) => {
                            let peers: BTreeSet<String> = report
                                .actions
                                .iter()
                                .map(|row| peer(row, outgoing).to_owned())
                                .collect();
                            let chosen = selected_peer();
                            let attacks: BTreeSet<String> = report
                                .actions
                                .iter()
                                .filter(|row| chosen.as_deref() == Some(peer(row, outgoing)))
                                .map(|row| row.action.attack.clone())
                                .collect();
                            let chosen_attack = selected_attack();
                            rsx! {
                                AggregateTable {
                                    key: "aggregate-{report.snapshot:?}-{combatant}-{category}-{selected_peer():?}-{selected_attack():?}",
                                    endpoint: LookupEndpoint(endpoint.clone()),
                                    snapshot: report.snapshot,
                                    combatant: combatant.clone(),
                                    category: category.clone(),
                                    peer: selected_peer(),
                                    attack: selected_attack(),
                                    selected_peer,
                                    selected_attack,
                                    selected_row,
                                }
                                p {
                                    "{report.actions.len()} retained category actions; {report.dropped_logs} original logs dropped. Select a combatant, then a skill and action."
                                }
                                for name in peers {
                                    {
                                        let value = name.clone();
                                        rsx! {
                                            Button {
                                                variant: ButtonVariant::Outline,
                                                onclick: move |_| {
                                                    selected_peer.set(Some(value.clone()));
                                                    selected_attack.set(None);
                                                    selected_row.set(None);
                                                    page.set(0);
                                                },
                                                "{name}"
                                            }
                                        }
                                    }
                                }
                                if let Some(name) = chosen {
                                    p { "Combatant: {name}" }
                                    Button {
                                        variant: ButtonVariant::Outline,
                                        onclick: move |_| {
                                            selected_attack.set(Some((String::new(), true)));
                                            selected_row.set(None);
                                            page.set(0);
                                        },
                                        "All actions"
                                    }
                                    for attack in attacks {
                                        {
                                            let value = attack.clone();
                                            rsx! {
                                                Button {
                                                    variant: ButtonVariant::Outline,
                                                    onclick: move |_| {
                                                        selected_attack.set(Some((value.clone(), false)));
                                                        selected_row.set(None);
                                                        page.set(0);
                                                    },
                                                    "{attack}"
                                                }
                                            }
                                        }
                                    }
                                }
                                if let (Some(name), Some((attack, all))) = (selected_peer(), chosen_attack) {
                                    {
                                        let matching = matched_indices(
                                            &report.actions,
                                            &name,
                                            outgoing,
                                            if all { None } else { Some(&attack) },
                                        );
                                        let total = matching.len();
                                        let current_page = page();
                                        rsx! {
                                            p { "{total} actions; page {current_page + 1}" }
                                            Button {
                                                variant: ButtonVariant::Outline,
                                                disabled: current_page == 0,
                                                onclick: move |_| {
                                                    page.set(current_page.saturating_sub(1));
                                                    selected_row.set(None);
                                                },
                                                "Previous"
                                            }
                                            Button {
                                                variant: ButtonVariant::Outline,
                                                disabled: (current_page + 1).saturating_mul(128) >= total,
                                                onclick: move |_| {
                                                    page.set(current_page + 1);
                                                    selected_row.set(None);
                                                },
                                                "Next"
                                            }
                                            for index in matching.into_iter().skip(current_page.saturating_mul(128)).take(128) {
                                                if let Some(row) = report.actions.get(index) {
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        onclick: move |_| selected_row.set(Some(index)),
                                                        "{row.action.time.ticks} — {row.action.attacker} → {row.action.victim}: {row.action.attack} {row.action.damage}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                if selected_row().is_some() {
                                    match log.read().clone() {
                                        Some(Ok(Some(text))) => rsx! {
                                            pre { "{text}" }
                                        },
                                        Some(Err(error)) => rsx! {
                                            p { role: "alert", "{error}" }
                                        },
                                        _ => rsx! {
                                            p { "Loading original log…" }
                                        },
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

#[derive(Clone, PartialEq, Eq)]
pub(super) struct RequestState {
    pub(super) offset: u32,
    pub(super) sort: Option<TableSort>,
    pub(super) columns: Option<Vec<String>>,
    pub(super) epoch: u64,
}
pub(super) fn request_is_current(requested: &RequestState, current: &RequestState) -> bool {
    requested == current
}

#[derive(Clone)]
struct LookupEndpoint(ContextEndpoint);
impl PartialEq for LookupEndpoint {
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id
            && self.0.generation == other.0.generation
            && std::sync::Arc::ptr_eq(&self.0.lifetime, &other.0.lifetime)
    }
}

#[derive(Clone, Debug)]
pub(super) struct AggregateData {
    pub(super) page: TablePage,
    catalog: aftercast::TableCatalog,
}

#[cfg(test)]
pub(super) async fn load_aggregate(
    endpoint: &ContextEndpoint,
    snapshot: ActionSnapshot,
    scope: TableScope,
    peer: Option<String>,
    skill: Option<String>,
    offset: u32,
    sort: Option<TableSort>,
) -> Result<TablePage, String> {
    load_aggregate_selected(
        endpoint,
        snapshot,
        scope,
        peer,
        skill,
        RequestState {
            offset,
            sort,
            columns: None,
            epoch: 0,
        },
    )
    .await
    .map(|result| result.page)
}

pub(super) async fn load_aggregate_selected(
    endpoint: &ContextEndpoint,
    snapshot: ActionSnapshot,
    scope: TableScope,
    peer: Option<String>,
    skill: Option<String>,
    request: RequestState,
) -> Result<AggregateData, String> {
    guard(endpoint, async {
        let RequestState {
            offset,
            sort,
            columns: selected,
            ..
        } = request;
        let catalog = endpoint
            .handle
            .table_columns(scope.level())
            .await
            .map_err(|error| error.to_string())?;
        let columns: Vec<String> = if let Some(selected) = selected {
            if selected.is_empty()
                || selected.len() > 32
                || selected.iter().any(|key| {
                    !catalog
                        .columns
                        .iter()
                        .any(|column| column.key == *key && column.can_read)
                })
            {
                return Err("Selected columns are unavailable; reload the table.".into());
            }
            selected
        } else {
            let mut defaults: Vec<String> = catalog
                .columns
                .iter()
                .filter(|column| column.default_visible && column.can_read)
                .take(32)
                .map(|column| column.key.clone())
                .collect();
            if defaults.is_empty()
                && let Some(column) = catalog.columns.iter().find(|column| column.can_read)
            {
                defaults.push(column.key.clone());
            }
            defaults
        };
        if columns.is_empty() {
            return Err("No readable plugin columns are registered for this table.".into());
        }
        let query = TableQuery {
            scope,
            revision: catalog.revision,
            columns,
            offset,
            limit: 16,
            sort,
        };
        let page = endpoint
            .handle
            .lookup_table_rows(snapshot, &query, peer.as_deref(), skill.as_deref())
            .await
            .map_err(|error| error.to_string())?;
        Ok(AggregateData { page, catalog })
    })
    .await
}

pub(super) fn aggregate_row_label(row: &aftercast::TableRow) -> String {
    match (&row.combatant, &row.attack) {
        (Some(peer), Some(attack)) => format!("{peer} / {attack}"),
        (Some(peer), None) => peer.clone(),
        _ => "Unknown".into(),
    }
}

#[component]
fn AggregateTable(
    endpoint: LookupEndpoint,
    snapshot: ActionSnapshot,
    combatant: String,
    category: String,
    peer: Option<String>,
    attack: Option<(String, bool)>,
    mut selected_peer: Signal<Option<String>>,
    mut selected_attack: Signal<Option<(String, bool)>>,
    mut selected_row: Signal<Option<usize>>,
) -> Element {
    let mut offset = use_signal(|| 0u32);
    let mut selected_columns = use_signal(|| None::<Vec<String>>);
    let mut request_epoch = use_signal(|| 0u64);
    let mut sort = use_signal(|| None::<TableSort>);
    let peer_value = peer.clone();
    let skill = attack
        .as_ref()
        .map(|(name, all)| if *all { "All".to_owned() } else { name.clone() });
    let scope = match (&peer, &skill) {
        (None, _) => TableScope::Categories {
            encounter: endpoint.0.id,
            combatant: combatant.clone(),
        },
        (Some(_), None) => TableScope::Attacks {
            encounter: endpoint.0.id,
            combatant: combatant.clone(),
            category: category.clone(),
        },
        (Some(_), Some(name)) => TableScope::Actions {
            encounter: endpoint.0.id,
            combatant: combatant.clone(),
            category: category.clone(),
            attack: name.clone(),
        },
    };
    let source = endpoint.0.clone();
    let request_skill = skill.clone();
    let mut resource = use_resource(move || {
        let endpoint = source.clone();
        let scope = scope.clone();
        let peer = peer_value.clone();
        let skill = request_skill.clone();
        let requested = RequestState {
            offset: offset(),
            sort: sort(),
            columns: selected_columns(),
            epoch: request_epoch(),
        };
        async move {
            let result =
                load_aggregate_selected(&endpoint, snapshot, scope, peer, skill, requested.clone())
                    .await;
            let current = RequestState {
                offset: offset(),
                sort: sort(),
                columns: selected_columns(),
                epoch: request_epoch(),
            };
            if !request_is_current(&requested, &current) {
                return Err("Lookup request was superseded.".into());
            }
            result
        }
    });
    let pending = matches!(*resource.state().read(), UseResourceState::Pending);
    let stopped = matches!(*resource.state().read(), UseResourceState::Stopped);
    let current = if pending || stopped {
        None
    } else {
        resource.read().clone()
    };
    rsx! {
        div { class: "space-y-2 border border-border rounded-md p-3",
            p { class: "font-semibold", "ACT aggregate table" }
            Button {
                variant: ButtonVariant::Outline,
                disabled: !pending,
                onclick: move |_| {
                    request_epoch.set(request_epoch().wrapping_add(1));
                    resource.cancel();
                },
                "Cancel table request"
            }
            Button {
                variant: ButtonVariant::Outline,
                disabled: pending,
                onclick: move |_| {
                    request_epoch.set(request_epoch().wrapping_add(1));
                    resource.restart();
                },
                "Retry table"
            }
            if stopped {
                p { "Table request canceled." }
            } else if pending {
                p { "Loading table…" }
            } else {
                match current {
                    Some(Err(error)) => rsx! {
                        p { role: "alert", "{error}" }
                    },
                    None => rsx! {
                        p { "Table unavailable." }
                    },
                    Some(Ok(result)) => {
                        let page = result.page;
                        let catalog = result.catalog;
                        let visible: Vec<String> = page
                            .columns
                            .iter()
                            .map(|column| column.key.clone())
                            .collect();
                        rsx! {
                            if let Some(error) = page.sort_error {
                                p { role: "alert", "Plugin sort failed: {error:?}" }
                            }
                            p { "{page.total} rows; offset {offset}" }
                            Accordion {
                                class: "border border-border rounded-md".to_owned(),
                                default_open: false,
                                trigger_class: "px-3 py-2".to_owned(),
                                header: rsx! { "Choose columns" },
                                for column in catalog.columns.iter().filter(|column| column.can_read) {
                                    {
                                        let key = column.key.clone();
                                        let mut next = visible.clone();
                                        let checked = next.contains(&key);
                                        let disabled = (checked && next.len() == 1) || (!checked && next.len() >= 32);
                                        rsx! {
                                            Button {
                                                variant: ButtonVariant::Outline,
                                                aria_pressed: checked,
                                                disabled,
                                                onclick: move |_| {
                                                    if checked {
                                                        next.retain(|item| item != &key);
                                                    } else {
                                                        next.push(key.clone());
                                                    }
                                                    selected_columns.set(Some(next.clone()));
                                                    offset.set(0);
                                                },
                                                "{column.label} [{column.key}]"
                                            }
                                        }
                                    }
                                }
                            }
                            div { class: "flex flex-wrap gap-2",
                                for column in page.columns.iter().filter(|column| column.can_sort) {
                                    {
                                        let key = column.key.clone();
                                        rsx! {
                                            Button {
                                                variant: ButtonVariant::Outline,
                                                onclick: move |_| {
                                                    let descending = sort()
                                                        .as_ref()
                                                        .is_none_or(|current| current.primary != key || !current.descending);
                                                    sort.set(
                                                        Some(TableSort {
                                                            primary: key.clone(),
                                                            secondary: None,
                                                            descending,
                                                        }),
                                                    );
                                                    offset.set(0);
                                                },
                                                "Sort: {column.label}"
                                            }
                                        }
                                    }
                                }
                            }
                            p { class: "text-xs text-muted-foreground",
                                for column in &page.columns {
                                    span { "{column.label} | " }
                                }
                            }
                            for row in &page.rows {
                                {
                                    let identity = aggregate_row_label(row);
                                    let peer_name = row.combatant.clone();
                                    let attack_name = row.attack.clone();
                                    let values = row
                                        .cells
                                        .iter()
                                        .map(|cell| {
                                            if cell.error.is_some() {
                                                "ERROR".to_owned()
                                            } else {
                                                cell.text.clone().unwrap_or_default()
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .join(" | ");
                                    let can_open = skill.is_none();
                                    rsx! {
                                        Button {
                                            variant: ButtonVariant::Outline,
                                            disabled: !can_open,
                                            onclick: move |_| {
                                                if let Some(name) = peer_name.clone() {
                                                    if let Some(skill) = attack_name.clone() {
                                                        selected_attack.set(Some((skill, false)));
                                                    } else {
                                                        selected_peer.set(Some(name));
                                                        selected_attack.set(None);
                                                    }
                                                    selected_row.set(None);
                                                    offset.set(0);
                                                    sort.set(None);
                                                }
                                            },
                                            "{identity}: {values}"
                                        }
                                    }
                                }
                            }
                            div { class: "flex gap-2",
                                Button {
                                    variant: ButtonVariant::Outline,
                                    disabled: offset() == 0,
                                    onclick: move |_| offset.set(offset().saturating_sub(16)),
                                    "Previous"
                                }
                                Button {
                                    variant: ButtonVariant::Outline,
                                    disabled: offset().saturating_add(16) >= page.total,
                                    onclick: move |_| offset.set(offset().saturating_add(16)),
                                    "Next"
                                }
                                Button {
                                    variant: ButtonVariant::Outline,
                                    disabled: peer.is_none(),
                                    onclick: move |_| {
                                        if attack.is_some() {
                                            selected_attack.set(None);
                                        } else {
                                            selected_peer.set(None);
                                        }
                                        selected_row.set(None);
                                        offset.set(0);
                                        sort.set(None);
                                    },
                                    "Back"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
