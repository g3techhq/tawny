//! Fab component - Floating Action Button with full Ionic parity.
//! Supports: Fab container, FabButton, FabList.
use super::fab_styles as s;
use crate::theme::{ComponentMode, merge_classes, use_component_mode};
use dioxus::prelude::*;
use dioxus_icons::lucide::X;
/// Fab container vertical alignment
#[derive(Clone, Copy, PartialEq, Default)]
pub enum FabVertical {
    /// Pin to the top of the container.
    #[default]
    Top,
    /// Center vertically.
    Center,
    /// Pin to the bottom - the usual placement for a primary action.
    Bottom,
}
/// Fab container horizontal alignment
#[derive(Clone, Copy, PartialEq, Default)]
pub enum FabHorizontal {
    /// Pin to the leading edge.
    #[default]
    Start,
    /// Center horizontally.
    Center,
    /// Pin to the trailing edge - the usual placement for a primary action.
    End,
}
/// FabList side relative to the main FabButton
#[derive(Clone, Copy, PartialEq, Default)]
pub enum FabListSide {
    /// Expand upward from the button.
    #[default]
    Top,
    /// Expand downward from the button.
    Bottom,
    /// Expand toward the leading edge.
    Start,
    /// Expand toward the trailing edge.
    End,
}
/// Fab button size
#[derive(Clone, Copy, PartialEq, Default)]
pub enum FabSize {
    /// Full-size action button.
    #[default]
    Normal,
    /// Reduced size, for the secondary buttons revealed by a `FabList`.
    Small,
}
/// Fab container - wraps buttons and lists, handles fixed positioning.
#[component]
pub fn Fab(
    children: Element,
    #[props(extends = Input)] attributes: Vec<Attribute>,
    vertical: Option<FabVertical>,
    horizontal: Option<FabHorizontal>,
    edge: Option<bool>,
    class: Option<String>,
) -> Element {
    let vert = vertical.unwrap_or_default();
    let horiz = horizontal.unwrap_or_default();
    let is_edge = edge.unwrap_or(false);
    let vert_cls = match vert {
        FabVertical::Top => "top-0",
        FabVertical::Center => "top-[50%] -translate-y-[50%]",
        FabVertical::Bottom => "bottom-6",
    };
    let horiz_cls = match horiz {
        FabHorizontal::Start => "left-6",
        FabHorizontal::Center => "left-[50%] -translate-x-[50%]",
        FabHorizontal::End => "right-6",
    };
    let edge_cls = if is_edge {
        "mt-[-3.5rem] mb-[-3.5rem] "
    } else {
        ""
    };
    let container_cls = merge_classes(
        format!("{} {vert_cls} {horiz_cls} {edge_cls}", s::FAB_CONTAINER),
        class.as_deref(),
    );
    let final_attributes: Vec<Attribute> = attributes
        .into_iter()
        .filter(|attr| attr.name as &str != "onclick")
        .collect();
    rsx! {
        div { class: container_cls, ..final_attributes, {children} }
    }
}
/// FabButton - the primary circular action button.
#[component]
pub fn FabButton(
    children: Element,
    #[props(extends = Input)] attributes: Vec<Attribute>,
    activated: Option<bool>,
    close_icon: Option<Element>,
    size: Option<FabSize>,
    translucent: Option<bool>,
    onclick: Option<Callback<Event<MouseData>>>,
    href: Option<String>,
    target: Option<String>,
    disabled: Option<bool>,
    class: Option<String>,
    mode: Option<ComponentMode>,
) -> Element {
    let mode = use_component_mode(mode);
    let is_activated = activated.unwrap_or(false);
    let is_small = size.unwrap_or_default() == FabSize::Small;
    let is_translucent = translucent.unwrap_or(false);
    let is_disabled = disabled.unwrap_or(false);
    let fab_cls = match mode {
        ComponentMode::Ios => s::FAB_IOS,
        ComponentMode::Md => s::FAB_MD,
    };
    let size_cls = if is_small { s::FAB_SMALL } else { "" };
    let translucent_cls = if is_translucent && mode == ComponentMode::Ios {
        s::FAB_TRANSLUCENT
    } else {
        ""
    };
    let btn_cls = merge_classes(
        format!("{} {fab_cls} {size_cls} {translucent_cls}", s::FAB),
        class.as_deref(),
    );
    let final_attributes: Vec<Attribute> = attributes
        .into_iter()
        .filter(|attr| attr.name as &str != "onclick")
        .collect();
    if let Some(ref href) = href {
        rsx! {
            a {
                href,
                target: target.clone(),
                class: btn_cls,
                aria_disabled: is_disabled.to_string(),
                ..final_attributes,
                if is_activated && close_icon.is_some() {
                    {close_icon}
                } else {
                    {children}
                }
            }
        }
    } else {
        rsx! {
            button {
                class: btn_cls,
                r#type: "button",
                disabled: is_disabled,
                onclick: move |event| {
                    if is_disabled {
                        return;
                    }
                    if let Some(ref handler) = onclick {
                        handler.call(event);
                    }
                },
                ..final_attributes,
                if is_activated && close_icon.is_some() {
                    {close_icon}
                } else {
                    {children}
                }
            }
        }
    }
}
/// FabList - expandable list of secondary fab buttons.
#[component]
pub fn FabList(
    children: Element,
    #[props(extends = Input)] attributes: Vec<Attribute>,
    activated: Option<bool>,
    side: Option<FabListSide>,
    class: Option<String>,
    mode: Option<ComponentMode>,
) -> Element {
    let mode = use_component_mode(mode);
    let is_activated = activated.unwrap_or(false);
    let side = side.unwrap_or_default();
    let list_cls = match mode {
        ComponentMode::Ios => s::FAB_LIST_IOS,
        ComponentMode::Md => s::FAB_LIST_MD,
    };
    let side_cls = match side {
        FabListSide::Top => s::FAB_LIST_TOP,
        FabListSide::Bottom => s::FAB_LIST_BOTTOM,
        FabListSide::Start => s::FAB_LIST_START,
        FabListSide::End => s::FAB_LIST_END,
    };
    let final_attributes: Vec<Attribute> = attributes.into_iter().collect();
    rsx! {
        div {
            class: merge_classes(
                format!("{} {} {}", s::FAB_LIST_BASE, list_cls, side_cls),
                class.as_deref(),
            ),
            aria_hidden: (!is_activated).to_string(),
            ..final_attributes,
            if !is_activated {
                div { style: "display: none;", {children} }
            } else {
                {children}
            }
        }
    }
}
/// Convenience component: Fab with integrated list.
/// Wraps Fab + FabButton + FabList into a single component with activation toggle.
#[component]
pub fn FabContainer(
    main_button: Element,
    list_buttons: Option<Element>,
    vertical: Option<FabVertical>,
    horizontal: Option<FabHorizontal>,
    edge: Option<bool>,
    list_side: Option<FabListSide>,
    class: Option<String>,
) -> Element {
    let mut is_activated = use_signal(|| false);
    let toggle = Callback::new(move |_| {
        is_activated.with_mut(|active| *active = !*active);
    });
    rsx! {
        Fab {
            vertical,
            horizontal,
            edge,
            class,
            FabButton {
                activated: is_activated(),
                onclick: toggle,
                close_icon: rsx! {
                    X { size: 24 }
                },
                {main_button}
            }
            if let Some(list) = list_buttons {
                FabList { activated: is_activated(), side: list_side, {list} }
            }
        }
    }
}
#[cfg(feature = "playground")]
#[component]
pub fn FabPlaygroundDemo() -> Element {
    let activated = use_signal(|| true);
    let small = use_signal(|| false);
    let edge = use_signal(|| false);
    let vertical_index = use_signal(|| 2_usize);
    let horizontal_index = use_signal(|| 2_usize);
    let list_side_index = use_signal(|| 0_usize);
    let playground_mode = crate::use_component_mode(None);
    let vertical = match vertical_index() {
        0 => FabVertical::Top,
        1 => FabVertical::Center,
        _ => FabVertical::Bottom,
    };
    let horizontal = match horizontal_index() {
        0 => FabHorizontal::Start,
        1 => FabHorizontal::Center,
        _ => FabHorizontal::End,
    };
    let list_side = match list_side_index() {
        1 => FabListSide::Bottom,
        2 => FabListSide::Start,
        3 => FabListSide::End,
        _ => FabListSide::Top,
    };
    rsx! {
        crate::PlaygroundDemoFrame {
            app: false,
            center: false,
            controls: rsx! {
                div {
                    span { "Vertical" }
                    crate::SegmentGroup { active: vertical_index,
                        crate::SegmentButton { index: 0, "Top" }
                        crate::SegmentButton { index: 1, "Center" }
                        crate::SegmentButton { index: 2, "Bottom" }
                    }
                }
                div {
                    span { "Horizontal" }
                    crate::SegmentGroup { active: horizontal_index,
                        crate::SegmentButton { index: 0, "Start" }
                        crate::SegmentButton { index: 1, "Center" }
                        crate::SegmentButton { index: 2, "End" }
                    }
                }
                div {
                    span { "List side" }
                    crate::SegmentGroup { active: list_side_index,
                        crate::SegmentButton { index: 0, "Top" }
                        crate::SegmentButton { index: 1, "Bottom" }
                        crate::SegmentButton { index: 2, "Start" }
                        crate::SegmentButton { index: 3, "End" }
                    }
                }
                crate::Checkbox { checked: activated, label: "List active".to_string() }
                crate::Checkbox { checked: small, label: "Small main button".to_string() }
                crate::Checkbox { checked: edge, label: "Edge".to_string() }
            },
            crate::AppWrapper { mode: playground_mode, class: "g3-playground-device-app",
                crate::Header { title: "Fab" }
                crate::Body {
                    fab: rsx! {
                        Fab { vertical, horizontal, edge: edge(),
                            FabButton { onclick: |_| {}, size: if small() { FabSize::Small } else { FabSize::Normal }, "+" }
                            FabList { activated: activated(), side: list_side,
                                FabButton { size: FabSize::Small, onclick: |_| {}, "A" }
                                FabButton { size: FabSize::Small, onclick: |_| {}, "B" }
                                FabButton { size: FabSize::Small, onclick: |_| {}, "C" }
                            }
                        }
                    },
                    crate::Card { title: "Actions",
                        "FAB is anchored to the body, and the list opens from the selected side."
                    }
                    crate::Card { title: "Content", "The button stays over scrolling body content." }
                    for index in 1..=8 {
                        crate::Card { title: format!("Row {index}"),
                            "Scrollable content for checking FAB overlap."
                        }
                    }
                }
            }
        }
    }
}
crate::g3_playground! {
    name: "Fab",
    description: "Floating action button container, button, and expandable list.",
    demo: FabPlaygroundDemo,
    source: "src/components/fab.rs",
}
