//! Responsive persistent navigation: bottom tabs on compact shells and a desktop rail on wide ones.
use super::navbar_styles as s;
use crate::theme::{ComponentMode, merge_classes, use_ambient_theme, use_component_mode};
use dioxus::prelude::*;
#[cfg(feature = "playground")]
use dioxus_icons::lucide::{CalendarDays, CircleUserRound, Trophy};
#[cfg(feature = "transitions")]
use g3_route_transitions::ROUTE_TRANSITION_BASE_CLASS;
/// Where a tab belongs when the bottom tab bar becomes a desktop rail.
///
/// This does not affect compact layouts: every tab remains in its declared
/// order in the mobile bottom bar. Tabs marked [`Bottom`](Self::Bottom) are
/// grouped at the bottom of the desktop rail, which is useful for account,
/// profile, and settings destinations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NavbarTabDesktopPlacement {
    /// Keep the tab with the primary destinations at the top of the rail.
    #[default]
    Top,
    /// Group the tab with secondary destinations at the bottom of the rail.
    Bottom,
}
#[component]
pub fn Navbar(children: Element, class: Option<String>, mode: Option<ComponentMode>) -> Element {
    let mode = use_component_mode(mode);
    let theme_style = use_ambient_theme().unwrap_or_default().to_style_attr();
    let navbar_cls = match mode {
        ComponentMode::Ios => format!("{} {}", s::NAVBAR_BASE, s::NAVBAR_IOS),
        ComponentMode::Md => format!("{} {}", s::NAVBAR_BASE, s::NAVBAR_MD),
    };
    #[cfg(feature = "transitions")]
    let navbar_cls = merge_classes(navbar_cls, Some(ROUTE_TRANSITION_BASE_CLASS));
    let navbar_cls = merge_classes(navbar_cls, class.as_deref());
    rsx! {
        div {
            class: navbar_cls,
            style: theme_style,
            "data-g3-mode": mode.as_str(),
            {children}
        }
    }
}
#[component]
pub fn NavbarTabBar(
    class: Option<String>,
    aria_label: Option<String>,
    children: Element,
) -> Element {
    rsx! {
        nav {
            class: merge_classes(s::TAB_BAR, class.as_deref()),
            role: "tablist",
            aria_label: aria_label.unwrap_or_else(|| "Primary navigation".to_string()),
            {children}
        }
    }
}
#[component]
pub fn NavbarTab(
    label: String,
    selected: Option<bool>,
    disabled: Option<bool>,
    desktop_placement: Option<NavbarTabDesktopPlacement>,
    icon: Option<Element>,
    class: Option<String>,
    onclick: Option<Callback<Event<MouseData>>>,
) -> Element {
    let selected = selected.unwrap_or(false);
    let disabled = disabled.unwrap_or(false);
    let desktop_placement = desktop_placement.unwrap_or_default();
    let selected_cls = if selected { s::TAB_SELECTED } else { "" };
    let disabled_cls = if disabled { s::TAB_DISABLED } else { "" };
    let desktop_placement_cls = match desktop_placement {
        NavbarTabDesktopPlacement::Top => "",
        NavbarTabDesktopPlacement::Bottom => s::TAB_DESKTOP_BOTTOM,
    };
    rsx! {
        button {
            class: merge_classes(
                format!("{} {selected_cls} {disabled_cls} {desktop_placement_cls}", s::TAB),
                class.as_deref(),
            ),
            r#type: "button",
            role: "tab",
            aria_label: label.clone(),
            disabled,
            aria_selected: selected.to_string(),
            title: label.clone(),
            onclick: move |event| {
                if disabled {
                    return;
                }
                if let Some(onclick) = onclick {
                    onclick.call(event);
                }
            },
            span { class: s::TAB_ICON, aria_hidden: "true",
                if let Some(icon) = icon {
                    {icon}
                }
            }
            span { class: s::TAB_LABEL, "{label}" }
        }
    }
}
#[cfg(feature = "playground")]
#[component]
pub fn NavbarPlaygroundDemo() -> Element {
    let playground_mode = crate::use_component_mode(None);
    let mut active = use_signal(|| 0_usize);
    rsx! {
        crate::PlaygroundDemoFrame { app: false,
            crate::AppWrapper { mode: playground_mode, class: "g3-playground-device-app",
                Navbar {
                    crate::Header { title: "Navbar" }
                    crate::Body { has_footer_space: false,
                        crate::Card { title: "Content", "Route content sits above a persistent navigation bar." }
                    }
                    NavbarTabBar {
                        NavbarTab {
                            label: "Games".to_string(),
                            selected: active() == 0,
                            icon: rsx! {
                                Trophy { size: 20, class: "fill-none" }
                            },
                            onclick: move |_| active.set(0),
                        }
                        NavbarTab {
                            label: "Tourneys".to_string(),
                            selected: active() == 1,
                            icon: rsx! {
                                CalendarDays { size: 20, class: "fill-none" }
                            },
                            onclick: move |_| active.set(1),
                        }
                        NavbarTab {
                            label: "Account".to_string(),
                            selected: active() == 2,
                            desktop_placement: NavbarTabDesktopPlacement::Bottom,
                            icon: rsx! {
                                CircleUserRound { size: 20, class: "fill-none" }
                            },
                            onclick: move |_| active.set(2),
                        }
                    }
                }
            }
        }
    }
}
crate::g3_playground! {
    name: "Navbar",
    description: "Responsive bottom tabs that become a desktop navigation rail.",
    demo: NavbarPlaygroundDemo,
    source: "src/components/navbar.rs",
}
