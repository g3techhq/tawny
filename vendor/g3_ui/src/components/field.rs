//! Field (text input) component with iOS/Android styling.

use super::field_styles as s;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::prelude::*;

#[component]
pub fn Field(
    #[props(extends=input)] attributes: Vec<Attribute>,
    label: String,
    mut value: Signal<String>,
    oninput: Option<EventHandler<Event<FormData>>>,
    onchange: Option<EventHandler<Event<FormData>>>,
    debounce: Option<u32>,
    disabled: Option<bool>,
    maxlength: Option<usize>,
    minlength: Option<usize>,
    min: Option<isize>,
    max: Option<isize>,
    r#type: Option<String>,
    placeholder: Option<String>,
    class: Option<String>,
    mode: Option<ComponentMode>,
    /// Renders a `<textarea>` instead of a single-line `<input>` — same
    /// label/value/debounce/validation wiring, for reviews/comments/notes
    /// that need more than one line. `#[props(extends=input)]`
    /// `attributes` aren't forwarded in this mode since they're typed for
    /// `input`, not `textarea`.
    multiline: Option<bool>,
    /// Visible row count when `multiline` is set. Defaults to 3.
    rows: Option<u32>,
    /// Trailing slot rendered inside the field's own box (e.g. a filter
    /// icon button) — vertically centered, overlaid on the input's right
    /// edge. The input gains right padding automatically so typed text
    /// never runs under it.
    end: Option<Element>,
) -> Element {
    let mut is_focused = use_signal(|| false);
    let mut modified = use_signal(|| false);
    let mut reapply_value = use_signal(|| false);
    let mut debounce_generation = use_signal(|| 0_u64);
    let mode = use_component_mode(mode);

    let mode_cls = match mode {
        ComponentMode::Ios => s::FIELD_IOS,
        ComponentMode::Md => s::FIELD_MD,
    };
    let field_cls = merge_classes(format!("{} {mode_cls}", s::FIELD), class.as_deref());

    let id = label.clone();
    let r#for = label.clone();

    let combo_value = use_memo(move || {
        if reapply_value() {
            format!("{}0", value())
        } else {
            format!("{}1", value())
        }
    });

    let mut check_validity = move |new_value: String| -> bool {
        if let Some(max) = max
            && let Ok(v) = new_value.parse::<isize>()
            && v > max
        {
            reapply_value.toggle();
            return false;
        }
        if let Some(min) = min
            && let Ok(v) = new_value.parse::<isize>()
            && v < min
        {
            reapply_value.toggle();
            return false;
        }
        if let Some(minlength) = minlength
            && new_value.len() < minlength
        {
            reapply_value.toggle();
            return false;
        }
        if let Some(maxlength) = maxlength
            && new_value.len() > maxlength
        {
            reapply_value.toggle();
            return false;
        }
        true
    };

    let input_style = if end.is_some() {
        "width: stretch; padding-right: 2.75rem;"
    } else {
        "width: stretch;"
    };

    rsx! {
        div { class: "relative w-full",
            if !label.is_empty() {
                label { r#for,
                    div { class: "g3-field-label", "{label}" }
                }
            }
            div { class: "relative",
            if multiline.unwrap_or(false) {
                textarea {
                    id,
                    class: field_cls,
                    style: input_style,
                    rows: rows.unwrap_or(3),
                    onfocus: move |_| {
                        is_focused.set(true);
                    },
                    onblur: move |_| {
                        is_focused.set(false);
                    },
                    oninput: move |event: Event<FormData>| {
                        let new_value = event.value();
                        if !check_validity(new_value.clone()) {
                            return;
                        }
                        value.set(new_value);
                        modified.set(true);
                        if let Some(oninput) = oninput {
                            oninput.call(event.clone());
                        }
                        if let Some(d) = debounce {
                            let generation = debounce_generation
                                .with_mut(|generation| {
                                    *generation += 1;
                                    *generation
                                });
                            let ev = event;
                            spawn(async move {
                                dioxus_sdk_time::sleep(std::time::Duration::from_millis(d as u64)).await;
                                if debounce_generation() != generation {
                                    return;
                                }
                                if let Some(onchange) = onchange {
                                    onchange.call(ev);
                                }
                            });
                        }
                    },
                    onchange: move |event| {
                        if debounce.is_some() {
                            return;
                        }
                        if let Some(onchange) = onchange {
                            onchange.call(event);
                        }
                    },
                    value: &combo_value()[..combo_value().len() - 1],
                    minlength,
                    maxlength,
                    placeholder,
                    disabled,
                }
            } else {
            input {
                id,
                class: field_cls,
                style: input_style,
                onfocus: move |_| {
                    is_focused.set(true);
                },
                onblur: move |_| {
                    is_focused.set(false);
                },
                oninput: move |event: Event<FormData>| {
                    let new_value = event.value();
                    if !check_validity(new_value.clone()) {
                        return;
                    }
                    value.set(new_value);
                    modified.set(true);
                    if let Some(oninput) = oninput {
                        oninput.call(event.clone());
                    }
                    if let Some(d) = debounce {
                        let generation = debounce_generation
                            .with_mut(|generation| {
                                *generation += 1;
                                *generation
                            });
                        let ev = event;
                        spawn(async move {
                            dioxus_sdk_time::sleep(std::time::Duration::from_millis(d as u64)).await;
                            if debounce_generation() != generation {
                                return;
                            }
                            if let Some(onchange) = onchange {
                                onchange.call(ev);
                            }
                        });
                    }
                },
                onchange: move |event| {
                    if debounce.is_some() {
                        return;
                    }
                    if let Some(onchange) = onchange {
                        onchange.call(event);
                    }
                },
                value: &combo_value()[..combo_value().len() - 1],
                min,
                max,
                minlength,
                maxlength,
                placeholder,
                disabled,
                r#type,
                ..attributes,
            }
            }
            if let Some(end) = end {
                div {
                    class: "absolute",
                    style: "right: 0.5rem; top: 50%; transform: translateY(-50%); display: flex; align-items: center;",
                    {end}
                }
            }
            }
        }
    }
}
#[cfg(feature = "playground")]
#[component]
pub fn FieldPlaygroundDemo() -> Element {
    let value = use_signal(String::new);
    let label = use_signal(|| "Club".to_string());
    let placeholder = use_signal(|| "Club name".to_string());
    let disabled = use_signal(|| false);
    let numeric = use_signal(|| false);
    let multiline = use_signal(|| false);

    rsx! {
        crate::PlaygroundDemoFrame {
            controls: rsx! {
                crate::Field { label: "Label".to_string(), value: label }
                crate::Field { label: "Placeholder".to_string(), value: placeholder }
                crate::Checkbox { checked: disabled, label: "Disabled".to_string() }
                crate::Checkbox { checked: numeric, label: "Number type".to_string() }
                crate::Checkbox { checked: multiline, label: "Multiline".to_string() }
            },
            Field {
                label: label(),
                value,
                placeholder: placeholder(),
                disabled: disabled(),
                r#type: if numeric() { "number" } else { "text" },
                multiline: multiline(),
            }
        }
    }
}
crate::g3_playground! {
    name: "Field",
    description: "Text input with optional validation limits and debounce.",
    demo: FieldPlaygroundDemo,
    source: "src/components/field.rs",
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
    fn FieldSmokeApp() -> Element {
        let single_line = use_signal(String::new);
        let multiline_value = use_signal(String::new);

        rsx! {
            G3ThemeProvider { mode: ComponentMode::Ios,
                Field { label: "Title", value: single_line }
                Field { label: "Review", value: multiline_value, multiline: true, rows: 4 }
            }
        }
    }

    #[test]
    fn field_family_renders() {
        render(FieldSmokeApp);
    }

    #[test]
    fn multiline_renders_a_textarea_reusing_field_styling() {
        let source = include_str!("field.rs");
        assert!(source.contains("if multiline.unwrap_or(false) {"));
        assert!(source.contains("textarea {"));
        assert!(source.contains("class: field_cls,"));
        let stylesheet = include_str!("../../assets/g3_ui.css");
        assert!(stylesheet.contains("textarea.g3-field"));
    }
}
