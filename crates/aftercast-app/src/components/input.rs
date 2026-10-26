// Adapt Rust/UI's input to the text fields used here; keep source provenance in ../../THIRD_PARTY.md.
use dioxus::prelude::*;
use tw_merge::tw_merge;

#[component]
pub fn Input(
    id: String,
    value: String,
    #[props(default)] disabled: bool,
    #[props(default)] placeholder: String,
    #[props(default)] class: String,
    oninput: EventHandler<FormEvent>,
) -> Element {
    let class = tw_merge!(
        "flex h-9 w-full min-w-0 rounded-md border border-input bg-background px-3 text-sm text-foreground placeholder:text-muted-foreground disabled:opacity-45 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring",
        class.as_str()
    );
    rsx! {
        input {
            id,
            class,
            r#type: "text",
            value,
            placeholder,
            disabled,
            oninput,
        }
    }
}
