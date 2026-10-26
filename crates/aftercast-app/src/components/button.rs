// Adapt Rust/UI's button to desktop-only actions, avoiding unused router and pointer-event features.
// Source and MIT license: ../../THIRD_PARTY.md.
use dioxus::prelude::*;
use tw_merge::tw_merge;

#[derive(Default, Clone, PartialEq)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Outline,
    Ghost,
}

#[component]
pub fn Button(
    #[props(default)] variant: ButtonVariant,
    #[props(default)] disabled: bool,
    #[props(default)] class: String,
    #[props(default)] id: String,
    #[props(default)] aria_label: Option<String>,
    #[props(default)] onclick: EventHandler<MouseEvent>,
    children: Element,
) -> Element {
    let variant = match variant {
        ButtonVariant::Primary => "bg-primary text-primary-foreground hover:brightness-95",
        ButtonVariant::Outline => {
            "border border-input bg-background text-foreground hover:bg-accent"
        }
        ButtonVariant::Ghost => "text-muted-foreground hover:bg-accent hover:text-foreground",
    };
    let class = tw_merge!(
        "inline-flex h-9 shrink-0 items-center justify-center gap-2 rounded-md px-3 text-sm font-medium transition-colors motion-reduce:transition-none disabled:pointer-events-none disabled:opacity-45 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring [&_svg]:size-4",
        variant,
        class.as_str()
    );
    rsx! {
        button {
            id,
            class,
            r#type: "button",
            "aria-label": aria_label,
            disabled,
            onclick,
            {children}
        }
    }
}
