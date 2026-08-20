//! ConfirmModal component - confirmation dialog with platform styling.

use super::button_styles;
use crate::theme::{ComponentMode, use_component_mode};
use dioxus::prelude::*;

#[component]
pub fn ConfirmModal(
    mut open: Signal<bool>,
    title: String,
    description: Option<Element>,
    cancel_text: Option<String>,
    confirm_text: Option<String>,
    class: Option<String>,
    mode: Option<ComponentMode>,
    on_confirm: Callback<Event<MouseData>>,
) -> Element {
    let mode = use_component_mode(mode);
    let button_mode_cls = match mode {
        ComponentMode::Ios => button_styles::IOS,
        ComponentMode::Md => button_styles::MD,
    };
    let cancel_cls = format!(
        "{} {button_mode_cls} {} {} g3-modal-button g3-modal-cancel",
        button_styles::BASE,
        button_styles::OUTLINE,
        button_styles::MD_SIZE
    );
    let confirm_cls = format!(
        "{} {button_mode_cls} {} {} g3-modal-button g3-modal-confirm",
        button_styles::BASE,
        button_styles::SOLID,
        button_styles::MD_SIZE
    );

    rsx! {
        crate::components::Modal {
            open,
            title,
            description,
            class,
            mode,
            actions: rsx! {
                button { class: cancel_cls, r#type: "button", onclick: move |_| open.set(false),
                    span { class: "g3-modal-button-label",
                        if let Some(text) = cancel_text {
                            "{text}"
                        } else {
                            "Cancel"
                        }
                    }
                }
                button {
                    class: confirm_cls,
                    r#type: "button",
                    onclick: move |event| on_confirm.call(event),
                    span { class: "g3-modal-button-label",
                        if let Some(text) = confirm_text {
                            "{text}"
                        } else {
                            "Confirm"
                        }
                    }
                }
            },
        }
    }
}
#[cfg(feature = "playground")]
#[component]
pub fn ConfirmModalPlaygroundDemo() -> Element {
    let mut open = use_signal(|| false);
    let title = use_signal(|| "Delete game".to_string());
    let confirm_text = use_signal(|| "Delete".to_string());
    rsx! {
        crate::PlaygroundDemoFrame {
            controls: rsx! {
                crate::Field { label: "Title".to_string(), value: title }
                crate::Field { label: "Confirm text".to_string(), value: confirm_text }
            },
            crate::Button { onclick: move |_| open.set(true), "Open confirmation" }
            ConfirmModal {
                open,
                title: title(),
                confirm_text: confirm_text(),
                description: rsx! { "This action cannot be undone." },
                on_confirm: move |_| open.set(false),
            }
        }
    }
}
crate::g3_playground! {
    name: "ConfirmModal",
    description: "Accessible confirmation dialog.",
    demo: ConfirmModalPlaygroundDemo,
    source: "src/components/confirm_modal.rs",
}
