//! Select component - dropdown picker using bottom sheet.

use super::Sheet;
use super::select_styles as s;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::prelude::*;
use dioxus_icons::lucide::ChevronDown;

/// One option in a `G3Select`. Construct with `SelectOption::new`, then
/// optionally give it display text that differs from its value.
#[derive(Clone, PartialEq)]
pub struct SelectOption {
    option_value: String,
    text: Option<String>,
}

impl From<&str> for SelectOption {
    fn from(value: &str) -> Self {
        Self {
            option_value: value.to_string(),
            text: None,
        }
    }
}

impl From<String> for SelectOption {
    fn from(value: String) -> Self {
        Self {
            option_value: value,
            text: None,
        }
    }
}

impl From<(&str, &str)> for SelectOption {
    fn from(value: (&str, &str)) -> Self {
        Self {
            option_value: value.0.to_string(),
            text: Some(value.1.to_string()),
        }
    }
}

impl From<(String, String)> for SelectOption {
    fn from(value: (String, String)) -> Self {
        Self {
            option_value: value.0,
            text: Some(value.1),
        }
    }
}

#[component]
pub fn Select(
    value: Signal<String>,
    disabled: Option<bool>,
    mode: Option<ComponentMode>,
    options: Vec<SelectOption>,
    class: Option<String>,
    onchange: Option<EventHandler<String>>,
) -> Element {
    let mut is_open = use_signal(|| false);
    let resolved_mode = use_component_mode(mode);
    let mode_cls = match resolved_mode {
        ComponentMode::Ios => s::SELECT_BTN_IOS,
        ComponentMode::Md => s::SELECT_BTN_MD,
    };

    rsx! {
        button {
            class: merge_classes(format!("{} {mode_cls}", s::SELECT_BTN), class.as_deref()),
            r#type: "button",
            aria_haspopup: "listbox",
            aria_expanded: is_open().to_string(),
            disabled: disabled.unwrap_or_default(),
            onclick: move |_| is_open.set(true),
            span { class: s::SELECT_VALUE, "{value()}" }
            ChevronDown { class: "fill-gray ml-1 shrink-0", size: 16 }
        }
        Sheet {
            is_open,
            mode,
            class: s::SELECT_SHEET,
            draggable: false,
            div { class: "flex flex-col w-full", role: "listbox",
                for (index , option) in options.into_iter().enumerate() {
                    SelectOptionComponent {
                        value,
                        is_open,
                        option_value: option.option_value,
                        text: option.text,
                        is_first: index == 0,
                        onchange,
                    }
                }
            }
        }
    }
}

#[component]
fn SelectOptionComponent(
    mut value: Signal<String>,
    is_open: Signal<bool>,
    option_value: String,
    text: Option<String>,
    is_first: bool,
    onchange: Option<EventHandler<String>>,
) -> Element {
    let is_selected = value() == option_value;
    let option_cls = merge_classes(s::OPTION, is_selected.then_some(s::OPTION_SELECTED));

    rsx! {
        if !is_first {
            div {
                class: s::SEPARATOR,
                role: "separator",
                aria_orientation: "horizontal",
            }
        }
        button {
            class: option_cls,
            r#type: "button",
            role: "option",
            aria_selected: is_selected.to_string(),
            onclick: move |_| {
                value.set(option_value.clone());
                if let Some(onchange) = onchange {
                    onchange.call(option_value.clone());
                }
                is_open.set(false);
            },
            if let Some(text) = text {
                "{text}"
            } else {
                "{option_value}"
            }
        }
    }
}
#[cfg(feature = "playground")]
#[component]
pub fn SelectPlaygroundDemo() -> Element {
    let value = use_signal(|| "Stroke".to_string());
    let disabled = use_signal(|| false);
    rsx! {
        crate::PlaygroundDemoFrame {
            controls: rsx! {
                crate::Checkbox { checked: disabled, label: "Disabled".to_string() }
            },
            Select {
                value,
                disabled: disabled(),
                options: vec![
                    SelectOption::from(("Stroke", "Stroke play")),
                    SelectOption::from(("Match", "Match play")),
                    SelectOption::from(("Skins", "Skins")),
                ],
            }
        }
    }
}
crate::g3_playground! {
    name: "Select",
    description: "Button-triggered picker backed by a sheet.",
    demo: SelectPlaygroundDemo,
    source: "src/components/select.rs",
}
