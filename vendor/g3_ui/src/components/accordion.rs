//! Accordion components for expandable mobile content groups.

use super::accordion_styles as s;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::prelude::*;
use dioxus_icons::lucide::ChevronDown;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ACCORDION_GROUP_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
struct AccordionGroupContext {
    value: Signal<Vec<String>>,
    // Held as a Signal (not a plain bool) so an already-mounted item — which may
    // be memoized and never re-read the context — still sees the current value
    // when `multiple` is toggled reactively.
    multiple: Signal<bool>,
    on_change: Option<Callback<Vec<String>>>,
    id_prefix: String,
}

fn next_accordion_group_id() -> String {
    format!(
        "g3-accordion-{}",
        NEXT_ACCORDION_GROUP_ID.fetch_add(1, Ordering::Relaxed)
    )
}

fn accordion_group_id(id: Option<String>, generated_id: &str) -> String {
    id.filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| generated_id.to_string())
}

pub fn accordion_panel_id(group_id: &str, value: &str) -> String {
    let slug: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "item" } else { slug };
    format!("{group_id}-panel-{slug}")
}

fn next_accordion_values(current: &[String], item_value: &str, multiple: bool) -> Vec<String> {
    let is_open = current.iter().any(|value| value == item_value);
    if is_open {
        current
            .iter()
            .filter(|value| value.as_str() != item_value)
            .cloned()
            .collect()
    } else if multiple {
        let mut next = current.to_vec();
        next.push(item_value.to_string());
        next
    } else {
        vec![item_value.to_string()]
    }
}

#[component]
pub fn AccordionGroup(
    value: Option<Signal<Vec<String>>>,
    id: Option<String>,
    /// When `true`, more than one item in the group can be expanded at a time.
    /// When `false` (the default), opening an item collapses any other open
    /// item so only one stays expanded.
    multiple: Option<bool>,
    class: Option<String>,
    mode: Option<ComponentMode>,
    on_change: Option<Callback<Vec<String>>>,
    children: Element,
) -> Element {
    let mode = use_component_mode(mode);
    let internal_value = use_signal(Vec::<String>::new);
    let value = value.unwrap_or(internal_value);
    let generated_id = use_hook(next_accordion_group_id);
    let id_prefix = accordion_group_id(id, &generated_id);
    let mode_cls = match mode {
        ComponentMode::Ios => s::GROUP_IOS,
        ComponentMode::Md => s::GROUP_MD,
    };
    let multiple_value = multiple.unwrap_or(false);
    let mut multiple = use_signal(|| multiple_value);
    // Keep the shared signal in sync with the reactive prop.
    if *multiple.peek() != multiple_value {
        multiple.set(multiple_value);
    }
    provide_context(AccordionGroupContext {
        value,
        multiple,
        on_change,
        id_prefix: id_prefix.clone(),
    });

    rsx! {
        div {
            id: id_prefix,
            class: merge_classes(format!("{} {mode_cls}", s::GROUP), class.as_deref()),
            role: "presentation",
            {children}
        }
    }
}

#[component]
pub fn AccordionItem(
    value: String,
    label: String,
    description: Option<String>,
    disabled: Option<bool>,
    class: Option<String>,
    header: Option<Element>,
    children: Element,
) -> Element {
    let mut context = use_context::<AccordionGroupContext>();
    let disabled = disabled.unwrap_or(false);
    let expanded = (context.value)().iter().any(|active| active == &value);
    let panel_id = accordion_panel_id(&context.id_prefix, &value);
    let button_id = format!("{panel_id}-button");
    let state = if expanded { "open" } else { "closed" };

    rsx! {
        div {
            class: merge_classes(
                format!("{} {}", s::ITEM, if disabled { s::ITEM_DISABLED } else { "" }),
                class.as_deref(),
            ),
            "data-state": state,
            button {
                id: button_id.clone(),
                class: s::HEADER,
                r#type: "button",
                disabled,
                aria_expanded: expanded.to_string(),
                aria_controls: panel_id.clone(),
                onclick: move |_| {
                    if disabled {
                        return;
                    }
                    let next = next_accordion_values(
                        &(context.value)(),
                        &value,
                        (context.multiple)(),
                    );
                    if let Some(on_change) = context.on_change {
                        on_change.call(next.clone());
                    }
                    (context.value).set(next);
                },
                span { class: s::HEADER_TEXT,
                    if let Some(header) = header {
                        {header}
                    } else {
                        span { class: s::LABEL, "{label}" }
                        if let Some(description) = description {
                            span { class: s::DESCRIPTION, "{description}" }
                        }
                    }
                }
                span { class: s::CHEVRON, aria_hidden: "true",
                    ChevronDown { size: 18, class: "fill-none" }
                }
            }
            div {
                id: panel_id,
                class: s::PANEL,
                role: "region",
                "data-state": state,
                aria_hidden: (!expanded).to_string(),
                aria_labelledby: button_id,
                inert: (!expanded).then(|| "".to_string()),
                div { class: s::CONTENT, {children} }
            }
        }
    }
}

#[cfg(feature = "playground")]
#[component]
pub fn AccordionPlaygroundDemo() -> Element {
    let value = use_signal(|| vec!["round".to_string()]);
    let allow_multiple = use_signal(|| false);
    rsx! {
        crate::PlaygroundDemoFrame {
            center: false,
            controls: rsx! {
                crate::Checkbox { checked: allow_multiple, label: "Allow multiple open".to_string() }
            },
            AccordionGroup { value, multiple: allow_multiple(),
                AccordionItem {
                    value: "round".to_string(),
                    label: "Round setup".to_string(),
                    description: "Players and tees".to_string(),
                    p { "Choose players, tees, and starting hole before creating the round." }
                }
                AccordionItem {
                    value: "scoring".to_string(),
                    label: "Scoring".to_string(),
                    description: "Bets and formats".to_string(),
                    p { "Match play, skins, and side bets can be configured per group." }
                }
            }
        }
    }
}

crate::g3_playground! {
    name: "Accordion",
    description: "Expandable mobile content sections with grouped state.",
    demo: AccordionPlaygroundDemo,
    source: "src/components/accordion.rs",
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::G3ThemeProvider;

    fn render(app: fn() -> Element) {
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
    }

    #[test]
    fn next_values_support_single_and_multiple_modes() {
        assert_eq!(
            next_accordion_values(&[], "a", false),
            vec!["a".to_string()]
        );
        assert_eq!(
            next_accordion_values(&["a".to_string()], "b", false),
            vec!["b".to_string()]
        );
        assert_eq!(
            next_accordion_values(&["a".to_string()], "b", true),
            vec!["a".to_string(), "b".to_string()]
        );
        assert!(next_accordion_values(&["a".to_string()], "a", true).is_empty());
    }

    #[test]
    fn panel_ids_are_group_scoped_and_dom_safe() {
        assert_eq!(
            accordion_panel_id("g3-accordion-1", "Round Setup"),
            "g3-accordion-1-panel-round-setup"
        );
        assert_eq!(
            accordion_panel_id("g3-accordion-1", "!!!"),
            "g3-accordion-1-panel-item"
        );
    }

    #[test]
    fn accordion_group_ids_can_be_explicit_or_generated() {
        assert_eq!(
            accordion_group_id(Some("custom".to_string()), "generated"),
            "custom"
        );
        assert_eq!(
            accordion_group_id(Some("".to_string()), "generated"),
            "generated"
        );
    }

    #[component]
    fn AccordionSmokeApp() -> Element {
        let value = use_signal(|| vec!["one".to_string()]);
        rsx! {
            G3ThemeProvider { mode: ComponentMode::Md,
                AccordionGroup { value,
                    AccordionItem { value: "one".to_string(), label: "One".to_string(), "Content" }
                }
            }
        }
    }

    #[test]
    fn accordion_renders() {
        render(AccordionSmokeApp);
    }
}
