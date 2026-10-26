// Adapt Rust/UI's progress to byte counts and a named accessible control, e.g. an asynchronous log import.
use dioxus::prelude::*;
use tw_merge::tw_merge;

#[component]
pub fn Progress(
    value: u64,
    max: u64,
    #[props(into)] aria_label: String,
    #[props(default)] class: String,
) -> Element {
    let max = max.max(1);
    let value = value.min(max);
    let percentage = value as f64 / max as f64 * 100.0;
    let class = tw_merge!(
        "relative h-2 w-full overflow-hidden rounded-full bg-accent",
        class.as_str()
    );
    rsx! {
        div {
            class,
            role: "progressbar",
            "aria-label": aria_label,
            aria_valuemin: "0",
            aria_valuemax: "{max}",
            aria_valuenow: "{value}",
            div {
                class: "h-full w-full bg-primary transition-transform duration-300 motion-reduce:transition-none",
                style: "transform: translateX(-{100.0 - percentage}%)",
            }
        }
    }
}
