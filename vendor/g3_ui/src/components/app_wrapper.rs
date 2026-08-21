//! AppWrapper component - root shell for the entire app layout.

use super::shell_styles as s;
use crate::UI_CSS;
use crate::theme::{
    CSS_PRELOAD_CLASS, ComponentMode, G3Mode, G3PreloadStyle, Theme, merge_classes,
    use_component_mode, use_css_preload_guard,
};
use dioxus::prelude::*;
#[cfg(feature = "transitions")]
use dx_route_transitions::{ROUTE_TRANSITION_COVER_CLASS, RouteTransitionProvider};

#[component]
pub fn AppWrapper(
    children: Element,
    class: Option<String>,
    mode: Option<ComponentMode>,
    theme: Option<Theme>,
    /// Whether to apply the responsive app-shell layout (`flex flex-col h-dvh
    /// overflow-hidden` + mode class). Defaults to `true`. Set to `false` for
    /// a root that provides its own top-level layout (e.g. a desktop page
    /// that should scroll normally) and only wants `AppWrapper` for theme
    /// tokens, the stylesheet link, and the first-paint guard.
    layout: Option<bool>,
    /// Disable text selection/highlighting across the whole app shell
    /// (`input`/`textarea` are exempted so typing still works). Useful for
    /// apps with drag gestures (reorder, swipe, scrubbing) where an
    /// accidental text selection during a drag is visually distracting.
    /// Defaults to `false`.
    disable_text_selection: Option<bool>,
) -> Element {
    let mode = use_component_mode(mode);
    // Nested AppWrapper (a device-frame demo inside a themed page, for
    // instance) shouldn't lose its ambient theme just because it wasn't
    // re-specified: fall back to whatever Theme an outer AppWrapper already
    // provided before reaching for the library default. This has to resolve
    // to a concrete Theme either way (not fall through to an empty inline
    // style) - `[data-g3-mode]` sets its own default color vars, and those
    // beat an *inherited* value on any element that carries the attribute,
    // which every AppWrapper does.
    let effective_theme = theme.or_else(try_use_context::<Theme>).unwrap_or_default();
    let theme_style = effective_theme.to_style_attr();
    provide_context(G3Mode { mode });
    provide_context(effective_theme);

    // The stylesheet is attached at runtime (below), so on first load the DOM is
    // painted before it applies. Without this, overlays (sheets, modals) would
    // render fully unstyled (and, once the transform rules do land, visibly
    // slide out of their unstyled position). Hold a `g3-preload` class - backed
    // by the inline `G3PreloadStyle` guard, since g3_ui.css hasn't loaded yet
    // during this exact window - until the stylesheet is confirmed applied.
    let preloading = use_css_preload_guard();

    let mut shell_cls = if layout.unwrap_or(true) {
        match mode {
            ComponentMode::Ios => format!("{} {}", s::SHELL_BASE, s::SHELL_IOS),
            ComponentMode::Md => format!("{} {}", s::SHELL_BASE, s::SHELL_MD),
        }
    } else {
        String::new()
    };
    if preloading() {
        shell_cls = format!("{shell_cls} {CSS_PRELOAD_CLASS}");
    }
    if disable_text_selection.unwrap_or(false) {
        shell_cls = format!("{shell_cls} {}", s::SHELL_NO_SELECT);
    }
    #[cfg(feature = "transitions")]
    let shell_cls = merge_classes(shell_cls, Some(ROUTE_TRANSITION_COVER_CLASS));
    let shell_cls = merge_classes(shell_cls, class.as_deref());

    let shell = rsx! {
        div {
            class: shell_cls,
            style: theme_style,
            "data-g3-mode": mode.as_str(),
            {children}
        }
    };

    #[cfg(feature = "transitions")]
    return rsx! {
        G3PreloadStyle {}
        document::Link { rel: "stylesheet", href: UI_CSS }
        RouteTransitionProvider { {shell} }
    };

    #[cfg(not(feature = "transitions"))]
    rsx! {
        G3PreloadStyle {}
        document::Link { rel: "stylesheet", href: UI_CSS }
        {shell}
    }
}
#[cfg(feature = "playground")]
#[component]
pub fn AppWrapperPlaygroundDemo() -> Element {
    let playground_mode = crate::use_component_mode(None);
    rsx! {
        crate::PlaygroundDemoFrame { app: false,
            crate::AppWrapper { mode: playground_mode, class: "g3-playground-device-app",
                crate::Header { title: "Games" }
                crate::Body { has_footer_space: false,
                    crate::Card { title: "Shell", "Mode and theme come from the playground controls." }
                    crate::Card { title: "Pending", "A full app shell with header and body." }
                }
            }
        }
    }
}

crate::g3_playground! {
    name: "AppWrapper",
    description: "Root app shell and mode provider.",
    demo: AppWrapperPlaygroundDemo,
    source: "src/components/app_wrapper.rs",
}
