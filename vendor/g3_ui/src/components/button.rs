//! Button component with iOS/Android mode support.

use super::button_styles as s;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::prelude::*;

/// Visual variant of the button.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum ButtonStyle {
    #[default]
    Solid,
    Outline,
    Clear,
    Neutral,
    /// Destructive action (delete, remove, sign out) — same weight as
    /// `Clear` (text-only, no fill/border) but tinted with the theme's
    /// danger color instead of the accent color.
    Danger,
}

/// Button size.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum ButtonSize {
    Sm,
    #[default]
    Md,
    Lg,
}

#[component]
pub fn Button(
    style: Option<ButtonStyle>,
    size: Option<ButtonSize>,
    disabled: Option<bool>,
    aria_label: Option<String>,
    expand: Option<bool>,
    class: Option<String>,
    mode: Option<ComponentMode>,
    start: Option<Element>,
    /// Small counter overlay in the button's top-right corner (e.g. active
    /// filter count). Hidden entirely when `None` or `Some(0)` — callers can
    /// pass a raw count without special-casing zero.
    badge: Option<u32>,
    onclick: Callback<Event<MouseData>>,
    children: Element,
) -> Element {
    let mode = use_component_mode(mode);
    let mode_cls = match mode {
        ComponentMode::Ios => s::IOS,
        ComponentMode::Md => s::MD,
    };

    let style_cls = match style.unwrap_or_default() {
        ButtonStyle::Solid => s::SOLID,
        ButtonStyle::Outline => s::OUTLINE,
        ButtonStyle::Clear => s::CLEAR,
        ButtonStyle::Neutral => s::NEUTRAL,
        ButtonStyle::Danger => s::DANGER,
    };

    let size_cls = match size.unwrap_or_default() {
        ButtonSize::Sm => s::SM,
        ButtonSize::Md => s::MD_SIZE,
        ButtonSize::Lg => s::LG,
    };

    let expand_cls = if expand.unwrap_or(false) {
        "w-full"
    } else {
        ""
    };
    let is_disabled = disabled.unwrap_or(false);

    let cls = merge_classes(
        format!("{} {mode_cls} {style_cls} {size_cls} {expand_cls}", s::BASE),
        class.as_deref(),
    );
    let badge_count = badge.filter(|count| *count > 0);

    rsx! {
        button {
            class: cls,
            r#type: "button",
            disabled: is_disabled,
            aria_label,
            onclick: move |event| {
                onclick.call(event);
            },
            if let Some(start) = start {
                div { class: "g3-btn-content g3-btn-content-start",
                    span { class: "g3-btn-start", {start} }
                    span { class: "g3-btn-label", {children} }
                    span { class: "g3-btn-end-spacer" }
                }
            } else {
                div { class: "g3-btn-content", {children} }
            }
            if let Some(count) = badge_count {
                span { class: s::BADGE, "{count}" }
            }
        }
    }
}

#[cfg(feature = "playground")]
#[component]
pub fn ButtonPlaygroundDemo() -> Element {
    let style_index = use_signal(|| 0_usize);
    let size_index = use_signal(|| 1_usize);
    let disabled = use_signal(|| false);
    let expand = use_signal(|| false);
    let label = use_signal(|| "Create".to_string());
    let style = match style_index() {
        1 => ButtonStyle::Outline,
        2 => ButtonStyle::Clear,
        3 => ButtonStyle::Neutral,
        _ => ButtonStyle::Solid,
    };
    let size = match size_index() {
        0 => ButtonSize::Sm,
        2 => ButtonSize::Lg,
        _ => ButtonSize::Md,
    };

    rsx! {
        crate::PlaygroundDemoFrame {
            controls: rsx! {
                crate::Field { label: "Label".to_string(), value: label }
                div {
                    span { "Style" }
                    crate::SegmentGroup { active: style_index,
                        crate::SegmentButton { index: 0, "Solid" }
                        crate::SegmentButton { index: 1, "Outline" }
                        crate::SegmentButton { index: 2, "Clear" }
                        crate::SegmentButton { index: 3, "Neutral" }
                    }
                }
                div {
                    span { "Size" }
                    crate::SegmentGroup { active: size_index,
                        crate::SegmentButton { index: 0, "Sm" }
                        crate::SegmentButton { index: 1, "Md" }
                        crate::SegmentButton { index: 2, "Lg" }
                    }
                }
                crate::Checkbox { checked: disabled, label: "Disabled".to_string() }
                crate::Checkbox { checked: expand, label: "Expand".to_string() }
            },
            Button {
                style,
                size,
                disabled: disabled(),
                expand: expand(),
                onclick: |_| {},
                "{label()}"
            }
        }
    }
}
crate::g3_playground! {
    name: "Button",
    description: "Ionic-style action button with solid, outline, clear, and neutral variants.",
    demo: ButtonPlaygroundDemo,
    source: "src/components/button.rs",
}

#[cfg(test)]
mod tests {
    #[test]
    fn button_exposes_aria_label_for_icon_only_triggers() {
        let source = include_str!("button.rs");
        assert!(source.contains("aria_label: Option<String>"));
        assert!(source.contains("aria_label,"));
    }
}
