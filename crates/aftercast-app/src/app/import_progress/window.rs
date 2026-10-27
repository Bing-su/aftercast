use super::*;
use crate::components::{Button, ButtonVariant, Progress};
use dioxus::desktop::{
    Config, LogicalSize, WindowBuilder, WindowCloseBehaviour,
    tao::event::{Event, WindowEvent},
    use_window, use_wry_event_handler,
};
use dioxus_i18n::{prelude::*, unic_langid::langid};
use model::{CloseAction, CloseState, Outcome, Snapshot, Token};

#[derive(Clone)]
struct Props {
    hub: Arc<Hub>,
    alive: Arc<AtomicBool>,
    appearance: super::super::log_window::WindowAppearance,
}
pub(super) fn open(
    desktop: &DesktopContext,
    hub: Arc<Hub>,
    alive: Arc<AtomicBool>,
    appearance: super::super::log_window::WindowAppearance,
) -> dioxus::desktop::PendingDesktopContext {
    let title = format!("aftercast — {}", appearance.title);
    desktop.new_window(
        VirtualDom::new_with_props(
            root,
            Props {
                hub,
                alive,
                appearance,
            },
        ),
        Config::new()
            .with_menu(None)
            .with_custom_head("<link rel=\"icon\" href=\"data:,\">".into())
            .with_close_behaviour(WindowCloseBehaviour::WindowHides)
            .with_window(
                WindowBuilder::new()
                    .with_title(title)
                    .with_inner_size(LogicalSize::new(640.0, 520.0))
                    .with_min_inner_size(LogicalSize::new(480.0, 400.0)),
            ),
    )
}
fn root(props: Props) -> Element {
    let hub = use_hook(|| props.hub.clone());
    use_context_provider(|| hub.clone());
    let appearance = props.appearance;
    let locale = try_use_init_i18n(|| {
        I18nConfig::new(appearance.language.parse().unwrap_or(langid!("en-US")))
            .with_fallback(langid!("en-US"))
            .with_locale((langid!("ko-KR"), include_str!("../../../locales/ko-KR.ftl")))
            .with_locale((langid!("en-US"), include_str!("../../../locales/en-US.ftl")))
    });
    let window = use_window();
    let id = window.id();
    let alive = use_hook(|| props.alive.clone());
    let closed = alive.clone();
    use_drop(move || {
        closed.store(false, Ordering::SeqCst);
    });
    let mut closing = use_signal(CloseState::default);
    let restore = use_hook(|| Arc::new(AtomicBool::new(false)));
    use_context_provider(|| restore.clone());
    let close_restore = restore.clone();
    let requested = restore.clone();
    let observed = hub.clone();
    use_wry_event_handler(move |event, _| {
        if matches!(event,Event::WindowEvent {window_id,event:WindowEvent::CloseRequested,..} if *window_id==id)
        {
            let confirm =
                closing.write().request(observed.snapshot().as_ref()) == CloseAction::Confirm;
            requested.store(confirm, Ordering::SeqCst);
        }
    });
    let mut snapshot = use_signal(|| hub.snapshot());
    let observed = hub.clone();
    let close = window.clone();
    use_future(move || {
        let observed = observed.clone();
        let close = close.clone();
        let restore = restore.clone();
        async move {
            while !observed.closed() {
                let current = observed.snapshot();
                closing.write().sync(current.as_ref());
                snapshot.set(current);
                // Dioxus hides on native X before the next poll; restore once to ask ACT's abort question.
                restore_if_requested(&close, &restore);
                dioxus_sdk::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            destroy(&close);
        }
    });
    if let Err(error) = locale {
        return rsx! {
            pre { role: "alert", "{error}" }
        };
    }
    rsx! {
        style { {include_str!("../../../assets/tailwind.css")} }
        main {
            class: "{appearance.theme} app-shell min-h-dvh space-y-5 bg-background p-6 text-foreground [color-scheme:dark] [&.latte]:[color-scheme:light]",
            lang: appearance.language,
            h1 { class: "text-xl font-semibold", {tr("import-progress-title")} }
            p { class: "text-sm leading-6 text-muted-foreground",
                {tr("import-progress-close-help")}
            }
            if let Some(target) = closing.read().confirmation {
                Confirmation { target, closing }
            }
            if let Some(frame) = snapshot.read().clone() {
                Frame { frame }
            } else {
                p { role: "status", {tr("import-progress-empty")} }
            }
            Button {
                variant: ButtonVariant::Outline,
                onclick: move |_| {
                    close_restore.store(false, Ordering::SeqCst);
                    let hide = closing.write().request(hub.snapshot().as_ref()) == CloseAction::Hide;
                    window.set_visible(!hide);
                },
                {tr("import-progress-close")}
            }
        }
    }
}
#[component]
fn Frame(frame: Snapshot) -> Element {
    let hub = use_context::<Arc<Hub>>();
    let kind = match frame.context.kind {
        Kind::File => "import-progress-file",
        Kind::Range => "import-progress-range",
        Kind::Text => "import-progress-text",
        Kind::Saved => "import-progress-saved",
    };
    let character = if frame.context.kind == Kind::Saved {
        tr("import-progress-saved-character")
    } else {
        super::super::import_text::character::description(frame.context.viewpoint.as_ref())
    };
    rsx! {
        section { class: "space-y-4", "aria-label": tr("import-progress-title"),
            p { class: "break-words text-sm",
                {tr(kind)}
                " · #{frame.token.serial} · {character}"
            }
            p {
                role: if frame.outcome == Outcome::Failed { "alert" } else { "status" },
                class: "text-sm font-semibold",
                {tr(frame.outcome.key())}
            }
            if frame.outcome.pending() {
                Progress {
                    aria_label: tr("import-progress-title"),
                    max: frame.progress.total_bytes,
                    value: frame.progress.bytes_read,
                }
            }
            p { class: "text-sm tabular-nums",
                "{frame.progress.bytes_read} / {frame.progress.total_bytes} B"
            }
            p { class: "text-xs leading-6 text-muted-foreground", {tr("import-progress-read-help")} }
            p { class: "text-sm tabular-nums",
                "{frame.report.as_ref().map_or(frame.progress.records_dispatched,|report|report.records_dispatched)} / {frame.report.as_ref().map_or(frame.progress.lines_read,|report|report.lines_read)} "
                {tr("records")}
            }
            p { class: "text-xs tabular-nums text-muted-foreground",
                {tr("sources-progress-excluded")}
                " {frame.report.as_ref().map_or(frame.progress.companion_heads_excluded,|report|report.companion_heads_excluded)}"
            }
            if let Some(report) = frame.report {
                p { class: "text-sm tabular-nums",
                    "{report.encounters} "
                    {tr("encounters")}
                }
            }
            if frame.outcome.pending() {
                Button {
                    disabled: frame.outcome != Outcome::Running,
                    onclick: move |_| {
                        hub.cancel(frame.token);
                    },
                    {tr("import-progress-cancel")}
                }
            }
        }
    }
}

pub(super) fn destroy(window: &DesktopContext) {
    window.set_close_behavior(WindowCloseBehaviour::WindowCloses);
    window.close();
}
#[component]
fn Confirmation(target: Token, mut closing: Signal<CloseState>) -> Element {
    let hub = use_context::<Arc<Hub>>();
    let yes_hub = hub.clone();
    let restore = use_context::<Arc<AtomicBool>>();
    let yes_restore = restore.clone();
    let window = use_window();
    let yes_window = window.clone();
    rsx! {
        div {
            role: "group",
            "aria-label": tr("import-progress-confirm-title"),
            class: "space-y-3 rounded-md border border-border p-4",
            p { class: "text-sm font-semibold", {tr("import-progress-confirm-title")} }
            p { class: "text-sm leading-6 text-muted-foreground",
                {tr("import-progress-confirm-help")}
            }
            div { class: "flex flex-wrap gap-3",
                Button {
                    onclick: move |_| {
                        if let Some(visible) = answer_visibility(
                            &mut closing.write(),
                            &yes_hub,
                            target,
                            true,
                            &yes_restore,
                        ) {
                            yes_window.set_visible(visible);
                        }
                    },
                    {tr("import-progress-confirm-yes")}
                }
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: move |_| {
                        if let Some(visible) = answer_visibility(
                            &mut closing.write(),
                            &hub,
                            target,
                            false,
                            &restore,
                        ) {
                            window.set_visible(visible);
                        }
                    },
                    {tr("import-progress-confirm-no")}
                }
            }
        }
    }
}

fn restore_if_requested(window: &DesktopContext, restore: &AtomicBool) {
    if restore.swap(false, Ordering::SeqCst) {
        window.set_visible(true);
        window.set_focus();
    }
}

// A stale confirmation must not clear a newer native-X restore request or resurrect a hidden window.
pub(super) fn answer_visibility(
    closing: &mut CloseState,
    hub: &Hub,
    target: Token,
    yes: bool,
    restore: &AtomicBool,
) -> Option<bool> {
    let visible = match closing.answer(hub, target, yes) {
        CloseAction::Keep => true,
        CloseAction::Hide => false,
        _ => return None,
    };
    restore.store(false, Ordering::SeqCst);
    Some(visible)
}
