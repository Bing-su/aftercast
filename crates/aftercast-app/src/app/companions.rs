use super::{AppState, Phase, import_text, tr};
use aftercast::{
    CompanionCatalog, CompanionSource, HostHandle, ImportJob, ImportRange, ImportTextOptions,
    LiveLogOptions, LiveLogSelection, LiveLogSnapshot, SourceImportOptions,
};
use camino::{Utf8Path, Utf8PathBuf};
use dioxus::{core::spawn_forever, prelude::*};

#[cfg(test)]
mod tests;
mod view;

#[derive(Clone, Debug)]
struct Choice {
    name: String,
    text: import_text::Draft,
}
#[derive(Clone, Debug)]
pub(super) struct Selection {
    path: Utf8PathBuf,
    sources: Vec<CompanionSource>,
}
impl Selection {
    pub(super) async fn start(
        &self,
        host: &HostHandle,
        path: &Utf8Path,
        range: Option<ImportRange>,
        text: Option<ImportTextOptions>,
    ) -> Result<ImportJob, aftercast::Error> {
        if path != self.path {
            return Err(aftercast::Error::InvalidCompanionRequest("Selected companions belong to a different main path; reload candidates for this source".into()));
        }
        host.start_import_sources(
            path,
            SourceImportOptions {
                range,
                text,
                companions: self.sources.clone(),
            },
        )
        .await
    }
}
/// The admitted request settings, kept separate from editable choices and live readers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ActiveImport {
    pub(super) path: Utf8PathBuf,
    pub(super) range: Option<ImportRange>,
    pub(super) text: Option<ImportTextOptions>,
    pub(super) sources: Vec<CompanionSource>,
}
impl ActiveImport {
    pub(super) fn new(
        path: Utf8PathBuf,
        range: Option<ImportRange>,
        text: Option<ImportTextOptions>,
        selected: Option<&Selection>,
    ) -> Self {
        Self {
            path,
            range,
            text,
            sources: selected.map_or_else(Vec::new, |selection| selection.sources.clone()),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Token {
    session: u64,
    generation: u64,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    Catalog,
    Open,
    Close,
    Status,
}
#[derive(Default)]
pub(super) struct State {
    generation: u64,
    pending: Option<(Token, Kind)>,
    catalog: Option<CompanionCatalog>,
    choices: Vec<Choice>,
    wait: String,
    zone: bool,
    live: Option<LiveLogSnapshot>,
    error: Option<String>,
    pub(super) excluded: u64,
}
impl State {
    pub(super) fn reset_session(&mut self) {
        self.generation += 1;
        self.pending = None;
        self.catalog = None;
        self.choices.clear();
        self.live = None;
        self.error = None;
        self.excluded = 0;
    }
    pub(super) fn invalidate_catalog(&mut self) {
        self.generation += 1;
        // A read-only status response may still be in flight when the path draft changes.
        if self.pending.is_some_and(|(_, kind)| kind == Kind::Status) {
            self.pending = None;
        }
        self.catalog = None;
        self.choices.clear();
    }
    pub(super) fn selection(&self) -> Option<Selection> {
        if self.choices.is_empty() {
            return None;
        }
        self.catalog.as_ref().map(|catalog| Selection {
            path: catalog.path.clone(),
            sources: self
                .choices
                .iter()
                .map(|choice| CompanionSource {
                    name: choice.name.clone(),
                    text: choice.text.options(),
                })
                .collect(),
        })
    }
    fn live_options(
        &self,
        text: Option<ImportTextOptions>,
    ) -> Result<LiveLogOptions, &'static str> {
        let wait = if self.wait.is_empty() {
            30000
        } else {
            self.wait
                .parse::<u32>()
                .map_err(|_error| "sources-invalid-wait")?
        };
        if !(1..=300000).contains(&wait) || self.choices.len() > 8 {
            return Err("sources-invalid-wait");
        }
        Ok(LiveLogOptions {
            text,
            companions: self
                .selection()
                .map_or_else(Vec::new, |selection| selection.sources),
            get_current_zone: self.zone,
            max_clock_wait: std::time::Duration::from_millis(u64::from(wait)),
        })
    }
}
impl AppState {
    fn source_import_snapshot(&self) -> Option<ActiveImport> {
        self.import_text
            .source_snapshot()
            .or_else(|| self.reparse.source_snapshot())
    }
    pub(super) fn source_view(&self) -> Token {
        Token {
            session: self.revision.session,
            generation: self.companions.generation,
        }
    }
    fn can_edit_sources(&self) -> bool {
        matches!(self.phase, Phase::Idle | Phase::Ready)
            && !self.loading
            && !self.exit_requested
            && self.companions.pending.is_none()
            && !self.import_text.active()
            && !self.reparse.active()
    }
    pub(super) fn edit_source(
        &mut self,
        view: Token,
        name: &str,
        field: &str,
        value: &str,
    ) -> Result<bool, &'static str> {
        if view != self.source_view() || !self.can_edit_sources() {
            return Ok(false);
        }
        if name.is_empty() {
            match field {
                "wait" => self.companions.wait = value.into(),
                "zone" => self.companions.zone = value == "true",
                _ => return Err("import-text-invalid-choice"),
            };
            return Ok(true);
        }
        let Some(catalog) = &self.companions.catalog else {
            return Ok(false);
        };
        if !catalog
            .candidates
            .iter()
            .any(|candidate| candidate.name == name)
        {
            return Ok(false);
        }
        if field == "selected" {
            if let Some(index) = self
                .companions
                .choices
                .iter()
                .position(|choice| choice.name == name)
            {
                self.companions.choices.remove(index);
            } else {
                self.companions.choices.push(Choice {
                    name: name.into(),
                    text: Default::default(),
                });
            }
        } else if let Some(choice) = self
            .companions
            .choices
            .iter_mut()
            .find(|choice| choice.name == name)
        {
            choice.text.change(field, value)?;
        }
        Ok(true)
    }
    pub(super) fn begin_sources(&mut self, kind: Kind) -> Option<Token> {
        if !(self.phase == Phase::Ready || kind == Kind::Status && self.phase == Phase::Importing)
            || self.exit_requested
            || self.loading
            || self.companions.pending.is_some()
        {
            return None;
        }
        self.companions.generation += 1;
        let token = self.source_view();
        self.companions.pending = Some((token, kind));
        if kind != Kind::Status {
            self.phase = Phase::ChangingSources;
            self.companions.error = None;
        }
        Some(token)
    }
    fn finish_sources(&mut self, token: Token, kind: Kind) -> bool {
        if self.source_view() != token || self.companions.pending != Some((token, kind)) {
            return false;
        }
        self.companions.pending = None;
        if kind != Kind::Status {
            self.phase = Phase::Ready;
        }
        true
    }
    pub(super) fn receive_catalog(
        &mut self,
        token: Token,
        result: Result<CompanionCatalog, aftercast::Error>,
    ) -> bool {
        if !self.finish_sources(token, Kind::Catalog) {
            return false;
        }
        match result {
            Ok(catalog) => {
                self.companions.choices.retain(|choice| {
                    catalog
                        .candidates
                        .iter()
                        .any(|candidate| candidate.name == choice.name)
                });
                self.import_path = catalog.path.to_string();
                self.companions.catalog = Some(catalog);
            }
            Err(error) => self.companions.error = Some(error.to_string()),
        };
        true
    }
    fn receive_live(
        &mut self,
        token: Token,
        kind: Kind,
        result: Result<LiveLogSnapshot, aftercast::Error>,
        retained: Option<LiveLogSnapshot>,
    ) -> bool {
        if !self.finish_sources(token, kind) {
            return false;
        }
        match result {
            Ok(snapshot) => {
                self.companions.live = Some(snapshot);
                // Polling healthy readers must not erase a failed explicit open/close result.
                if kind != Kind::Status {
                    self.companions.error = None;
                }
            }
            Err(error) => {
                self.companions.live = retained;
                self.companions.error = Some(error.to_string());
            }
        }
        true
    }
    fn receive_close(
        &mut self,
        token: Token,
        result: Result<(), aftercast::Error>,
        retained: Option<LiveLogSnapshot>,
    ) -> bool {
        if !self.finish_sources(token, Kind::Close) {
            return false;
        }
        match result {
            Ok(()) => self.companions.live = None,
            Err(error) => {
                self.companions.live = retained;
                self.companions.error = Some(error.to_string());
            }
        }
        true
    }
}
fn edit(mut state: Signal<AppState>, view: Token, name: &str, field: &str, value: &str) {
    let result = state.write().edit_source(view, name, field, value);
    if let Err(key) = result {
        state.write().fail(tr(key));
    }
}
fn catalog(mut state: Signal<AppState>, view: Token) {
    if state.read().source_view() != view {
        return;
    }
    let Some(host) = state.read().handle() else {
        return;
    };
    let path = crate::config::resolve(&state.read().root, state.read().import_path.trim());
    let Some(token) = state.write().begin_sources(Kind::Catalog) else {
        return;
    };
    spawn_forever(async move {
        let result = host.companion_candidates(path).await;
        state.write().receive_catalog(token, result);
    });
}
async fn retained(
    host: &HostHandle,
    selection: Option<LiveLogSelection>,
) -> Option<LiveLogSnapshot> {
    match selection {
        Some(selection) => host.live_logs(selection).await.ok(),
        None => None,
    }
}
fn open(mut state: Signal<AppState>, view: Token) {
    if state.read().source_view() != view {
        return;
    }
    let Some(host) = state.read().handle() else {
        return;
    };
    let compiled = {
        let current = state.read();
        current
            .companions
            .live_options(current.import_text.options())
    };
    let options = match compiled {
        Ok(options) => options,
        Err(key) => {
            state.write().fail(tr(key));
            return;
        }
    };
    let path = crate::config::resolve(&state.read().root, state.read().import_path.trim());
    let previous = state
        .read()
        .companions
        .live
        .as_ref()
        .map(|snapshot| snapshot.selection);
    let Some(token) = state.write().begin_sources(Kind::Open) else {
        return;
    };
    spawn_forever(async move {
        let result = if let Some(selection) = previous {
            host.reopen_live_logs(selection, path, options).await
        } else {
            host.open_live_logs(path, options).await
        };
        let kept = if result.is_err() {
            retained(&host, previous).await
        } else {
            None
        };
        state.write().receive_live(token, Kind::Open, result, kept);
    });
}
fn close(mut state: Signal<AppState>, view: Token) {
    if state.read().source_view() != view {
        return;
    }
    let Some(host) = state.read().handle() else {
        return;
    };
    let Some(selection) = state
        .read()
        .companions
        .live
        .as_ref()
        .map(|snapshot| snapshot.selection)
    else {
        return;
    };
    let Some(token) = state.write().begin_sources(Kind::Close) else {
        return;
    };
    spawn_forever(async move {
        let result = host.close_live_logs(selection).await;
        let kept = if result.is_err() {
            retained(&host, Some(selection)).await
        } else {
            None
        };
        state.write().receive_close(token, result, kept);
    });
}
pub(super) async fn poll(mut state: Signal<AppState>) {
    loop {
        dioxus_sdk::time::sleep(std::time::Duration::from_secs(1)).await;
        let Some(host) = state.read().handle() else {
            continue;
        };
        let Some(selection) = state
            .read()
            .companions
            .live
            .as_ref()
            .map(|snapshot| snapshot.selection)
        else {
            continue;
        };
        let Some(token) = state.write().begin_sources(Kind::Status) else {
            continue;
        };
        let result = host.live_logs(selection).await;
        state
            .write()
            .receive_live(token, Kind::Status, result, None);
    }
}
#[component]
pub(super) fn Controls() -> Element {
    rsx! {
        view::Panel {}
    }
}
