//! Generic View Transition helpers for Dioxus route navigation.
//!
//! This crate is intentionally independent of any app component library. It provides:
//!
//! - [`route_transitions`], an attribute macro for deriving route transition behavior from
//!   route-variant metadata.
//! - [`RouteTransitions`], the trait implemented by the macro and consumed by
//!   [`animated_navigate`].
//! - [`NavigationAnimation`], the animation vocabulary used by the generated route method and the
//!   JS bridge.
//! - [`RouteTransitionProvider`], a small Dioxus provider component that imports the default View
//!   Transition CSS.
//! - [`animated_navigate`] and [`animated_go_back`], Dioxus router helpers that
//!   wrap route changes in `document.startViewTransition` without waiting for
//!   DOM mutations.
//!
//! # Route transition attributes
//!
//! Put `#[route_transitions]` on the same enum that derives Dioxus `Routable`. Then place
//! `#[transition(...)]` on variants that need non-default behavior.
//!
//! ```rust,ignore
//! use dioxus::prelude::*;
//! use dx_route_transitions::route_transitions;
//!
//! #[route_transitions]
//! #[derive(Clone, Routable, PartialEq)]
//! enum Route {
//!     #[transition(base, push(group = sections, order = tab))]
//!     #[route("/sections?:tab")]
//!     Sections { tab: SectionTab },
//!
//!     #[transition(cover)]
//!     #[route("/items/new")]
//!     NewItem {},
//!
//!     #[transition(cover, push(group = item_details, key = item_id, order = tab))]
//!     #[route("/items/:item_id?:tab")]
//!     ItemDetails { item_id: String, tab: ItemTab },
//! }
//! ```
//!
//! Attribute rules:
//!
//! - `base` marks a normal page. It is the default when no transition attribute exists.
//! - `cover` marks a sheet/modal-like route. Navigating `base -> cover` returns
//!   [`NavigationAnimation::CoverUp`]; `cover -> base` returns
//!   [`NavigationAnimation::UncoverDown`].
//! - `morph` marks a route that grows out of a card on a `base` route. Navigating
//!   `base -> morph` returns [`NavigationAnimation::MorphIn`]; `morph -> base` returns
//!   [`NavigationAnimation::MorphOut`].
//! - `push(group = name, order = field)` marks ordered peers inside a static group. The `order`
//!   field must implement [`Ord`]. Moving to a greater order returns `PushLeft`; moving lower
//!   returns `PushRight`.
//! - `key = field` scopes a push group to a route parameter, such as an item id.
//! - `key = (field_a, field_b)` scopes a push group to multiple route parameters.
//! - If two routes are not equivalent, not a cover/uncover pair, and not matching push peers, the
//!   generated method returns [`NavigationAnimation::Fade`].
//!
//! # CSS provider and snapshot markers
//!
//! Wrap the router in [`RouteTransitionProvider`] to load the default CSS. Mark the parts of your
//! app that should participate in named snapshots with these generic markers:
//!
//! - `route-transition-base`: the stable base page under covers.
//! - `route-transition-cover`: the app shell that should slide over or off the base page.
//! - `route-transition-segment`: the body area that should push left/right for peer routes.
//!
//! The runtime names `route-transition-cover` only during cover/uncover transitions so normal
//! rendering and peer-route pushes do not create an extra named snapshot.

use dioxus::{document::eval, prelude::*};
use manganis::{Asset, asset};

pub use dx_route_transitions_macros::route_transitions;

pub static ROUTE_TRANSITIONS_CSS: Asset = asset!("/assets/route_transitions.css");

pub const ROUTE_TRANSITION_BASE_CLASS: &str = "route-transition-base";
pub const ROUTE_TRANSITION_COVER_CLASS: &str = "route-transition-cover";
pub const ROUTE_TRANSITION_SEGMENT_CLASS: &str = "route-transition-segment";

fn merge_transition_class(base: &'static str, extra: Option<&str>) -> String {
    match extra {
        Some(extra) if !extra.is_empty() => format!("{base} {extra}"),
        _ => base.to_string(),
    }
}

#[component]
pub fn RouteTransitionBase(children: Element, class: Option<String>) -> Element {
    let class = merge_transition_class(ROUTE_TRANSITION_BASE_CLASS, class.as_deref());

    rsx! {
        div { class, {children} }
    }
}

#[component]
pub fn RouteTransitionCover(children: Element, class: Option<String>) -> Element {
    let class = merge_transition_class(ROUTE_TRANSITION_COVER_CLASS, class.as_deref());

    rsx! {
        div { class, {children} }
    }
}

#[component]
pub fn RouteTransitionSegment(children: Element, class: Option<String>) -> Element {
    let class = merge_transition_class(ROUTE_TRANSITION_SEGMENT_CLASS, class.as_deref());

    rsx! {
        div { class, {children} }
    }
}

/// The animation vocabulary shared by the generated route method and the JS
/// bridge. Each variant is a *semantic* navigation event (push into a
/// hierarchy, present a modal, morph a card into its detail view, ...); the
/// stylesheet gives each one a platform-specific look via
/// [`Platform::data_value`], so the same variant renders as an iOS parallax
/// push on `ios` and a Material shared-axis slide on `md`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NavigationAnimation {
    None,
    #[default]
    Fade,
    PushLeft,
    PushRight,
    CoverUp,
    UncoverDown,
    /// A card-like element growing into its own full-screen detail route
    /// (Material "container transform"; approximated on iOS as a soft
    /// scale/fade since iOS has no native equivalent).
    MorphIn,
    /// The reverse of [`NavigationAnimation::MorphIn`]: a detail route
    /// shrinking back down into the card that opened it.
    MorphOut,
}

impl NavigationAnimation {
    pub fn data_value(self) -> &'static str {
        match self {
            NavigationAnimation::None => "none",
            NavigationAnimation::Fade => "fade",
            NavigationAnimation::PushLeft => "push-left",
            NavigationAnimation::PushRight => "push-right",
            NavigationAnimation::CoverUp => "cover-up",
            NavigationAnimation::UncoverDown => "uncover-down",
            NavigationAnimation::MorphIn => "morph-in",
            NavigationAnimation::MorphOut => "morph-out",
        }
    }
}

/// Platform styling mode for the transition stylesheet - mirrors the
/// iOS-vs-Material split app component libraries (e.g. Ionic, g3_ui) already
/// use for widget styling, so the same signal can drive both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    /// UINavigationController/UIKit-flavored motion: parallax push where the
    /// outgoing page slides back and dims rather than leaving the screen,
    /// page-sheet modals, quick cross-dissolves.
    Ios,
    /// Material Design 3 motion: shared-axis slides, fade-through, modal
    /// bottom sheets with a scrim.
    Md,
}

impl Platform {
    pub fn data_value(self) -> &'static str {
        match self {
            Platform::Ios => "ios",
            Platform::Md => "md",
        }
    }
}

thread_local! {
    static GLOBAL_PLATFORM: std::cell::Cell<Option<Platform>> = const { std::cell::Cell::new(None) };
}

/// Explicitly set the platform used to pick a transition's visual style.
/// Call this once at startup and again whenever the app's platform mode
/// changes (e.g. a user-facing iOS/Material style toggle).
pub fn set_platform(platform: Platform) {
    GLOBAL_PLATFORM.with(|p| p.set(Some(platform)));
}

/// The platform currently used to style transitions. Falls back to
/// compile-time/runtime auto-detection if [`set_platform`] was never called.
pub fn get_platform() -> Platform {
    GLOBAL_PLATFORM
        .with(|p| p.get())
        .unwrap_or_else(detect_platform)
}

/// Detect a reasonable default platform from `cfg(target_os)` (native
/// mobile builds) or, on wasm, from the user agent.
pub fn detect_platform() -> Platform {
    #[cfg(target_os = "ios")]
    {
        Platform::Ios
    }
    #[cfg(target_os = "android")]
    {
        Platform::Md
    }
    #[cfg(all(
        target_arch = "wasm32",
        not(target_os = "ios"),
        not(target_os = "android")
    ))]
    {
        detect_platform_web()
    }
    #[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
    {
        Platform::Md
    }
}

#[cfg(target_arch = "wasm32")]
fn detect_platform_web() -> Platform {
    let Some(window) = web_sys::window() else {
        return Platform::Md;
    };
    let navigator = window.navigator();
    let user_agent = navigator.user_agent().unwrap_or_default().to_lowercase();
    let platform = navigator.platform().unwrap_or_default().to_lowercase();
    let max_touch_points = navigator.max_touch_points();

    let is_iphone_or_ipod = user_agent.contains("iphone") || user_agent.contains("ipod");
    let is_ipad = user_agent.contains("ipad")
        || (platform.contains("mac") && max_touch_points > 1 && user_agent.contains("safari"));

    if is_iphone_or_ipod || is_ipad {
        Platform::Ios
    } else {
        Platform::Md
    }
}

/// Initialize the global platform from compile-time/runtime auto-detection.
/// Prefer calling [`set_platform`] directly when the host app already tracks
/// an explicit iOS/Material mode.
pub fn init_auto_platform() {
    set_platform(detect_platform());
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RouteTransitionLayer {
    #[default]
    Base,
    Cover,
    /// A card-like element that morphs into (and back out of) its own
    /// full-screen route, distinct from a modal `Cover` layer.
    Morph,
}

pub trait RouteTransitions: PartialEq {
    fn transition_to(&self, next: &Self) -> NavigationAnimation;
}

#[component]
pub fn RouteTransitionProvider(children: Element) -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: ROUTE_TRANSITIONS_CSS }
        {children}
    }
}

#[component]
pub fn RouteTransitionRoot(children: Element, class: Option<String>) -> Element {
    let class = merge_transition_class(ROUTE_TRANSITION_COVER_CLASS, class.as_deref());

    rsx! {
        RouteTransitionProvider {
            div { class, {children} }
        }
    }
}

/// Placeholders substituted before the script is evaluated.
///
/// These used to arrive over the eval channel, which cost two round trips
/// between wasm and JS before the transition could start — dead time between
/// the tap and the first frame of the animation. Both values are known on the
/// Rust side already, so they are written into the script instead. They come
/// from fixed enums, so there is nothing to escape.
const ANIMATION_PLACEHOLDER: &str = "__DX_ROUTE_TRANSITION_ANIMATION__";
const PLATFORM_PLACEHOLDER: &str = "__DX_ROUTE_TRANSITION_PLATFORM__";

const VIEW_TRANSITION_NAVIGATE: &str = r#"
const animation = "__DX_ROUTE_TRANSITION_ANIMATION__";
const platform = "__DX_ROUTE_TRANSITION_PLATFORM__";
const prefersReducedMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches ?? false;
// Resolves once the router has replaced the page, or after a ceiling if it
// renders something indistinguishable.
//
// This must be started *before* the route is asked for, so the observer is
// already live when the update lands. Starting it afterwards meant the mutation
// had usually already happened, leaving nothing to observe and the ceiling as
// the de facto wait — measured at a flat ~133ms of doing nothing after the new
// route was on screen, on every navigation.
//
// There is deliberately no requestAnimationFrame fallback: rendering is paused
// inside a view transition's update callback, so frames do not tick and it
// could never fire.
const dxRouteTransitionRouteRendered = () => new Promise((resolve) => {
    let settled = false;
    const finish = () => {
        if (settled) return;
        settled = true;
        window.clearTimeout(ceiling);
        observer?.disconnect?.();
        resolve();
    };
    const observer = typeof MutationObserver === "undefined" ? null : new MutationObserver(() => finish());
    // Only structural changes: an attribute tick from a clock or a progress bar
    // is not the route arriving.
    observer?.observe?.(document.body ?? document.documentElement, {
        childList: true,
        subtree: true,
    });
    const ceiling = window.setTimeout(() => finish(), 120);
    if (!observer) finish();
});

try {
    if (!document.startViewTransition || prefersReducedMotion) {
        dioxus.send("navigate");
        dioxus.send("done");
    } else {
        document.documentElement.dataset.routeTransition = animation;
        document.documentElement.dataset.routeTransitionPlatform = platform;

        // The outgoing snapshot is taken synchronously inside
        // startViewTransition, so the attributes set above have to reach
        // computed style before that call. Without this flush an element whose
        // `view-transition-name` is granted by those attributes is still
        // unnamed when it is captured, and so gets no group at all.
        //
        // The failure is asymmetric and easy to miss: a cover entering needs
        // its name only in the *new* state, which is styled later anyway and
        // works, while the same cover leaving needs it in the old state and
        // silently drops out of the transition.
        void document.documentElement.offsetHeight;

        const transition = document.startViewTransition(async () => {
            // Watch first, then ask. The router usually renders while the ack
            // is still in flight, so an observer started afterwards has already
            // missed the only mutation it cares about.
            const rendered = dxRouteTransitionRouteRendered();

            dioxus.send("navigate");

            const routeCommit = await dioxus.recv();
            if (routeCommit !== "navigated") {
                throw new Error(`unexpected route transition ack: ${routeCommit}`);
            }

            await rendered;
        });

        try {
            await transition.ready;
        } catch (_) {
        }

        try {
            await transition.finished;
        } catch (_) {
        }

        dioxus.send("done");
    }
} catch (_) {
    dioxus.send("fallback");
} finally {
    delete document.documentElement.dataset.routeTransition;
    delete document.documentElement.dataset.routeTransitionPlatform;
}
"#;

async fn run_animated_navigation(animation: NavigationAnimation, mut navigate: impl FnMut()) {
    let script = VIEW_TRANSITION_NAVIGATE
        .replace(ANIMATION_PLACEHOLDER, animation.data_value())
        .replace(PLATFORM_PLACEHOLDER, get_platform().data_value());
    let mut transition = eval(&script);
    let mut navigated = false;

    loop {
        match transition.recv::<String>().await.as_deref() {
            Ok("navigate") => {
                if !navigated {
                    navigate();
                    navigated = true;
                }
                _ = transition.send("navigated");
            }
            Ok("done") => break,
            Ok("fallback") | Err(_) => {
                if !navigated {
                    navigate();
                }
                break;
            }
            _ => {}
        }
    }
}

pub async fn animated_navigate<Route>(route: Route)
where
    Route: Clone + ToString + RouteTransitions + Routable + 'static,
{
    let current_route = router().current::<Route>().clone();
    let animation = current_route.transition_to(&route);
    let navigator = use_navigator();
    let route = route.to_string();

    if animation == NavigationAnimation::None {
        _ = navigator.push(route);
        return;
    }

    run_animated_navigation(animation, || {
        _ = navigator.push(route.clone());
    })
    .await;
}

/// Pop the router history while using `fallback` to select the reverse
/// transition and as the destination when no history entry exists.
///
/// Calling [`dioxus_router::prelude::Navigator::go_back`] directly commits the
/// route before a View Transition can take its outgoing snapshot. This helper
/// starts the snapshot first, then performs the actual history traversal from
/// inside the same acknowledgement handshake as [`animated_navigate`].
pub async fn animated_go_back<Route>(fallback: Route)
where
    Route: Clone + ToString + RouteTransitions + Routable + 'static,
{
    let navigator = use_navigator();
    if !navigator.can_go_back() {
        animated_navigate(fallback).await;
        return;
    }

    let current_route = router().current::<Route>().clone();
    let animation = current_route.transition_to(&fallback);
    if animation == NavigationAnimation::None {
        navigator.go_back();
        return;
    }

    run_animated_navigation(animation, || navigator.go_back()).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_transitions_dim_the_full_base_snapshot_per_platform() {
        let stylesheet = include_str!("../assets/route_transitions.css");

        assert!(stylesheet.contains("cover-up\"]::view-transition-new(cover)"));
        assert!(stylesheet.contains("uncover-down\"]::view-transition-old(cover)"));
        assert!(!stylesheet.contains("[data-route-transition-layer=\"sheet\"]"));
        assert!(stylesheet.contains("route-transition-ios-dim-base"));
        assert!(stylesheet.contains("route-transition-md-dim-base"));
        assert!(stylesheet.contains("filter: brightness"));
        assert!(stylesheet.contains("cover-up\"]::view-transition-old(cover)"));
        assert!(stylesheet.contains("uncover-down\"]::view-transition-new(cover)"));
        assert!(stylesheet.contains("opacity: 0"));
    }

    #[test]
    fn cover_transitions_are_scoped_by_platform_attribute() {
        let stylesheet = include_str!("../assets/route_transitions.css");

        assert!(stylesheet.contains(
            "html[data-route-transition-platform=\"ios\"][data-route-transition=\"cover-up\"]::view-transition-old(base)"
        ));
        assert!(stylesheet.contains(
            "html[data-route-transition-platform=\"md\"][data-route-transition=\"cover-up\"]::view-transition-old(base)"
        ));
        assert!(!stylesheet.contains("route-transition-mobile-dim-base"));
        assert!(!stylesheet.contains("route-transition-mobile-undim-base"));
        // iOS gets the page-sheet scale/round treatment; Material does not.
        assert!(stylesheet.contains("border-radius: 12px"));
    }

    #[test]
    fn cover_transitions_do_not_apply_debug_offsets_to_snapshots() {
        let stylesheet = include_str!("../assets/route_transitions.css");

        assert!(!stylesheet.contains("--route-transition-debug-peek"));
        assert!(!stylesheet.contains("--route-transition-cover-x"));
        assert!(!stylesheet.contains("--route-transition-base-x"));
        assert!(stylesheet.contains("transform: translateY(100vh)"));
        assert!(stylesheet.contains("transform: translateY(0vh)"));
        assert!(!stylesheet.contains("translateX(var(--route-transition-base-x))"));
    }

    #[test]
    fn cover_transitions_force_active_snapshots_to_paint_above_base() {
        let stylesheet = include_str!("../assets/route_transitions.css");

        assert!(stylesheet.contains("cover-up\"]::view-transition-new(cover),"));
        assert!(stylesheet.contains("uncover-down\"]::view-transition-old(cover)"));
        assert!(stylesheet.contains("mix-blend-mode: normal"));
        assert!(stylesheet.contains("opacity: 1"));
        assert!(stylesheet.contains("z-index: 2"));
        assert!(stylesheet.contains("cover-up\"]::view-transition-old(base),"));
        assert!(stylesheet.contains("uncover-down\"]::view-transition-new(base)"));
        assert!(stylesheet.contains("z-index: 1"));
    }
    #[test]
    fn snapshot_marker_components_are_public_contract() {
        let source = include_str!("lib.rs");
        let production_source = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source precedes tests");

        assert!(production_source.contains("pub const ROUTE_TRANSITION_BASE_CLASS"));
        assert!(production_source.contains("pub const ROUTE_TRANSITION_COVER_CLASS"));
        assert!(production_source.contains("pub const ROUTE_TRANSITION_SEGMENT_CLASS"));
        assert!(production_source.contains("pub fn RouteTransitionBase"));
        assert!(production_source.contains("pub fn RouteTransitionCover"));
        assert!(production_source.contains("pub fn RouteTransitionSegment"));
        assert!(production_source.contains("merge_transition_class"));
    }
    #[test]
    fn route_transition_root_wraps_provider_and_cover_marker() {
        let source = include_str!("lib.rs");

        assert!(source.contains("pub fn RouteTransitionRoot"));
        assert!(source.contains("RouteTransitionProvider"));
        assert!(source.contains("ROUTE_TRANSITION_COVER_CLASS"));
        assert!(source.contains("merge_transition_class(ROUTE_TRANSITION_COVER_CLASS"));
        assert!(source.contains("div { class, {children} }"));
    }
    #[test]
    fn style_is_flushed_before_the_outgoing_snapshot_is_taken() {
        // The attributes granting view-transition-name have to reach computed
        // style before startViewTransition captures the old state, or a region
        // leaving the page is captured unnamed and gets no group. Order
        // matters, so this asserts the flush sits between the two.
        let script = VIEW_TRANSITION_NAVIGATE;
        let attributes = script
            .find("dataset.routeTransitionPlatform = platform")
            .expect("platform attribute is set");
        let flush = script
            .find("void document.documentElement.offsetHeight")
            .expect("style is flushed");
        let capture = script
            .find("document.startViewTransition(async () =>")
            .expect("transition is started");
        assert!(
            attributes < flush,
            "the flush must come after the attributes"
        );
        assert!(flush < capture, "the flush must come before the snapshot");
    }

    #[test]
    fn view_transition_update_waits_for_native_route_commit_without_blocking_on_raf() {
        assert!(VIEW_TRANSITION_NAVIGATE.contains("document.startViewTransition(async () =>"));
        assert!(VIEW_TRANSITION_NAVIGATE.contains("dioxus.send(\"navigate\")"));
        assert!(VIEW_TRANSITION_NAVIGATE.contains("const routeCommit = await dioxus.recv()"));
        assert!(VIEW_TRANSITION_NAVIGATE.contains("routeCommit !== \"navigated\""));
        // The wait has to be armed before the route is asked for, or it misses
        // the render it exists to observe and falls back to its ceiling.
        // Scoped to the callback: the reduced-motion branch above it also asks
        // for the route, and would otherwise match first.
        let callback = VIEW_TRANSITION_NAVIGATE
            .find("document.startViewTransition(async () =>")
            .expect("the transition is started");
        let body = &VIEW_TRANSITION_NAVIGATE[callback..];
        let armed = body
            .find("const rendered = dxRouteTransitionRouteRendered();")
            .expect("the wait is armed");
        let asked = body
            .find("dioxus.send(\"navigate\")")
            .expect("the route is asked for");
        assert!(
            armed < asked,
            "the observer must be live before the request"
        );
        assert!(VIEW_TRANSITION_NAVIGATE.contains("await rendered;"));
        // Frames do not tick inside a transition callback, so the frame-based
        // fallback could never fire and is gone.
        assert!(!VIEW_TRANSITION_NAVIGATE.contains("dxRouteTransitionNextFrame"));
        assert!(VIEW_TRANSITION_NAVIGATE.contains("await transition.ready"));
        assert!(!VIEW_TRANSITION_NAVIGATE.contains("console.info"));
        assert!(!VIEW_TRANSITION_NAVIGATE.contains("g3RouteTransition"));
    }

    #[test]
    fn rust_side_acknowledges_navigation_after_router_push() {
        let source = include_str!("lib.rs");
        let production_source = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source precedes tests");

        assert!(production_source.contains("Ok(\"navigate\")"));
        assert!(production_source.contains("transition.send(\"navigated\")"));
        assert!(!production_source.contains("eprintln!"));
    }

    #[test]
    fn history_back_uses_the_view_transition_handshake() {
        let source = include_str!("lib.rs");
        let production_source = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source precedes tests");
        let helper = production_source
            .split("pub async fn animated_go_back")
            .nth(1)
            .expect("animated_go_back is public");

        assert!(helper.contains("navigator.can_go_back()"));
        assert!(helper.contains("animated_navigate(fallback).await"));
        assert!(helper.contains("run_animated_navigation(animation"));
        assert!(helper.contains("navigator.go_back()"));
    }

    #[test]
    fn a_cross_dissolve_never_uncovers_what_is_behind_the_page() {
        // Only the outgoing page may animate. Fading both at once depends on
        // plus-lighter compositing to hold full coverage through the middle of
        // the dissolve, and engines that ignore it show the backdrop as a
        // flash.
        let stylesheet = include_str!("../assets/route_transitions.css");
        assert!(
            !stylesheet.contains("route-transition-fade-in"),
            "nothing may fade in during a cross-dissolve"
        );
        assert!(
            stylesheet.contains("route-transition-fade-out"),
            "the outgoing page still fades out"
        );
    }

    #[test]
    fn route_transition_css_uses_production_durations() {
        let stylesheet = include_str!("../assets/route_transitions.css");

        assert!(stylesheet.contains("--route-transition-cover-duration: 0.6s"));
        assert!(stylesheet.contains("--route-transition-push-duration: 260ms"));
        assert!(stylesheet.contains("--route-transition-fade-duration: 200ms"));
        assert!(stylesheet.contains("--route-transition-morph-duration: 350ms"));
        assert!(!stylesheet.contains("route-transition-duration-debug"));
        assert!(!stylesheet.contains("route-transition-mobile-dim-base"));
        assert!(!stylesheet.contains("route-transition-mobile-undim-base"));
    }

    #[test]
    fn push_transitions_diverge_between_ios_parallax_and_md_shared_axis() {
        let stylesheet = include_str!("../assets/route_transitions.css");

        // iOS: outgoing page parallax-shifts and dims rather than leaving.
        assert!(stylesheet.contains("route-transition-ios-push-out-left"));
        assert!(stylesheet.contains("translateX(-30%); filter: brightness(0.85)"));
        // Material: symmetric shared-axis slide+fade, no dimming.
        assert!(stylesheet.contains("route-transition-md-axis-out-left"));
        assert!(stylesheet.contains("route-transition-md-axis-in-left"));
    }

    #[test]
    fn fade_transitions_diverge_between_ios_cross_dissolve_and_md_fade_through() {
        let stylesheet = include_str!("../assets/route_transitions.css");

        assert!(stylesheet.contains(
            "html[data-route-transition-platform=\"ios\"][data-route-transition=\"fade\"]"
        ));
        assert!(stylesheet.contains("route-transition-md-fade-through-out"));
        assert!(stylesheet.contains("route-transition-md-fade-through-in"));
        assert!(stylesheet.contains("animation-delay"));
    }

    #[test]
    fn morph_transitions_exist_for_both_platforms() {
        let stylesheet = include_str!("../assets/route_transitions.css");

        assert!(stylesheet.contains("data-route-transition=\"morph-in\""));
        assert!(stylesheet.contains("data-route-transition=\"morph-out\""));
        assert!(stylesheet.contains("route-transition-morph-grow-in"));
        assert!(stylesheet.contains("route-transition-morph-shrink-out"));
    }

    #[test]
    fn animation_data_values_match_css_contract() {
        assert_eq!(NavigationAnimation::None.data_value(), "none");
        assert_eq!(NavigationAnimation::Fade.data_value(), "fade");
        assert_eq!(NavigationAnimation::PushLeft.data_value(), "push-left");
        assert_eq!(NavigationAnimation::PushRight.data_value(), "push-right");
        assert_eq!(NavigationAnimation::CoverUp.data_value(), "cover-up");
        assert_eq!(
            NavigationAnimation::UncoverDown.data_value(),
            "uncover-down"
        );
        assert_eq!(NavigationAnimation::MorphIn.data_value(), "morph-in");
        assert_eq!(NavigationAnimation::MorphOut.data_value(), "morph-out");
    }

    #[test]
    fn platform_data_values_match_css_contract() {
        assert_eq!(Platform::Ios.data_value(), "ios");
        assert_eq!(Platform::Md.data_value(), "md");
    }

    #[test]
    fn set_platform_overrides_auto_detection() {
        set_platform(Platform::Ios);
        assert_eq!(get_platform(), Platform::Ios);
        set_platform(Platform::Md);
        assert_eq!(get_platform(), Platform::Md);
    }

    #[test]
    fn animation_and_platform_are_written_in_rather_than_awaited() {
        // Nothing may be awaited before the transition starts. Every round trip
        // over the eval channel is dead time between the tap and the first
        // frame of the animation, which is the one thing the user feels.
        let start = VIEW_TRANSITION_NAVIGATE
            .find("document.startViewTransition")
            .expect("the transition is started");
        assert!(
            !VIEW_TRANSITION_NAVIGATE[..start].contains("await dioxus.recv()"),
            "no round trip may precede the snapshot"
        );

        let filled = VIEW_TRANSITION_NAVIGATE
            .replace(
                ANIMATION_PLACEHOLDER,
                NavigationAnimation::CoverUp.data_value(),
            )
            .replace(PLATFORM_PLACEHOLDER, Platform::Ios.data_value());
        assert!(filled.contains(r#"const animation = "cover-up";"#));
        assert!(filled.contains(r#"const platform = "ios";"#));
        assert!(
            !filled.contains("__DX_ROUTE_TRANSITION"),
            "every placeholder is substituted"
        );
        assert!(
            filled.contains("document.documentElement.dataset.routeTransitionPlatform = platform;")
        );
        assert!(!filled.contains("navigator.userAgent"));
    }
}
