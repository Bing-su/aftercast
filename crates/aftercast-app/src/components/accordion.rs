// Adapt Rust/UI's disclosure styling to button semantics, e.g. Enter and Space toggle the panel.
// Source and MIT license: ../../THIRD_PARTY.md.
use std::sync::atomic::{AtomicUsize, Ordering};

use dioxus::prelude::*;
use tw_merge::tw_merge;

static NEXT_ACCORDION: AtomicUsize = AtomicUsize::new(0);

#[component]
pub fn Accordion(
    header: Element,
    children: Element,
    #[props(default)] default_open: bool,
    #[props(default)] class: String,
    #[props(default)] trigger_class: String,
) -> Element {
    let mut expanded = use_signal(|| default_open);
    let id = use_hook(|| NEXT_ACCORDION.fetch_add(1, Ordering::Relaxed));
    let trigger_class = tw_merge!(
        "accordion-trigger flex min-h-9 w-full items-center gap-2 rounded-md px-2 py-2 text-left text-sm font-medium hover:bg-accent focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-ring",
        trigger_class.as_str()
    );
    rsx! {
        div { class: "accordion {class}",
            button {
                id: "accordion-{id}-trigger",
                r#type: "button",
                class: trigger_class,
                "aria-expanded": expanded(),
                "aria-controls": "accordion-{id}-panel",
                onclick: move |_| expanded.toggle(),
                span { class: "min-w-0 flex-1", {header} }
                span {
                    class: "accordion-chevron shrink-0 text-muted-foreground transition-transform duration-150 motion-reduce:transition-none",
                    "aria-hidden": "true",
                    icons::ChevronRight { class: "size-4" }
                }
            }
            div {
                id: "accordion-{id}-panel",
                class: "accordion-content",
                hidden: !expanded(),
                {children}
            }
        }
    }
}
