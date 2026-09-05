//! InfoButton component - info icon with optional click handler.
use super::info_button_styles as s;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::prelude::*;
use dioxus_icons::lucide::Info;
#[component]
pub fn InfoButton(
    onclick: Option<Callback<Event<MouseData>>>,
    aria_label: Option<String>,
    class: Option<String>,
    mode: Option<ComponentMode>,
) -> Element {
    let mode = use_component_mode(mode);
    let mode_cls = match mode {
        ComponentMode::Ios => s::IOS,
        ComponentMode::Md => s::MD,
    };
    rsx! {
        button {
            class: merge_classes(format!("{} {mode_cls}", s::BASE), class
                    .as_deref()),
            r#type: "button",
            aria_label: aria_label.unwrap_or_else(||
                    "More information".to_string()),
            onpointerdown: move |event| {
                event.stop_propagation();
            },
            onmousedown: move |event| {
                event.stop_propagation();
            },
            onclick: move |event| {
                event.stop_propagation();
                if let Some(ref onclick) = onclick {
                    onclick.call(event);
                }
            },
            Info { class: "text-focused fill-none", size: 24 }
        }
    }
}
