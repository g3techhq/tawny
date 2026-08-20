//! Checkbox component with Ionic-style label placement.

use super::checkbox_styles as s;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CHECKBOX_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlLabelPlacement {
    #[default]
    Start,
    End,
    Fixed,
    Stacked,
}

impl ControlLabelPlacement {
    pub(crate) fn class(self) -> &'static str {
        match self {
            Self::Start => s::PLACEMENT_START,
            Self::End => s::PLACEMENT_END,
            Self::Fixed => s::PLACEMENT_FIXED,
            Self::Stacked => s::PLACEMENT_STACKED,
        }
    }
}

#[component]
pub fn Checkbox(
    mut checked: Signal<bool>,
    id: Option<String>,
    label: String,
    indeterminate: Option<bool>,
    disabled: Option<bool>,
    error: Option<String>,
    hint: Option<String>,
    label_placement: Option<ControlLabelPlacement>,
    mode: Option<ComponentMode>,
    class: Option<String>,
    onchange: Option<Callback<bool>>,
) -> Element {
    let mode = use_component_mode(mode);
    let is_checked = checked();
    let is_indeterminate = indeterminate.unwrap_or(false);
    let is_disabled = disabled.unwrap_or(false);
    let has_error = error.as_ref().is_some_and(|value| !value.is_empty());
    let has_hint = hint.as_ref().is_some_and(|value| !value.is_empty());
    let placement = label_placement.unwrap_or_default();
    let mode_cls = match mode {
        ComponentMode::Ios => s::CHECKBOX_IOS,
        ComponentMode::Md => s::CHECKBOX_MD,
    };
    let checked_cls = if is_checked { "checked" } else { "" };
    let indeterminate_cls = if is_indeterminate {
        "indeterminate"
    } else {
        ""
    };
    let disabled_cls = if is_disabled { "disabled" } else { "" };
    let invalid_cls = if has_error { "invalid" } else { "" };
    let single_line_cls = if !has_error
        && !has_hint
        && matches!(
            placement,
            ControlLabelPlacement::Start | ControlLabelPlacement::End
        ) {
        "g3-checkbox-single-line"
    } else {
        ""
    };
    let cls = merge_classes(
        format!(
            "{} {mode_cls} {} {checked_cls} {indeterminate_cls} {disabled_cls} {invalid_cls} {single_line_cls}",
            s::CHECKBOX,
            placement.class(),
        ),
        class.as_deref(),
    );
    let generated_id = use_hook(next_checkbox_id);
    let control_id = checkbox_base_id(id, &generated_id);
    let hint_id = format!("{control_id}-hint");
    let error_id = format!("{control_id}-error");
    let aria_checked = if is_indeterminate {
        "mixed".to_string()
    } else {
        is_checked.to_string()
    };
    let aria_describedby = describedby(&hint, &error, &hint_id, &error_id);

    rsx! {
        button {
            class: cls,
            r#type: "button",
            role: "checkbox",
            aria_checked,
            aria_invalid: has_error.to_string(),
            id: control_id.clone(),
            aria_describedby,
            disabled: is_disabled,
            onclick: move |_| {
                if is_disabled {
                    return;
                }
                let next = !checked();
                checked.set(next);
                if let Some(ref onchange) = onchange {
                    onchange.call(next);
                }
            },
            span { class: s::CONTROL, aria_hidden: "true",
                span { class: s::MARK }
            }
            span { class: s::LABEL, "{label}" }
            if let Some(hint) = hint.filter(|value| !value.is_empty()) {
                span { id: hint_id, class: s::HINT, "{hint}" }
            }
            if let Some(error) = error.filter(|value| !value.is_empty()) {
                span { id: error_id, class: s::ERROR, "{error}" }
            }
        }
    }
}

fn next_checkbox_id() -> String {
    let id = NEXT_CHECKBOX_ID.fetch_add(1, Ordering::Relaxed);
    format!("g3-checkbox-{id}")
}

fn checkbox_base_id(id: Option<String>, fallback_id: &str) -> String {
    id.filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback_id.to_string())
}

fn describedby(
    hint: &Option<String>,
    error: &Option<String>,
    hint_id: &str,
    error_id: &str,
) -> Option<String> {
    let mut ids = Vec::new();
    if hint.as_ref().is_some_and(|value| !value.is_empty()) {
        ids.push(hint_id);
    }
    if error.as_ref().is_some_and(|value| !value.is_empty()) {
        ids.push(error_id);
    }
    (!ids.is_empty()).then(|| ids.join(" "))
}

#[cfg(feature = "playground")]
#[component]
pub fn CheckboxPlaygroundDemo() -> Element {
    let checked = use_signal(|| true);
    let disabled = use_signal(|| false);
    let mut indeterminate = use_signal(|| false);

    rsx! {
        crate::PlaygroundDemoFrame {
            center: false,
            controls: rsx! {
                crate::Checkbox { checked: disabled, label: "Disabled".to_string() }
                crate::Checkbox { checked: indeterminate, label: "Indeterminate".to_string() }
            },
            div { class: "g3-checkbox-demo-stack",
                Checkbox {
                    checked,
                    label: "Push notifications",
                    hint: "Course updates and tee-time reminders",
                    indeterminate: indeterminate(),
                    disabled: disabled(),
                    onchange: move |_| indeterminate.set(false),
                }
                Checkbox {
                    checked: use_signal(|| false),
                    label: "Share scorecard",
                    label_placement: ControlLabelPlacement::End,
                    error: "Requires a signed-in player",
                }
                Checkbox {
                    checked: use_signal(|| true),
                    label: "Skins game",
                    label_placement: ControlLabelPlacement::Stacked,
                    hint: "Shown as a stacked mobile setting row",
                }
            }
        }
    }
}

crate::g3_playground! {
    name: "Checkbox",
    description: "Controlled checkbox with Ionic-style label placement.",
    demo: CheckboxPlaygroundDemo,
    source: "src/components/checkbox.rs",
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
    fn CheckboxSmokeApp() -> Element {
        let checked = use_signal(|| false);

        rsx! {
            G3ThemeProvider { mode: ComponentMode::Ios,
                Checkbox {
                    checked,
                    label: "Accept terms",
                    hint: "Required before play",
                    onchange: |_| {},
                }
            }
        }
    }

    #[test]
    fn checkbox_renders() {
        render(CheckboxSmokeApp);
    }

    #[test]
    fn checkbox_base_id_prefers_explicit_id() {
        assert_eq!(
            checkbox_base_id(Some("terms-opt-in".to_string()), "g3-checkbox-99"),
            "terms-opt-in"
        );
    }

    #[test]
    fn checkbox_generated_fallback_ids_are_distinct_and_label_independent() {
        let first = next_checkbox_id();
        let second = next_checkbox_id();

        assert_ne!(first, second);
        assert_eq!(checkbox_base_id(None, &first), first);
        assert_eq!(checkbox_base_id(None, &second), second);
    }
}
