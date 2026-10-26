// Use Rust/UI's outline badge for noninteractive host/plugin status, without introducing a second palette.
use dioxus::prelude::*;
use tw_merge::tw_merge;

#[component]
pub fn Badge(#[props(default)] class: String, children: Element) -> Element {
    let class = tw_merge!(
        "inline-flex w-fit items-center gap-2 rounded-md border border-border px-2.5 py-0.5 text-xs font-semibold text-foreground",
        class.as_str()
    );
    rsx! {
        span { class, {children} }
    }
}
