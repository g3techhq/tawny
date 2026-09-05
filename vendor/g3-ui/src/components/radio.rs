//! Radio group and radio components.
use super::checkbox::ControlLabelPlacement;
use super::radio_styles as s;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT_RADIO_GROUP_NAME: AtomicU64 = AtomicU64::new(1);
#[derive(Clone)]
struct RadioGroupContext {
    value: Signal<String>,
    name: String,
    disabled: ReadSignal<bool>,
    allow_empty_selection: bool,
    on_change: Option<Callback<String>>,
}
#[component]
pub fn RadioGroup(
    value: Signal<String>,
    name: Option<String>,
    disabled: Option<bool>,
    allow_empty_selection: Option<bool>,
    class: Option<String>,
    on_change: Option<Callback<String>>,
    children: Element,
) -> Element {
    let generated_name = use_hook(next_radio_group_name);
    let group_name = radio_group_name(name, &generated_name);
    let is_disabled = disabled.unwrap_or(false);
    let aria_disabled = is_disabled.then(|| "true".to_string());
    let mut disabled_signal = use_signal(|| is_disabled);
    if *disabled_signal.peek() != is_disabled {
        disabled_signal.set(is_disabled);
    }
    provide_context(RadioGroupContext {
        value,
        name: group_name,
        disabled: disabled_signal.into(),
        allow_empty_selection: allow_empty_selection.unwrap_or(false),
        on_change,
    });
    rsx! {
        div {
            class: merge_classes(s::GROUP, class.as_deref()),
            role: "radiogroup",
            aria_disabled,
            {children}
        }
    }
}
#[component]
pub fn Radio(
    value: String,
    label: Option<String>,
    disabled: Option<bool>,
    placement: Option<ControlLabelPlacement>,
    class: Option<String>,
    mode: Option<ComponentMode>,
) -> Element {
    debug_assert!(
        !value.is_empty(),
        "Radio values must not be empty because RadioGroup uses an empty string as its no-selection sentinel",
    );
    let mode = use_component_mode(mode);
    let context = use_context::<RadioGroupContext>();
    let selected = (context.value)() == value;
    let is_disabled = disabled.unwrap_or(false) || (context.disabled)();
    let placement = placement.unwrap_or_default();
    let mode_cls = match mode {
        ComponentMode::Ios => s::RADIO_IOS,
        ComponentMode::Md => s::RADIO_MD,
    };
    let checked_cls = if selected { "checked" } else { "" };
    let disabled_cls = if is_disabled { "disabled" } else { "" };
    let cls = merge_classes(
        format!(
            "{} {mode_cls} {} {checked_cls} {disabled_cls}",
            s::RADIO,
            placement.class(),
        ),
        class.as_deref(),
    );
    let aria_checked = selected.to_string();
    let pointer_value = value.clone();
    let change_value = value.clone();
    let mut pointer_context = context.clone();
    let mut change_context = context.clone();
    let mut suppress_click = use_signal(|| false);
    rsx! {
        label {
            class: cls,
            onpointerdown: move |event| {
                if is_disabled {
                    event.prevent_default();
                    suppress_click.set(true);
                    return;
                }
                let current = (pointer_context.value).peek().clone();
                let Some(next) = next_radio_value(
                    &current,
                    &pointer_value,
                    pointer_context.allow_empty_selection,
                ) else {
                    return;
                };
                if !next.is_empty() {
                    return;
                }
                event.prevent_default();
                suppress_click.set(true);
                (pointer_context.value).set(next.clone());
                if let Some(ref on_change) = pointer_context.on_change {
                    on_change.call(next);
                }
            },
            onclick: move |event| {
                if suppress_click() {
                    event.prevent_default();
                    suppress_click.set(false);
                }
            },
            input {
                class: s::INPUT,
                r#type: "radio",
                role: "radio",
                name: context.name.clone(),
                value: value.clone(),
                checked: selected,
                aria_checked,
                disabled: is_disabled,
                onchange: move |_| {
                    if is_disabled {
                        return;
                    }
                    let current = (change_context.value).peek().clone();
                    let Some(next) = next_radio_value(&current, &change_value, false) else {
                        return;
                    };
                    (change_context.value).set(next.clone());
                    if let Some(ref on_change) = change_context.on_change {
                        on_change.call(next);
                    }
                },
            }
            span { class: s::CONTROL, aria_hidden: "true",
                span { class: s::MARK, aria_hidden: "true" }
            }
            if let Some(label) = label.filter(|value| !value.is_empty()) {
                span { class: s::LABEL, "{label}" }
            }
        }
    }
}
fn next_radio_group_name() -> String {
    let id = NEXT_RADIO_GROUP_NAME.fetch_add(1, Ordering::Relaxed);
    format!("g3-radio-group-{id}")
}
fn radio_group_name(name: Option<String>, fallback_name: &str) -> String {
    name.filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback_name.to_string())
}
/// `String::new()` is the controlled no-selection sentinel for RadioGroup.
fn next_radio_value(current: &str, clicked: &str, allow_empty_selection: bool) -> Option<String> {
    if current == clicked {
        allow_empty_selection.then(String::new)
    } else {
        Some(clicked.to_string())
    }
}
#[cfg(feature = "playground")]
#[component]
pub fn RadioPlaygroundDemo() -> Element {
    let selected = use_signal(|| "push".to_string());
    let disabled = use_signal(|| false);
    let allow_empty = use_signal(|| false);
    rsx! {
        crate::PlaygroundDemoFrame {
            center: false,
            controls: rsx! {
                crate::Checkbox { checked: disabled, label: "Disabled".to_string() }
                crate::Checkbox { checked: allow_empty, label: "Allow empty".to_string() }
            },
            div { class: "g3-radio-demo-stack",
                RadioGroup {
                    value: selected,
                    disabled: disabled(),
                    allow_empty_selection: allow_empty(),
                    Radio { value: "push", label: "Push notifications" }
                    Radio { value: "email", label: "Email summaries" }
                    Radio {
                        value: "none",
                        label: "No reminders",
                        placement: ControlLabelPlacement::End,
                    }
                }
            }
        }
    }
}
crate::g3_playground! {
    name: "Radio",
    description: "Single-select radio group.",
    demo: RadioPlaygroundDemo,
    source: "src/components/radio.rs",
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
    fn RadioSmokeApp() -> Element {
        let selected = use_signal(|| "walking".to_string());
        rsx! {
            G3ThemeProvider { mode: ComponentMode::Ios,
                RadioGroup { value: selected, on_change: |_| {},
                    Radio { value: "walking", label: "Walking" }
                    Radio {
                        value: "riding",
                        label: "Riding",
                        placement: ControlLabelPlacement::End,
                    }
                }
            }
        }
    }
    #[test]
    fn radio_group_renders() {
        render(RadioSmokeApp);
    }
    #[test]
    fn radio_selection_selects_clicked_value() {
        assert_eq!(
            next_radio_value("walking", "riding", false),
            Some("riding".to_string()),
        );
    }
    #[test]
    fn radio_selection_clears_selected_value_only_when_allowed() {
        assert_eq!(
            next_radio_value("walking", "walking", true),
            Some(String::new())
        );
        assert_eq!(next_radio_value("walking", "walking", false), None);
    }
    #[test]
    fn radio_group_name_prefers_non_empty_explicit_name() {
        assert_eq!(
            radio_group_name(Some("pace".to_string()), "g3-radio-group-99"),
            "pace",
        );
        assert_eq!(
            radio_group_name(Some(String::new()), "g3-radio-group-99"),
            "g3-radio-group-99",
        );
    }
}
