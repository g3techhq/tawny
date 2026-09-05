//! Segment control with iOS and Android styling.
use super::header::HeaderToolbarContext;
use super::segment_styles as s;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::{core::DynamicNode, prelude::*};
#[derive(Clone)]
struct SegmentGroupContext {
    active: Signal<usize>,
    on_change: Option<Callback<usize>>,
    defer_active: bool,
}
#[component]
pub fn SegmentGroup(
    active: Signal<usize>,
    on_change: Option<Callback<usize>>,
    defer_active: Option<bool>,
    class: Option<String>,
    mode: Option<ComponentMode>,
    children: Element,
) -> Element {
    let mode = use_component_mode(mode);
    let segment_count = count_segment_children(&children);
    let in_toolbar = try_consume_context::<HeaderToolbarContext>().is_some();
    provide_context(SegmentGroupContext {
        active,
        on_change,
        defer_active: defer_active.unwrap_or(false),
    });
    let segment_cls = match mode {
        ComponentMode::Ios => s::SEGMENT_IOS,
        ComponentMode::Md => s::SEGMENT_MD,
    };
    rsx! {
        div {
            class: merge_classes(
                format!("{segment_cls} {}", if in_toolbar { s::TOOLBAR } else { s::STANDALONE }),
                class.as_deref(),
            ),
            role: "tablist",
            "data-active": active().to_string(),
            style: format!("--g3-segment-active: {}; --g3-segment-count: {};", active(), segment_count),
            {children}
        }
    }
}
fn count_segment_children(children: &Element) -> usize {
    children
        .as_ref()
        .map(|node| count_dynamic_components(&node.dynamic_nodes).max(1))
        .unwrap_or(1)
}
fn count_dynamic_components(nodes: &[DynamicNode]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            DynamicNode::Component(_) => 1,
            DynamicNode::Fragment(children) => children
                .iter()
                .map(|child| count_dynamic_components(&child.dynamic_nodes))
                .sum(),
            DynamicNode::Text(_) | DynamicNode::Placeholder(_) => 0,
        })
        .sum()
}
#[component]
pub fn SegmentButton(
    index: usize,
    disabled: Option<bool>,
    class: Option<String>,
    mode: Option<ComponentMode>,
    children: Element,
) -> Element {
    let mode = use_component_mode(mode);
    let mut context = use_context::<SegmentGroupContext>();
    let disabled = disabled.unwrap_or(false);
    let selected = (context.active)() == index;
    let btn_cls = match mode {
        ComponentMode::Ios => s::SEGMENT_BTN_IOS,
        ComponentMode::Md => s::SEGMENT_BTN_MD,
    };
    rsx! {
        button {
            class: merge_classes(btn_cls, class.as_deref()),
            r#type: "button",
            role: "tab",
            disabled,
            aria_selected: selected.to_string(),
            "data-state": if selected { "on" } else { "off" },
            "data-disabled": disabled.to_string(),
            onclick: move |_| {
                if disabled || *(context.active).peek() == index {
                    return;
                }
                if let Some(ref on_change) = context.on_change {
                    on_change.call(index);
                }
                if !context.defer_active {
                    (context.active).set(index);
                }
            },
            {children}
        }
    }
}
#[cfg(feature = "playground")]
#[component]
pub fn SegmentPlaygroundDemo() -> Element {
    let toolbar_active = use_signal(|| 0_usize);
    let standalone_active = use_signal(|| 0_usize);
    let playground_mode = crate::use_component_mode(None);
    rsx! {
        crate::PlaygroundDemoFrame { app: false, center: false,
            crate::AppWrapper { mode: playground_mode, class: "g3-playground-device-app",
                crate::Header {
                    title: "Segments",
                    toolbar: rsx! {
                        SegmentGroup { active: toolbar_active,
                            SegmentButton { index: 0, "Players" }
                            SegmentButton { index: 1, "Bet" }
                            SegmentButton { index: 2, "Ready" }
                        }
                    },
                }
                crate::Body { has_footer_space: false, padding: true,
                    crate::Card { title: "Standalone", class: "g3-segment-demo-card",
                        SegmentGroup { active: standalone_active,
                            SegmentButton { index: 0, "Gross" }
                            SegmentButton { index: 1, "Net" }
                            SegmentButton { index: 2, "Skins" }
                        }
                    }
                }
            }
        }
    }
}
crate::g3_playground! {
    name: "SegmentGroup",
    description: "Single-select segmented control.",
    demo: SegmentPlaygroundDemo,
    source: "src/components/segment.rs",
}
