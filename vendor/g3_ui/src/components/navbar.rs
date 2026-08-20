//! Responsive persistent navigation: bottom tabs on compact shells and a desktop rail on wide ones.

use super::navbar_styles as s;
use crate::theme::{ComponentMode, Theme, merge_classes, use_component_mode};
use dioxus::prelude::*;
#[cfg(feature = "playground")]
use dioxus_icons::lucide::{CalendarDays, CircleUserRound, Trophy};
#[cfg(feature = "transitions")]
use dx_route_transitions::ROUTE_TRANSITION_BASE_CLASS;

#[component]
pub fn Navbar(children: Element, class: Option<String>, mode: Option<ComponentMode>) -> Element {
    let mode = use_component_mode(mode);
    // Like AppWrapper: `[data-g3-mode]` sets its own default color vars, which
    // beat an *inherited* theme value on any element carrying the attribute -
    // including this one, when Navbar is nested inside an already-themed
    // AppWrapper. Re-apply the ambient theme (if any) via inline style so
    // nesting doesn't silently reset the app's theme back to mode defaults.
    let theme_style = try_use_context::<Theme>()
        .unwrap_or_default()
        .to_style_attr();

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
    icon: Option<Element>,
    class: Option<String>,
    onclick: Option<Callback<Event<MouseData>>>,
) -> Element {
    let selected = selected.unwrap_or(false);
    let disabled = disabled.unwrap_or(false);
    let selected_cls = if selected { s::TAB_SELECTED } else { "" };
    let disabled_cls = if disabled { s::TAB_DISABLED } else { "" };

    rsx! {
        button {
            class: merge_classes(format!("{} {selected_cls} {disabled_cls}", s::TAB), class.as_deref()),
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
