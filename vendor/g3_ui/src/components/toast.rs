//! Toast component for transient mobile feedback.

use super::toast_styles as s;
use crate::components::StatusColor;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::prelude::*;
use dioxus_icons::lucide::X;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastPosition {
    Top,
    Middle,
    #[default]
    Bottom,
}

impl ToastPosition {
    fn class(self) -> &'static str {
        match self {
            Self::Top => s::POSITION_TOP,
            Self::Middle => s::POSITION_MIDDLE,
            Self::Bottom => s::POSITION_BOTTOM,
        }
    }
}

fn color_class(color: StatusColor) -> &'static str {
    match color {
        StatusColor::Neutral => s::COLOR_NEUTRAL,
        StatusColor::Accent => s::COLOR_ACCENT,
        StatusColor::Success => s::COLOR_SUCCESS,
        StatusColor::Warning => s::COLOR_WARNING,
        StatusColor::Danger => s::COLOR_DANGER,
    }
}

#[component]
pub fn Toast(
    mut open: Signal<bool>,
    message: String,
    position: Option<ToastPosition>,
    color: Option<StatusColor>,
    duration_ms: Option<u64>,
    close_label: Option<String>,
    action: Option<Element>,
    class: Option<String>,
    mode: Option<ComponentMode>,
    on_dismiss: Option<Callback<()>>,
) -> Element {
    let mode = use_component_mode(mode);
    let color = color.unwrap_or_default();
    let position = position.unwrap_or_default();
    let close_label = close_label.unwrap_or_else(|| "Dismiss".to_string());
    let role = if matches!(color, StatusColor::Danger | StatusColor::Warning) {
        "alert"
    } else {
        "status"
    };
    let mode_cls = match mode {
        ComponentMode::Ios => s::TOAST_IOS,
        ComponentMode::Md => s::TOAST_MD,
    };
    let state = if open() { "open" } else { "closed" };
    let auto_dismiss_ms = duration_ms.unwrap_or(3000);
    let timer_state = if auto_dismiss_ms == 0 {
        "none"
    } else {
        "active"
    };
    let closed_inert = (!open()).then(|| "".to_string());
    let mut dismiss_generation = use_signal(|| 0_u64);

    use_effect(move || {
        let generation = dismiss_generation.with_mut(|value| {
            *value += 1;
            *value
        });
        if !open() {
            return;
        }
        if auto_dismiss_ms == 0 {
            return;
        }
        spawn(async move {
            dioxus_sdk_time::sleep(Duration::from_millis(auto_dismiss_ms)).await;
            if dismiss_generation() == generation && open() {
                open.set(false);
                if let Some(on_dismiss) = on_dismiss {
                    on_dismiss.call(());
                }
            }
        });
    });

    rsx! {
        div {
            class: merge_classes(
                format!("{} {mode_cls} {} {}", s::TOAST, position.class(), color_class(color)),
                class.as_deref(),
            ),
            role,
            aria_live: if role == "alert" { "assertive" } else { "polite" },
            aria_hidden: (!open()).to_string(),
            inert: closed_inert,
            "data-state": state,
            "data-timer": timer_state,
            style: format!("--g3-toast-duration: {auto_dismiss_ms}ms;"),
            span { class: s::INDICATOR, aria_hidden: "true" }
            div { class: s::MESSAGE, "{message}" }
            if let Some(action) = action {
                div { class: s::ACTION, {action} }
            }
            button {
                class: s::CLOSE,
                r#type: "button",
                aria_label: close_label.clone(),
                disabled: !open(),
                onclick: move |_| {
                    dismiss_generation.with_mut(|value| *value += 1);
                    open.set(false);
                    if let Some(on_dismiss) = on_dismiss {
                        on_dismiss.call(());
                    }
                },
                X { class: s::CLOSE_ICON, size: 18 }
            }
            div { class: s::TIMER, aria_hidden: "true" }
        }
    }
}

#[cfg(feature = "playground")]
#[component]
pub fn ToastPlaygroundDemo() -> Element {
    let mut open = use_signal(|| true);
    let duration_index = use_signal(|| 1_usize);
    let position_index = use_signal(|| 2_usize);
    let color_index = use_signal(|| 1_usize);
    let duration_ms = match duration_index() {
        0 => 1500,
        2 => 5000,
        3 => 0,
        _ => 2500,
    };
    let position = match position_index() {
        0 => ToastPosition::Top,
        1 => ToastPosition::Middle,
        _ => ToastPosition::Bottom,
    };
    let color = match color_index() {
        0 => StatusColor::Neutral,
        1 => StatusColor::Accent,
        3 => StatusColor::Warning,
        4 => StatusColor::Danger,
        _ => StatusColor::Success,
    };
    rsx! {
        crate::PlaygroundDemoFrame {
            center: false,
            controls: rsx! {
                crate::Button { onclick: move |_| open.set(true), "Show toast" }
                div {
                    span { "Duration" }
                    crate::SegmentGroup { active: duration_index,
                        crate::SegmentButton { index: 0, "1.5s" }
                        crate::SegmentButton { index: 1, "2.5s" }
                        crate::SegmentButton { index: 2, "5s" }
                        crate::SegmentButton { index: 3, "Off" }
                    }
                }
                div {
                    span { "Position" }
                    crate::SegmentGroup { active: position_index,
                        crate::SegmentButton { index: 0, "Top" }
                        crate::SegmentButton { index: 1, "Middle" }
                        crate::SegmentButton { index: 2, "Bottom" }
                    }
                }
                div {
                    span { "Color" }
                    crate::SegmentGroup { active: color_index,
                        crate::SegmentButton { index: 0, "Neutral" }
                        crate::SegmentButton { index: 1, "Accent" }
                        crate::SegmentButton { index: 2, "Success" }
                        crate::SegmentButton { index: 3, "Warn" }
                        crate::SegmentButton { index: 4, "Danger" }
                    }
                }
            },
            div { class: "g3-toast-demo-stage",
                Toast {
                    open,
                    message: "Round saved".to_string(),
                    color,
                    position,
                    duration_ms,
                }
            }
        }
    }
}
crate::g3_playground! {
    name: "Toast",
    description: "Transient mobile feedback banner with positions and status colors.",
    demo: ToastPlaygroundDemo,
    source: "src/components/toast.rs",
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::G3ThemeProvider;

    fn render(app: fn() -> Element) {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
    }

    #[component]
    fn ToastSmokeApp() -> Element {
        let open = use_signal(|| true);
        rsx! {
            G3ThemeProvider { mode: ComponentMode::Ios,
                Toast {
                    open,
                    message: "Saved".to_string(),
                    color: StatusColor::Success,
                    duration_ms: 0,
                }
            }
        }
    }

    #[test]
    fn toast_renders() {
        render(ToastSmokeApp);
    }

    #[test]
    fn toast_uses_alert_role_for_urgent_colors() {
        assert_eq!(color_class(StatusColor::Danger), s::COLOR_DANGER);
        let source = include_str!("toast.rs");
        assert!(source.contains("StatusColor::Danger | StatusColor::Warning"));
        assert!(source.contains("aria_live"));
    }

    #[test]
    fn closed_toasts_are_not_keyboard_focusable() {
        let source = include_str!("toast.rs");
        assert!(source.contains("inert: closed_inert"));
        assert!(source.contains("disabled: !open()"));
    }

    #[test]
    fn toast_autodismiss_uses_generation_guard() {
        let source = include_str!("toast.rs");
        assert!(source.contains("dismiss_generation"));
        assert!(source.contains("dismiss_generation() == generation && open()"));
    }

    #[test]
    fn toast_autodismiss_uses_dioxus_sdk_time() {
        let source = include_str!("toast.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("toast source should have production section");
        assert!(source.contains("dioxus_sdk_time::sleep"));
        assert!(source.contains("duration_ms.unwrap_or(3000)"));
        assert!(!source.contains("document::eval"));
    }

    #[test]
    fn toast_close_button_owns_the_trailing_column_without_an_action() {
        let stylesheet = include_str!("../../assets/g3_ui.css");
        let close = stylesheet
            .split(".g3-toast-close {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("missing toast close style");

        assert!(close.contains("grid-column: 4"));
        assert!(close.contains("justify-self: end"));
    }
}
