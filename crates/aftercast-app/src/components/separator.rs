// Keep Rust/UI's horizontal separator for settings groups; omit the unused vertical variant.
use dioxus::prelude::*;
use tw_merge::tw_merge;

#[component]
pub fn Separator(#[props(default)] class: String) -> Element {
    let class = tw_merge!("h-px w-full shrink-0 bg-border", class.as_str());
    rsx! {
        div { class, role: "separator", "aria-orientation": "horizontal" }
    }
}
