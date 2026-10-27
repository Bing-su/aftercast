use super::{AppState, tr};
use dioxus::desktop::{DesktopContext, WeakDesktopContext};
use dioxus::{core::spawn_forever, prelude::*};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
mod model;
mod window;
use model::{Context, Hub, Kind};
#[cfg(test)]
mod tests;

#[derive(Default)]
struct Launch {
    serial: u64,
    pending: Option<u64>,
    alive: Arc<AtomicBool>,
    error: bool,
}
impl Launch {
    fn reserve(&mut self) -> Option<(u64, Arc<AtomicBool>)> {
        if self.pending.is_some() || self.alive.load(Ordering::SeqCst) {
            return None;
        }
        self.serial = self.serial.checked_add(1)?;
        self.pending = Some(self.serial);
        self.error = false;
        self.alive = Arc::new(AtomicBool::new(true));
        Some((self.serial, self.alive.clone()))
    }
    fn resolve(&mut self, serial: u64, success: bool) -> bool {
        if self.pending != Some(serial) {
            return false;
        }
        self.pending = None;
        self.error = !success;
        if !success {
            self.alive.store(false, Ordering::SeqCst);
        }
        true
    }
}
#[derive(Default)]
pub(super) struct State {
    pub(super) hub: Arc<Hub>,
    launch: Launch,
    desktop: WeakDesktopContext,
}
impl State {
    pub(super) fn retire(&self) {
        self.hub.retire();
    }
    pub(super) fn close(&self) {
        self.hub.close();
    }
}
impl AppState {
    fn progress_context(&self) -> Context {
        let kind = if self.clipboard.importing() {
            Kind::Text
        } else if self.reparse.active() {
            Kind::Range
        } else if self.import_text.active() {
            Kind::File
        } else {
            Kind::Saved
        };
        Context {
            kind,
            viewpoint: self.active_import_character().flatten(),
        }
    }
}
fn desktop_context() -> Option<DesktopContext> {
    dioxus::core::Runtime::try_current()?.try_current_scope_id()?;
    try_consume_context::<DesktopContext>()
}
pub(super) fn open(mut state: Signal<AppState>) {
    if state.peek().exit_requested || state.peek().import_progress.hub.closed() {
        return;
    }
    let Some(desktop) = desktop_context() else {
        state.write().import_progress.launch.error = true;
        return;
    };
    let (serial, alive, hub, appearance) = {
        let mut current = state.write();
        if current.exit_requested || current.import_progress.hub.closed() {
            return;
        }
        if current.import_progress.launch.alive.load(Ordering::SeqCst)
            && let Some(existing) = current.import_progress.desktop.upgrade()
        {
            existing.set_visible(true);
            existing.set_focus();
            return;
        }
        let Some((serial, alive)) = current.import_progress.launch.reserve() else {
            return;
        };
        let appearance = super::log_window::WindowAppearance {
            title: tr("import-progress-title"),
            theme: current.settings.ui.theme.clone(),
            language: current.settings.ui.language.clone(),
        };
        (
            serial,
            alive,
            current.import_progress.hub.clone(),
            appearance,
        )
    };
    let pending = window::open(&desktop, hub.clone(), alive, appearance);
    // Creation belongs to the app, e.g. leaving Import cannot strand a pending singleton window.
    spawn_forever(async move {
        let result = pending.try_resolve().await;
        if !state
            .write()
            .import_progress
            .launch
            .resolve(serial, result.is_ok())
        {
            if let Ok(window) = result {
                window::destroy(&window);
            }
            return;
        }
        match result {
            Ok(window) if hub.closed() => window::destroy(&window),
            Ok(window) => state.write().import_progress.desktop = std::rc::Rc::downgrade(&window),
            Err(_) => {}
        }
    });
}
// Called by the existing common owner wait: all raw imports and captured replay share cleanup.
pub(super) async fn wait(
    mut state: Signal<AppState>,
    mut job: aftercast::ImportJob,
    session: u64,
) -> Result<aftercast::ImportReport, aftercast::Error> {
    let hub = state.peek().import_progress.hub.clone();
    let token = (state.peek().revision.session == session)
        .then(|| hub.begin(session, state.peek().progress_context(), job.progress()))
        .flatten();
    if token.is_some() && desktop_context().is_some() {
        open(state);
    }
    loop {
        if state.peek().revision.session != session
            || state.peek().cancel_import
            || token.is_none_or(|token| hub.should_cancel(token))
        {
            job.cancel();
        }
        tokio::select! {
            result=&mut job=>{
                if let Some(token)=token {hub.publish(token,job.progress());hub.finish(token,&result);}
                return result;
            },
            ()=dioxus_sdk::time::sleep(std::time::Duration::from_millis(100))=>{
                if let Some(token)=token {hub.publish(token,job.progress());}
                if state.peek().revision.session==session {state.write().progress=job.progress();}
            }
        }
    }
}
#[component]
pub(super) fn OpenButton() -> Element {
    let state = use_context::<Signal<AppState>>();
    rsx! {
        crate::components::Button {
            variant: crate::components::ButtonVariant::Outline,
            disabled: state.read().exit_requested,
            onclick: move |_| open(state),
            {tr("import-progress-open")}
        }
        if state.read().import_progress.launch.error {
            p { role: "alert", class: "text-sm text-ctp-red", {tr("import-progress-open-failed")} }
        }
    }
}
