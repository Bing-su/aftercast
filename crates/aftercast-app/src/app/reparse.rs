use super::{AppState, Phase, tr};
use crate::components::{Accordion, Button, ButtonVariant, Input, Label};
use aftercast::{ImportRange, ImportReport};
use camino::{Utf8Path, Utf8PathBuf};
use dioxus::{core::spawn_forever, prelude::*};

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub(super) struct Draft {
    path: String,
    start: String,
    end: String,
    file_start: bool,
    file_end: bool,
    extend: bool,
}
#[derive(Clone, Debug)]
struct Request {
    path: Utf8PathBuf,
    range: ImportRange,
    bounds: Draft,
    text: Option<aftercast::ImportTextOptions>,
    sources: Option<super::companions::Selection>,
    viewpoint: Option<aftercast::ImportCharacter>,
}
impl Draft {
    fn compile(&self, root: &Utf8Path) -> Result<Request, &'static str> {
        let path = self.path.trim();
        if path.is_empty() || path.contains('\0') {
            return Err("reparse-invalid-path");
        }
        let start = if self.file_start {
            None
        } else {
            Some(source_bound(&self.start, false)?.ok_or("reparse-required-range")?)
        };
        let end = if self.file_end {
            None
        } else if self.end.trim().len() == 10 {
            // A date means its whole inclusive day; exact timestamps remain inclusive without subtraction.
            source_bound(&self.end, true)?.and_then(|ticks| ticks.checked_sub(1))
        } else {
            source_bound(&self.end, false)?
        };
        if !self.file_end && end.is_none() {
            return Err("reparse-required-range");
        }
        let range = ImportRange::from_bounds(start, end, self.extend)
            .map_err(|_error| "reparse-invalid-range")?;
        Ok(Request {
            path: crate::config::resolve(root, path),
            range,
            bounds: self.clone(),
            text: None,
            sources: None,
            viewpoint: None,
        })
    }
}
fn source_bound(value: &str, end: bool) -> Result<Option<u64>, &'static str> {
    super::history::bound(value, end).map_err(|_error| "reparse-invalid-date")
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Token {
    session: u64,
    generation: u64,
}
#[derive(Default)]
pub(super) struct ReparseState {
    draft: Draft,
    generation: u64,
    pending: Option<(Token, Request)>,
}
impl ReparseState {
    pub(super) fn viewpoint_snapshot(&self) -> Option<Option<aftercast::ImportCharacter>> {
        Some(self.pending.as_ref()?.1.viewpoint.clone())
    }
    pub(super) fn source_snapshot(&self) -> Option<super::companions::ActiveImport> {
        let (_, request) = self.pending.as_ref()?;
        Some(super::companions::ActiveImport::new(
            request.path.clone(),
            Some(request.range),
            request.text,
            request.sources.as_ref(),
        ))
    }
    pub(super) fn reset_session(&mut self) {
        self.generation += 1;
        self.pending = None;
    }
    pub(super) fn active(&self) -> bool {
        self.pending.is_some()
    }
}
impl AppState {
    fn compile_reparse(&self) -> Result<Request, &'static str> {
        let mut request = self.reparse.draft.compile(&self.root)?;
        request.text = self.import_text.options();
        request.viewpoint = self.import_text.viewpoint()?;
        request.sources = self.companions.selection();
        Ok(request)
    }
    fn can_edit_reparse(&self) -> bool {
        !self.reparse.active()
            && matches!(self.phase, Phase::Idle | Phase::Ready)
            && !self.loading
            && !self.import_text.active()
            && !self.exit_requested
    }
    fn edit_reparse(&mut self, view: Token, field: &str, value: String) -> bool {
        if !self.current_reparse_view(view) || !self.can_edit_reparse() {
            return false;
        }
        match field {
            "reparse-path" => self.reparse.draft.path = value,
            "reparse-start" if !self.reparse.draft.file_start => self.reparse.draft.start = value,
            "reparse-end" if !self.reparse.draft.file_end => self.reparse.draft.end = value,
            "reparse-file-start" | "reparse-file-end" | "reparse-extend" => {
                let Ok(enabled) = value.parse::<bool>() else {
                    return false;
                };
                match field {
                    "reparse-file-start" => self.reparse.draft.file_start = enabled,
                    "reparse-file-end" => self.reparse.draft.file_end = enabled,
                    _ => self.reparse.draft.extend = enabled,
                }
            }
            _ => return false,
        }
        true
    }
    fn reparse_view(&self) -> Token {
        Token {
            session: self.revision.session,
            generation: self.reparse.generation,
        }
    }
    fn current_reparse_view(&self, view: Token) -> bool {
        self.reparse_view() == view
    }
    fn can_reparse(&self) -> bool {
        self.phase == Phase::Ready
            && self.import_text.viewpoint().is_ok()
            && !self.history_sources.holds_resources()
            && !self.loading
            && !self.reparse.active()
            && !self.import_text.active()
            && !self.exit_requested
            && self
                .plugins
                .iter()
                .any(|plugin| plugin.info.state == aftercast::PluginState::Initialized)
    }
    fn begin_reparse(&mut self, request: Request) -> Option<Token> {
        if !self.can_reparse() {
            return None;
        }
        self.reparse.generation += 1;
        self.import_text.invalidate_view();
        let token = Token {
            session: self.revision.session,
            generation: self.reparse.generation,
        };
        self.reparse.pending = Some((token, request));
        self.phase = Phase::Importing;
        self.cancel_import = false;
        self.progress = Default::default();
        self.error = None;
        self.message.clear();
        self.invalidate_restored_context();
        Some(token)
    }
    fn cancel_reparse(&mut self, token: Token) -> bool {
        if self
            .reparse
            .pending
            .as_ref()
            .is_none_or(|(current, _)| *current != token)
            || self.revision.session != token.session
            || self.cancel_import
        {
            return false;
        }
        self.cancel_import = true;
        true
    }
    fn receive_reparse(
        &mut self,
        token: Token,
        result: Result<ImportReport, aftercast::Error>,
        complete: &str,
        canceled: &str,
    ) -> bool {
        if self
            .reparse
            .pending
            .as_ref()
            .is_none_or(|(current, _)| *current != token)
            || self.revision.session != token.session
        {
            return false;
        }
        self.reparse.pending = None;
        self.import_text.invalidate_view();
        self.phase = Phase::Ready;
        self.invalidate_restored_context();
        match result {
            Ok(report) => {
                self.message = format!(
                    "{complete} · {} / {} · {}",
                    report.records_dispatched, report.lines_read, report.encounters
                );
                self.companions.excluded = report.companion_heads_excluded
            }
            Err(aftercast::Error::Cancelled) => self.message = canceled.to_owned(),
            Err(error) => self.fail(error),
        }
        true
    }
}
fn submit(mut state: Signal<AppState>, view: Token) {
    if !state.read().can_reparse() || !state.read().current_reparse_view(view) {
        return;
    }
    let Some(handle) = state.read().handle() else {
        return;
    };
    let request = state.read().compile_reparse();
    let request = match request {
        Ok(request) => request,
        Err(key) => {
            state.write().fail(tr(key));
            return;
        }
    };
    let Some(token) = state.write().begin_reparse(request.clone()) else {
        return;
    };
    let complete = tr("reparse-complete");
    let canceled = tr("import-cancelled");
    // The app owns accepted cleanup, e.g. leaving Import must not abandon a partially parsed graph.
    spawn_forever(async move {
        let result = match super::import_text::start_selected(
            &handle,
            &request.path,
            Some(request.range),
            request.text,
            request.sources.as_ref(),
            request.viewpoint.as_ref(),
        )
        .await
        {
            Ok(job) => super::wait_import(state, job, token.session).await,
            Err(error) => Err(error),
        };
        let received = state
            .write()
            .receive_reparse(token, result, &complete, &canceled);
        if received {
            super::refresh(state, 0);
        }
    });
}
fn edit(mut state: Signal<AppState>, view: Token, field: &str, value: String) {
    state.write().edit_reparse(view, field, value);
}
fn bounds_label(draft: &Draft) -> String {
    let start = if draft.file_start {
        tr("reparse-file-start")
    } else {
        draft.start.clone()
    };
    let end = if draft.file_end {
        tr("reparse-file-end")
    } else {
        draft.end.clone()
    };
    format!("{start} — {end}")
}
#[component]
pub(super) fn ReparseControls() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let view = current.reparse_view();
    let draft = current.reparse.draft.clone();
    let pending = current.reparse.pending.clone();
    let blocked = !current.can_edit_reparse();
    let can_submit = current.can_reparse();
    let canceling = current.cancel_import;
    drop(current);
    rsx! {
        Accordion {
            default_open: true,
            class: "my-6 rounded-md border border-border".to_owned(),
            trigger_class: "px-4 py-3 font-semibold".to_owned(),
            header: rsx! {
                {tr("reparse-title")}
            },
            section { class: "space-y-4 p-4", "aria-label": tr("reparse-title"),
                p { class: "text-sm leading-7 text-muted-foreground", {tr("reparse-help")} }
                div { class: "space-y-1",
                    Label { html_for: "reparse-path", {tr("reparse-path")} }
                    Input {
                        id: "reparse-path",
                        value: draft.path,
                        disabled: blocked,
                        oninput: move |event: FormEvent| edit(state, view, "reparse-path", event.value()),
                    }
                }
                for (id, value, toggle, file) in [
                    ("reparse-start", draft.start, "reparse-file-start", draft.file_start),
                    ("reparse-end", draft.end, "reparse-file-end", draft.file_end),
                ]
                {
                    div { class: "space-y-2",
                        Label { html_for: id.to_owned(), {tr(id)} }
                        div { class: "flex flex-wrap items-center gap-3",
                            Button {
                                id: toggle.to_owned(),
                                variant: ButtonVariant::Outline,
                                disabled: blocked,
                                aria_pressed: file,
                                onclick: move |_| edit(state, view, toggle, (!file).to_string()),
                                {tr(toggle)}
                            }
                            Input {
                                id: id.to_owned(),
                                value,
                                disabled: blocked || file,
                                oninput: move |event: FormEvent| edit(state, view, id, event.value()),
                            }
                        }
                    }
                }
                Button {
                    id: "reparse-extend",
                    variant: ButtonVariant::Outline,
                    disabled: blocked,
                    aria_pressed: draft.extend,
                    onclick: move |_| edit(state, view, "reparse-extend", (!draft.extend).to_string()),
                    {tr("reparse-extend")}
                }
                p { class: "text-xs leading-6 text-muted-foreground", {tr("reparse-boundary-help")} }
                Button {
                    disabled: !can_submit,
                    onclick: move |_| submit(state, view),
                    {tr("reparse-action")}
                }
                if let Some((token, request)) = pending {
                    p { role: "status", class: "break-words text-sm",
                        {tr("reparse-running")}
                        " {request.path} · "
                        {bounds_label(&request.bounds)}
                        " · "
                        {
                            tr(
                                if request.range.extend_to_combat_end() {
                                    "reparse-policy-extend"
                                } else {
                                    "reparse-policy-exact"
                                },
                            )
                        }
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: canceling,
                        onclick: move |_| {
                            state.write().cancel_reparse(token);
                        },
                        {tr("reparse-cancel")}
                    }
                }
            }
        }
    }
}
#[cfg(test)]
mod tests;
