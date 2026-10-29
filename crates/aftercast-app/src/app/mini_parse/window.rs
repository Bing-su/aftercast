use super::*;
use crate::components::{Button, ButtonVariant};
use dioxus::desktop::{Config, LogicalSize, WindowBuilder, WindowCloseBehaviour, use_window};
use dioxus_i18n::{prelude::*, unic_langid::langid};

#[derive(Clone)]
struct Props {
    frame: Arc<Mutex<Frame>>,
    alive: Arc<AtomicBool>,
    lifetime: Arc<super::super::log_window::WindowLifetime>,
    generation: u64,
    appearance: super::super::log_window::WindowAppearance,
}
pub(super) fn open(mut state: Signal<AppState>) {
    if state.peek().phase != Phase::Ready || state.peek().exit_requested {
        return;
    }
    let (serial, props) = {
        let mut current = state.write();
        if current.mini_parse.window_open() {
            if let Some(existing) = current.mini_parse.window.desktop.upgrade() {
                existing.set_visible(true);
                existing.set_focus();
            }
            return;
        }
        if current.mini_parse.window.pending.is_some() {
            return;
        }
        // A closed window's accepted work must not populate its replacement.
        if current.mini_parse.window.observed {
            current.mini_parse.invalidate();
        }
        let Some(serial) = current.mini_parse.window.reserve() else {
            return;
        };
        current.mini_parse.mini_style = current.settings.appearance.mini_style();
        current.mini_parse.publish();
        let appearance = super::super::log_window::WindowAppearance {
            title: tr("mini-title"),
            theme: current.settings.ui.theme.clone(),
            language: current.settings.ui.language.clone(),
        };
        let lifetime = current.log_windows.clone();
        let props = Props {
            frame: current.mini_parse.window.frame.clone(),
            alive: current.mini_parse.window.alive.clone(),
            generation: lifetime.generation(),
            lifetime,
            appearance,
        };
        (serial, props)
    };
    let title = format!("aftercast — {}", props.appearance.title);
    let pending = dioxus::desktop::window().new_window(
        VirtualDom::new_with_props(root, props.clone()),
        Config::new()
            .with_menu(None)
            .with_custom_head("<link rel=\"icon\" href=\"data:,\">".into())
            .with_close_behaviour(WindowCloseBehaviour::WindowCloses)
            .with_window(
                WindowBuilder::new()
                    .with_title(title)
                    .with_inner_size(LogicalSize::new(640.0, 480.0))
                    .with_min_inner_size(LogicalSize::new(360.0, 240.0)),
            ),
    );
    spawn_forever(async move {
        let result = pending.try_resolve().await;
        let mut current = state.write();
        let valid = props.lifetime.valid(props.generation);
        if !current
            .mini_parse
            .window
            .resolve(serial, valid && result.is_ok())
            || !valid
        {
            if let Ok(window) = result {
                window.close();
            }
            return;
        }
        match result {
            Ok(window) => current.mini_parse.window.desktop = std::rc::Rc::downgrade(&window),
            Err(_) => {
                props.alive.store(false, Ordering::SeqCst);
                current.mini_parse.window.observed = false;
                if !current.mini_parse.enabled {
                    current.mini_parse.invalidate();
                }
                current.mini_parse.error = Some(tr("mini-window-error"));
                current.mini_parse.publish();
            }
        }
    });
}
async fn observe(
    mut frame: Signal<Frame>,
    hub: Arc<Mutex<Frame>>,
    alive: Arc<AtomicBool>,
    lifetime: Arc<super::super::log_window::WindowLifetime>,
    generation: u64,
    desktop: dioxus::desktop::DesktopContext,
) {
    while alive.load(Ordering::SeqCst) && lifetime.valid(generation) {
        frame.set(
            hub.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        );
        dioxus_sdk::time::sleep(Duration::from_millis(100)).await;
    }
    desktop.close();
}
fn root(props: Props) -> Element {
    let appearance = props.appearance;
    let locale = try_use_init_i18n(|| {
        I18nConfig::new(appearance.language.parse().unwrap_or(langid!("en-US")))
            .with_fallback(langid!("en-US"))
            .with_locale((langid!("ko-KR"), include_str!("../../../locales/ko-KR.ftl")))
            .with_locale((langid!("en-US"), include_str!("../../../locales/en-US.ftl")))
    });
    let alive = use_hook(|| props.alive.clone());
    let closed = alive.clone();
    use_drop(move || closed.store(false, Ordering::SeqCst));
    let frame = use_signal(Frame::default);
    let hub = props.frame.clone();
    let lifetime = props.lifetime.clone();
    let generation = props.generation;
    let desktop = use_window();
    use_future(move || {
        observe(
            frame,
            hub.clone(),
            alive.clone(),
            lifetime.clone(),
            generation,
            desktop.clone(),
        )
    });
    if let Err(error) = locale {
        return rsx! {
            pre { role: "alert", "{error}" }
        };
    }
    let current = frame.read().clone();
    rsx! {
        style { {include_str!("../../../assets/tailwind.css")} }
        main {
            class: "{appearance.theme} app-shell min-h-dvh space-y-3 bg-background p-4 text-foreground [color-scheme:dark] [&.latte]:[color-scheme:light]",
            lang: appearance.language,
            div { class: "flex items-center justify-between gap-3",
                h1 { class: "text-lg font-semibold", {tr("mini-title")} }
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: move |_| dioxus::desktop::window().close(),
                    {tr("close-log-window")}
                }
            }
            if let Some(error) = current.error {
                p { class: "text-sm text-destructive",
                    {if error == "report-sort-missing" { tr("report-sort-missing") } else { error }}
                }
            }
            pre {
                class: "max-h-[70vh] overflow-auto whitespace-pre-wrap rounded-md border border-border p-3 text-sm",
                style: current.style,
                {
                    match current.output {
                        Some(text) if text.is_empty() => tr("mini-empty-output"),
                        Some(text) => text,
                        None if current.pending => tr("mini-loading"),
                        None => tr("mini-empty"),
                    }
                }
            }
            if current.truncated {
                p { class: "text-xs text-muted-foreground", {tr("mini-truncated")} }
            }
        }
    }
}
