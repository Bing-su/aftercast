use super::{ContextEndpoint, TimelineReport, logs_at};
use crate::components::{Button, ButtonVariant};
use dioxus::desktop::{Config, LogicalSize, WindowBuilder, WindowCloseBehaviour};
use dioxus::prelude::*;
use std::sync::Arc;

#[derive(Clone)]
struct Props {
    endpoint: ContextEndpoint,
    report: Arc<TimelineReport>,
    time: f64,
    width: f64,
}
pub(super) fn open(
    endpoint: ContextEndpoint,
    report: Arc<TimelineReport>,
    time: f64,
    width: f64,
) -> Option<dioxus::desktop::PendingDesktopContext> {
    if !endpoint.valid() {
        return None;
    }
    let dom = VirtualDom::new_with_props(
        root,
        Props {
            endpoint,
            report,
            time,
            width,
        },
    );
    Some(
        dioxus::desktop::window().new_window(
            dom,
            Config::new()
                .with_menu(None)
                .with_close_behaviour(WindowCloseBehaviour::WindowCloses)
                .with_window(
                    WindowBuilder::new()
                        .with_title("aftercast — timeline logs")
                        .with_inner_size(LogicalSize::new(960.0, 720.0)),
                ),
        ),
    )
}
fn root(props: Props) -> Element {
    let endpoint = use_hook(|| props.endpoint.clone());
    let report = use_hook(|| props.report.clone());
    let indices = use_hook(|| logs_at(&report, props.time, props.width));
    let mut page = use_signal(|| 0_usize);
    let mut valid = use_signal(|| endpoint.valid());
    let window = dioxus::desktop::use_window();
    use_future(move || {
        let endpoint = endpoint.clone();
        let window = window.clone();
        async move {
            while endpoint.valid() {
                dioxus_sdk::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            valid.set(false);
            window.close();
        }
    });
    let start = page().saturating_mul(100);
    let end_time = props.time + props.width;
    let theme = props.endpoint.appearance.theme.clone();
    rsx! {
        style { {include_str!("../../../assets/tailwind.css")} }
        main {
            class: "{theme} app-shell min-h-dvh bg-background p-6 text-foreground [color-scheme:dark] [&.latte]:[color-scheme:light]",
            "data-theme": theme,
            div { class: "p-4 space-y-2",
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: move |_| dioxus::desktop::window().close(),
                    "Close"
                }
                if !valid() {
                    p { role: "alert", "Session ended or changed." }
                } else {
                    p {
                        "Frozen range: +{props.time}s to +{end_time}s (end excluded). Omitted logs: {report.dropped_logs}."
                    }
                    if indices.is_empty() {
                        p { "No retained logs in this range." }
                    }
                    for index in indices.iter().skip(start).take(100) {
                        if let Some(log) = report.logs.get(*index) {
                            p { class: "whitespace-pre-wrap", "{log.time}: {log.text}" }
                        }
                    }
                    div { class: "flex gap-2",
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: start == 0,
                            onclick: move |_| page.set(page().saturating_sub(1)),
                            "Previous"
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: start.saturating_add(100) >= indices.len(),
                            onclick: move |_| page.set(page().saturating_add(1)),
                            "Next"
                        }
                    }
                }
            }
        }
    }
}
