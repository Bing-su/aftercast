use super::{encounter_context::ContextBrowser, tr};
use aftercast::{EncounterId, HostHandle};
use dioxus::desktop::{Config, LogicalSize, WindowBuilder, WindowCloseBehaviour, use_window};
use dioxus::prelude::*;
use dioxus_i18n::{prelude::*, unic_langid::langid};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

// Shared plain data, never a Dioxus Signal: each window owns a separate VirtualDom.
#[derive(Default)]
pub(super) struct WindowLifetime {
    generation: AtomicU64,
    closed: AtomicBool,
}

impl WindowLifetime {
    pub(super) fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
    pub(super) fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }
    pub(super) fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
    fn valid(&self, generation: u64) -> bool {
        !self.closed.load(Ordering::SeqCst) && self.generation() == generation
    }
}

#[derive(Clone)]
pub(super) struct WindowAppearance {
    pub title: String,
    pub theme: String,
    pub language: String,
}

#[derive(Clone)]
pub(super) struct ContextEndpoint {
    pub handle: HostHandle,
    pub id: EncounterId,
    pub generation: u64,
    pub lifetime: Arc<WindowLifetime>,
    pub appearance: WindowAppearance,
}

impl ContextEndpoint {
    pub(super) fn valid(&self) -> bool {
        self.lifetime.valid(self.generation)
    }
}

#[derive(Clone, Copy, Default, PartialEq)]
pub(super) struct InitialPage {
    pub logs: bool,
    pub offset: u32,
    pub actions_offset: u32,
    pub source: Option<(u32, i32)>,
}

#[derive(Clone)]
struct LogWindowProps {
    endpoint: ContextEndpoint,
    initial: InitialPage,
}

pub(super) fn open(
    endpoint: ContextEndpoint,
    initial: InitialPage,
) -> Option<dioxus::desktop::PendingDesktopContext> {
    if !endpoint.valid() {
        return None;
    }
    let title = format!("aftercast — {}", endpoint.appearance.title);
    let dom = VirtualDom::new_with_props(log_window_root, LogWindowProps { endpoint, initial });
    let pending = dioxus::desktop::window().new_window(
        dom,
        Config::new()
            .with_menu(None)
            .with_custom_head("<link rel=\"icon\" href=\"data:,\">".into())
            .with_close_behaviour(WindowCloseBehaviour::WindowCloses)
            .with_window(
                WindowBuilder::new()
                    .with_title(title)
                    .with_inner_size(LogicalSize::new(960.0, 720.0))
                    .with_min_inner_size(LogicalSize::new(640.0, 480.0)),
            ),
    );
    // The caller awaits outside the WebView callback. The root close token also
    // covers a pending window that mounts after the main window closes.
    Some(pending)
}

fn log_window_root(props: LogWindowProps) -> Element {
    let endpoint = use_hook(|| props.endpoint.clone());
    use_context_provider(|| endpoint.clone());
    let appearance = endpoint.appearance.clone();
    let locale = try_use_init_i18n(|| {
        I18nConfig::new(appearance.language.parse().unwrap_or(langid!("en-US")))
            .with_fallback(langid!("en-US"))
            .with_locale((langid!("ko-KR"), include_str!("../../locales/ko-KR.ftl")))
            .with_locale((langid!("en-US"), include_str!("../../locales/en-US.ftl")))
    });
    let window = use_window();
    use_future(move || {
        let window = window.clone();
        let lifetime = endpoint.lifetime.clone();
        async move {
            while !lifetime.closed.load(Ordering::SeqCst) {
                dioxus_sdk::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            window.close();
        }
    });
    if let Err(error) = locale {
        return rsx! {
            pre { role: "alert", "{error}" }
        };
    }
    rsx! {
        style { {include_str!("../../assets/tailwind.css")} }
        main {
            class: "{appearance.theme} app-shell min-h-dvh bg-background p-6 text-foreground [color-scheme:dark] [&.latte]:[color-scheme:light]",
            "data-theme": appearance.theme,
            lang: appearance.language,
            div { class: "flex items-start justify-between gap-4",
                h1 { class: "text-xl font-semibold break-words", "{appearance.title}" }
                crate::components::Button {
                    variant: crate::components::ButtonVariant::Outline,
                    onclick: move |_| dioxus::desktop::window().close(),
                    {tr("close-log-window")}
                }
            }
            p { class: "mt-2 text-sm text-muted-foreground", {tr("log-window-context")} }
            ContextBrowser { initial: props.initial }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WindowLifetime;
    #[test]
    fn a_window_queued_during_restore_is_invalidated_at_completion() {
        let lifetime = WindowLifetime::default();
        let before = lifetime.generation();
        lifetime.invalidate();
        let during = lifetime.generation();
        lifetime.invalidate();
        assert!(!lifetime.valid(before));
        assert!(!lifetime.valid(during));
        assert!(lifetime.valid(lifetime.generation()));
    }

    #[test]
    fn restore_or_worker_change_invalidates_old_windows_without_rebinding_them() {
        let lifetime = WindowLifetime::default();
        let original = lifetime.generation();
        assert!(lifetime.valid(original));
        lifetime.invalidate();
        assert!(!lifetime.valid(original));
        let restored = lifetime.generation();
        assert!(lifetime.valid(restored));
        lifetime.close();
        assert!(!lifetime.valid(original));
        assert!(!lifetime.valid(restored));
        assert!(!lifetime.valid(lifetime.generation()));
    }
}
