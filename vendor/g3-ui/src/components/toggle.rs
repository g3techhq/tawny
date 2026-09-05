//! Toggle (switch) component with iOS/Android styling.
use super::toggle_styles as s;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::prelude::*;
/// Size of a toggle switch.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum ToggleSize {
    /// Compact, for list rows and settings tables.
    #[default]
    Sm,
    /// Standard size.
    Md,
}
#[component]
pub fn Toggle(
    mut checked: Signal<bool>,
    onchange: Option<Callback<bool>>,
    size: Option<ToggleSize>,
    class: Option<String>,
    mode: Option<ComponentMode>,
) -> Element {
    let mode = use_component_mode(mode);
    let switch_cls = match mode {
        ComponentMode::Ios => format!("{} {}", s::SWITCH, s::SWITCH_IOS),
        ComponentMode::Md => format!("{} {}", s::SWITCH, s::SWITCH_MD),
    };
    let thumb_cls = match mode {
        ComponentMode::Ios => format!("{} {}", s::THUMB, s::THUMB_IOS),
        ComponentMode::Md => format!("{} {}", s::THUMB, s::THUMB_MD),
    };
    let size_cls = if size.unwrap_or_default() == ToggleSize::Sm {
        s::SM
    } else {
        ""
    };
    rsx! {
        button {
            class: merge_classes(
                format!("{switch_cls} {size_cls} {}", if checked() { "checked" } else { "" }),
                class.as_deref(),
            ),
            r#type: "button",
            role: "switch",
            aria_checked: checked().to_string(),
            onclick: move |_| {
                let new_checked = !checked();
                checked.set(new_checked);
                if let Some(ref onchange) = onchange {
                    onchange.call(new_checked);
                }
            },
            span { class: thumb_cls, aria_hidden: "true" }
        }
    }
}
#[cfg(feature = "playground")]
#[component]
pub fn TogglePlaygroundDemo() -> Element {
    let checked = use_signal(|| true);
    let large = use_signal(|| false);
    rsx! {
        crate::PlaygroundDemoFrame {
            controls: rsx! {
                crate::Checkbox { checked, label: "Checked".to_string() }
                crate::Checkbox { checked: large, label: "Large"
                            .to_string() }
            },
            Toggle { checked, size: if large() { ToggleSize::Md } else { ToggleSize::Sm } }
        }
    }
}
crate::g3_playground! {
    name: "Toggle",
    description: "Platform-styled switch control.",
    demo: TogglePlaygroundDemo,
    source: "src/components/toggle.rs",
}
