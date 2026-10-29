use super::{ContextEndpoint, TimelineReport, logs_at};
use crate::components::{Button, ButtonVariant};
use dioxus::desktop::{Config, LogicalSize, WindowBuilder, WindowCloseBehaviour};
use dioxus::prelude::*;
use std::sync::Arc;

const PAGE_SIZE: usize = 100;

#[derive(Clone, Copy)]
struct LogView {
    time: f64,
    width: f64,
    start: usize,
}
impl LogView {
    fn new(report: &TimelineReport, time: f64, width: f64) -> Self {
        let mut view = Self {
            time,
            width,
            start: 0,
        };
        view.select(report, time);
        view
    }
    fn select(&mut self, report: &TimelineReport, time: f64) {
        self.time = time;
        self.start = logs_at(report, time, self.width)
            .first()
            .copied()
            .unwrap_or(0);
    }
    fn matches(&self, seconds: f64) -> bool {
        seconds >= self.time && seconds < self.time + self.width
    }
}

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
    let mut view = use_signal(|| LogView::new(&report, props.time, props.width));
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
    let current = view();
    let start = current.start;
    let end_time = current.time + current.width;
    let max_time = f64::from(report.points) * current.width;
    let has_matches = !logs_at(&report, current.time, current.width).is_empty();
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
                        "Retained encounter logs: {report.logs.len()}. Omitted by retention limit: {report.dropped_logs}. This is not the complete game log."
                    }
                    if report.summary.status == 3 {
                        p { role: "note",
                            "This encounter is a partial import; retained logs may be incomplete."
                        }
                    }
                    p { "Highlighted range: +{current.time}s to +{end_time}s (end excluded)." }
                    div { class: "flex gap-2",
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: current.time <= current.width,
                            onclick: {
                                let report = report.clone();
                                move |_| {
                                    let mut next = view();
                                    next.select(&report, (next.time - next.width).max(next.width));
                                    view.set(next);
                                }
                            },
                            "Previous range"
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: current.time >= max_time,
                            onclick: {
                                let report = report.clone();
                                move |_| {
                                    let mut next = view();
                                    next.select(&report, (next.time + next.width).min(max_time));
                                    view.set(next);
                                }
                            },
                            "Next range"
                        }
                    }
                    if !has_matches {
                        p { "No retained logs in this range." }
                    }
                    for log in report.logs.iter().skip(start).take(PAGE_SIZE) {
                        p { class: if current.matches(log.seconds) { "whitespace-pre-wrap bg-accent text-accent-foreground" } else { "whitespace-pre-wrap" },
                            if current.matches(log.seconds) {
                                "[Selected] "
                            }
                            "{log.time}: {log.text}"
                        }
                    }
                    div { class: "flex gap-2",
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: start == 0,
                            onclick: move |_| {
                                let mut next = view();
                                next.start = next.start.saturating_sub(PAGE_SIZE);
                                view.set(next);
                            },
                            "Previous"
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            disabled: start.saturating_add(PAGE_SIZE) >= report.logs.len(),
                            onclick: move |_| {
                                let mut next = view();
                                next.start = next.start.saturating_add(PAGE_SIZE);
                                view.set(next);
                            },
                            "Next"
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
pub(super) fn verify_retained_view(report: &TimelineReport) {
    let mut sample = report.clone();
    let original = sample.logs.first().expect("fixture log").clone();
    sample.logs = (0..205)
        .map(|i| {
            let mut line = original.clone();
            line.seconds = if i == 150 || i == 151 {
                10.0
            } else if i == 152 {
                11.0
            } else {
                0.0
            };
            line
        })
        .collect();
    sample.dropped_logs = 9;
    let mut first = LogView::new(&sample, 10.0, 1.0);
    let other = LogView::new(&sample, 10.0, 1.0);
    assert_eq!(first.start, 150);
    assert!(first.matches(sample.logs[150].seconds));
    assert!(first.matches(sample.logs[151].seconds));
    assert!(!first.matches(sample.logs[152].seconds));
    assert_eq!(sample.logs.len(), 205);
    assert_eq!(sample.dropped_logs, 9);
    first.select(&sample, 11.0);
    assert_eq!(first.start, 152);
    assert!(first.matches(sample.logs[152].seconds));
    assert!(other.matches(sample.logs[150].seconds));
    first.select(&sample, 99.0);
    assert_eq!(first.start, 0);
    assert!(logs_at(&sample, first.time, first.width).is_empty());
    let reopened = LogView::new(&sample, 10.0, 1.0);
    assert_eq!(reopened.start, 150);
}
