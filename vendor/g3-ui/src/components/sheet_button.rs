//! SheetButton component - info button that opens a sheet with description.
use super::InfoButton;
use super::Sheet;
use crate::theme::ComponentMode;
use dioxus::prelude::*;
#[component]
pub fn SheetButton(
    description: String,
    class: Option<String>,
    mode: Option<ComponentMode>,
) -> Element {
    let mut is_open = use_signal(|| false);
    rsx! {
        InfoButton {
            onclick: move |event: Event<MouseData>| {
                event.stop_propagation();
                is_open.set(true);
            },
        }
        Sheet { is_open, class, mode, "{description}" }
    }
}
#[cfg(feature = "playground")]
#[component]
pub fn SheetButtonPlaygroundDemo() -> Element {
    let description = use_signal(|| "Handicaps adjust player scoring for the match.".to_string());
    rsx! {
        crate::PlaygroundDemoFrame {
            controls: rsx! {
                crate::Field { label: "Description".to_string(), value: description }
            },
            SheetButton { description: description() }
        }
    }
}
crate::g3_playground! {
    name: "SheetButton",
    description: "Information button that opens a sheet.",
    demo: SheetButtonPlaygroundDemo,
    source: "src/components/sheet_button.rs",
}
