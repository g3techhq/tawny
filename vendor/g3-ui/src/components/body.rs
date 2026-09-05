//! Body component - scrollable page content with error/loading boundaries.
use super::body_styles as s;
use crate::components::Spinner;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::prelude::*;
#[cfg(feature = "transitions")]
use g3_route_transitions::ROUTE_TRANSITION_SEGMENT_CLASS;
#[component]
pub fn Body(
    children: Element,
    has_footer_space: Option<bool>,
    padding: Option<bool>,
    fab: Option<Element>,
    class: Option<String>,
    mode: Option<ComponentMode>,
) -> Element {
    let mode = use_component_mode(mode);
    let body_cls = match mode {
        ComponentMode::Ios => format!("{} {}", s::BODY_BASE, s::BODY_IOS),
        ComponentMode::Md => format!("{} {}", s::BODY_BASE, s::BODY_MD),
    };
    let padding_enabled = padding.unwrap_or(true);
    let body_content_cls = if padding_enabled {
        s::BODY_CONTENT.to_string()
    } else {
        format!("{} {}", s::BODY_CONTENT, s::BODY_CONTENT_NO_PADDING)
    };
    let body_padding_style = if padding_enabled {
        "--g3-body-padding: 1.5rem;"
    } else {
        "--g3-body-padding: 0;"
    };
    let body_padding_state = padding_enabled.to_string();
    #[cfg(feature = "transitions")]
    let content_cls = merge_classes(body_content_cls, Some(ROUTE_TRANSITION_SEGMENT_CLASS));
    #[cfg(not(feature = "transitions"))]
    let content_cls = body_content_cls;
    rsx! {
        div { class: merge_classes(body_cls, class.as_deref()),
            div {
                class: content_cls,
                style: body_padding_style,
                "data-padding": body_padding_state,
                ErrorBoundary {
                    handle_error: |_| rsx! {
                        div {
                            class: "flex items-center justify-center py-8",
                            style: "color: var(--color-danger);",
                            role: "alert",
                            "Failed to load resource. Please try again."
                        }
                    },
                    SuspenseBoundary {
                        fallback: |_| rsx! {
                            Spinner { center: true }
                        },
                        {children}
                        if has_footer_space.unwrap_or(true) {
                            div { class: s::FOOTER_SPACER }
                        }
                    }
                }
            }
            if let Some(fab) = fab {
                {fab}
            }
        }
    }
}
#[cfg(feature = "playground")]
#[component]
pub fn BodyPlaygroundDemo() -> Element {
    let footer_space = use_signal(|| false);
    let padding = use_signal(|| true);
    let show_fab = use_signal(|| true);
    let playground_mode = crate::use_component_mode(None);
    rsx! {
        crate::PlaygroundDemoFrame {
            app: false,
            controls: rsx! {
                crate::Checkbox { checked: footer_space, label: "Footer space".to_string() }
                crate::Checkbox { checked: padding, label: "Padding".to_string() }
                crate::Checkbox { checked: show_fab, label: "FAB".to_string() }
            },
            crate::AppWrapper { mode: playground_mode, class: "g3-playground-device-app",
                crate::Header { title: "Body" }
                Body {
                    has_footer_space: footer_space(),
                    padding: padding(),
                    fab: show_fab().then_some(rsx! {
                        crate::Fab {
                            vertical: crate::FabVertical::Bottom,
                            horizontal: crate::FabHorizontal::End,
                            crate::FabButton { onclick: |_| {}, "+" }
                        }
                    }),
                    for index in 1..=12 {
                        crate::Card { title: format!("Hole {index}"),
                            "Scrollable content row with enough body content to show overflow."
                        }
                    }
                }
            }
        }
    }
}
crate::g3_playground! {
    name: "Body",
    description: "Scrollable page body with loading and error boundaries.",
    demo: BodyPlaygroundDemo,
    source: "src/components/body.rs",
}
