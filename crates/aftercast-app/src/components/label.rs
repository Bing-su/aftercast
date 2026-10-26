// Adapt Rust/UI's label to required field associations, e.g. html_for: "language"; see ../../THIRD_PARTY.md.
use dioxus::prelude::*;
use tw_merge::tw_merge;

#[component]
pub fn Label(
    #[props(into)] html_for: String,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    let class = tw_merge!(
        "flex items-center gap-2 text-sm leading-none font-medium select-none peer-disabled:cursor-not-allowed peer-disabled:opacity-50",
        class.as_str()
    );
    rsx! {
        label { r#for: html_for, class, {children} }
    }
}
