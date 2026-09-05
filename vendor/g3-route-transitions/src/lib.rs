#![warn(missing_docs)]
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
//! - With the `native-back` feature, [`use_native_back_navigation`] connects
//!   Android system Back from `g3-native-plugins` to the same animated router
//!   pop without app-specific bridge code.
//!
//! # Route transition attributes
//!
//! Put `#[route_transitions]` on the same enum that derives Dioxus `Routable`. Then place
//! `#[transition(...)]` on variants that need non-default behavior.
//!
//! ```rust,ignore
//! use dioxus::prelude::*;
//! use g3_route_transitions::route_transitions;
//!
//! #[route_transitions]
//! #[derive(Clone, Routable, PartialEq)]
//! enum Route {
//!     #[transition(root, replace)]
//!     #[route("/sections?:tab")]
//!     Sections { tab: SectionTab },
//!
//!     #[transition(cover)]
//!     #[route("/items/new")]
//!     NewItem {},
//!
//!     #[transition(pushed, replace(key = item_id), forward = ItemComments)]
//!     #[route("/items/:item_id?:tab")]
//!     ItemDetails { item_id: String, tab: ItemTab },
//!
//!     #[transition(pushed)]
//!     #[route("/items/:item_id/comments")]
//!     ItemComments { item_id: String },
//! }
//! ```
//!
//! Attribute rules:
//!
//! - `base` marks a normal page. It is the default when no transition attribute exists.
//! - `root` marks a stable application root such as a bottom-tab destination.
//! - `pushed` marks a full-screen page above a root. Root-to-pushed navigation
//!   uses [`NavigationAnimation::PushLeft`], and the reverse uses
//!   [`NavigationAnimation::PushRight`].
//! - `cover` marks a sheet/modal-like route. Navigating into a cover returns
//!   [`NavigationAnimation::CoverUp`]; navigating out returns
//!   [`NavigationAnimation::UncoverDown`].
//! - `morph` marks a route that grows out of a card on a `base` route. Navigating
//!   `base -> morph` returns [`NavigationAnimation::MorphIn`]; `morph -> base` returns
//!   [`NavigationAnimation::MorphOut`].
//! - `push(group = name, order = field)` marks ordered peers inside a static group. The `order`
//!   field must implement [`Ord`]. Moving to a greater order returns `PushLeft`; moving lower
//!   returns `PushRight`.
//! - `key = field` scopes a push group to a route parameter, such as an item id.
//! - `key = (field_a, field_b)` scopes a push group to multiple route parameters.
//! - `forward = RouteA` or `forward = (RouteA, RouteB)` declares route variants
//!   reached by drilling further into the hierarchy. The forward direction
//!   pushes left and the reverse direction pushes right.
//! - `replace` marks changes within the same route variant as in-place updates.
//!   [`animated_navigate`] uses router replacement and skips the page transition,
//!   so query-backed filters do not fill browser history. `replace(key = id)`
//!   limits that behavior to matching logical records.
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
//! - `route-transition-page`: one full-viewport snapshot for pushed-page and
//!   sheet transitions. Prefer [`RouteTransitionPage`] around each routed page
//!   when the shell contains other transition markers.
//!
//! The runtime names `route-transition-cover` only during cover/uncover transitions so normal
//! rendering and peer-route pushes do not create an extra named snapshot.
use dioxus::{document::eval, prelude::*};
pub use g3_route_transitions_macros::route_transitions;
use manganis::{Asset, asset};
/// The stylesheet backing every transition. Link it once via
/// [`RouteTransitionProvider`], or attach it yourself if the app manages its
/// own `document::Link` tags.
pub static ROUTE_TRANSITIONS_CSS: Asset = asset!("/assets/route_transitions.css");
/// Marks the element that holds ordinary page content. Applied by
/// [`RouteTransitionRoot`]; the runtime gives it a view-transition name for
/// push/fade animations.
pub const ROUTE_TRANSITION_BASE_CLASS: &str = "route-transition-base";
/// Marks the element that rides above the base layer during a cover/uncover
/// (modal) transition. Named only while such a transition is running, so
/// normal rendering does not pay for an extra snapshot.
pub const ROUTE_TRANSITION_COVER_CLASS: &str = "route-transition-cover";
/// Marks a sub-region that should animate independently of the page around
/// it - a tab body swapping under a fixed header, for instance.
pub const ROUTE_TRANSITION_SEGMENT_CLASS: &str = "route-transition-segment";
/// Marks a full-viewport routed page. During push and sheet transitions this
/// becomes one stable snapshot, avoiding nested header/body snapshots that can
/// drift or overlap in embedded WebViews.
pub const ROUTE_TRANSITION_PAGE_CLASS: &str = "route-transition-page";
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
/// Wraps one routed page in the library's full-viewport snapshot marker.
///
/// This is useful for application shells whose header, body, or navigation
/// components already carry [`RouteTransitionBase`] or
/// [`RouteTransitionSegment`] markers. The stylesheet suppresses those nested
/// markers only inside this wrapper and captures the page as a single image.
#[component]
pub fn RouteTransitionPage(children: Element, class: Option<String>) -> Element {
    let class = merge_transition_class(ROUTE_TRANSITION_PAGE_CLASS, class.as_deref());
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
    /// No animation: navigate immediately, skipping the view transition
    /// entirely.
    None,
    /// A plain cross-dissolve. The default, and the safe choice when no
    /// spatial relationship between the two routes is implied.
    #[default]
    Fade,
    /// Forward motion deeper into a hierarchy - the new route enters from the
    /// trailing edge.
    PushLeft,
    /// Backward motion out of a hierarchy - the reverse of
    /// [`NavigationAnimation::PushLeft`].
    PushRight,
    /// A modal rising over the current route, which stays in place beneath it.
    CoverUp,
    /// A modal dropping away to reveal the route beneath, the reverse of
    /// [`NavigationAnimation::CoverUp`].
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
    /// The `data-route-transition` attribute value the stylesheet keys on.
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
/// iOS-vs-Material split app component libraries (e.g. Ionic, g3-ui) already
/// use for widget styling, so the same signal can drive both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    /// UINavigationController/UIKit-flavored motion: parallax push where the
    /// outgoing page slides back and dims rather than leaving the screen,
    /// page-sheet modals, quick cross-dissolves.
    Ios,
    /// Material Design 3 motion: shared-axis slides, quick cross-dissolves,
    /// and modal bottom sheets with a scrim.
    Md,
}
impl Platform {
    /// The `data-route-platform` attribute value the stylesheet keys on.
    pub fn data_value(self) -> &'static str {
        match self {
            Platform::Ios => "ios",
            Platform::Md => "md",
        }
    }
}
thread_local! {
    static GLOBAL_PLATFORM: std::cell::Cell<Option<Platform>> = const {
        std::cell::Cell::new(None)
    };
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
/// Which stacking layer a route occupies during a transition. The runtime
/// uses this to decide which element gets a view-transition name, so a modal
/// can animate over a page that is itself not moving.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RouteTransitionLayer {
    /// Ordinary page content, animating in the base layer.
    #[default]
    Base,
    /// A modal or sheet riding above the base layer.
    Cover,
    /// A card-like element that morphs into (and back out of) its own
    /// full-screen route, distinct from a modal `Cover` layer.
    Morph,
    /// A stable application root such as a bottom-tab destination.
    Root,
    /// A full-screen page pushed above an application root.
    Pushed,
}
/// Implemented by a `Route` enum to declare how it animates toward each of
/// its peers. The `#[route_transitions]` macro generates this, but it can be
/// written by hand when the choice depends on route data.
pub trait RouteTransitions: PartialEq {
    /// The animation to play when navigating from `self` to `next`.
    fn transition_to(&self, next: &Self) -> NavigationAnimation;
    /// Whether navigation from `self` to `next` should replace the current
    /// browser-history entry rather than push a new one.
    ///
    /// The route macro uses this for `#[transition(replace)]` declarations.
    /// Manual implementations can leave the default when every navigation
    /// should add an entry.
    fn replaces_history(&self, _next: &Self) -> bool {
        false
    }
    /// The animation to use before traversing actual router history.
    ///
    /// The destination is deliberately not passed here: the router owns the
    /// real history entry, and a caller-provided fallback may not match it.
    /// Implementations generated by [`route_transitions`] choose from the
    /// current route layer; manual implementations default to a safe fade.
    fn transition_back(&self) -> NavigationAnimation {
        NavigationAnimation::Fade
    }
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
/// Navigate to `route`, playing the animation `route.transition_to` selects.
/// Falls back to an immediate push when the pair resolves to
/// [`NavigationAnimation::None`] or the platform has no View Transition
/// support.
pub async fn animated_navigate<Route>(route: Route)
where
    Route: Clone + ToString + RouteTransitions + Routable + 'static,
{
    let current_route = router().current::<Route>().clone();
    if current_route == route {
        return;
    }
    let animation = current_route.transition_to(&route);
    let replace = current_route.replaces_history(&route);
    let navigator = use_navigator();
    let route = route.to_string();
    if animation == NavigationAnimation::None {
        if replace {
            _ = navigator.replace(route);
        } else {
            _ = navigator.push(route);
        }
        return;
    }
    run_animated_navigation(animation, || {
        if replace {
            _ = navigator.replace(route.clone());
        } else {
            _ = navigator.push(route.clone());
        }
    })
    .await;
}
/// Pop actual router history with the reverse animation selected by the
/// current route.
///
/// Calling [`dioxus_router::prelude::Navigator::go_back`] directly commits the
/// route before a View Transition can take its outgoing snapshot. This helper
/// starts the snapshot first, then performs the actual history traversal from
/// inside the same acknowledgement handshake as [`animated_navigate`].
///
/// Returns `false` without changing routes when there is no previous entry.
/// This makes it suitable for platform Back gestures, where the operating
/// system should retain its normal root behavior.
pub async fn try_animated_go_back<Route>() -> bool
where
    Route: Clone + ToString + RouteTransitions + Routable + 'static,
{
    let navigator = use_navigator();
    if !navigator.can_go_back() {
        return false;
    }
    let current_route = router().current::<Route>().clone();
    let animation = current_route.transition_back();
    if animation == NavigationAnimation::None {
        navigator.go_back();
        return true;
    }
    run_animated_navigation(animation, || navigator.go_back()).await;
    true
}
/// Pop actual router history with an animated reverse transition, falling back
/// to a normal animated navigation only when no previous entry exists.
pub async fn animated_go_back<Route>(fallback: Route)
where
    Route: Clone + ToString + RouteTransitions + Routable + 'static,
{
    if !try_animated_go_back::<Route>().await {
        animated_navigate(fallback).await;
    }
}
#[cfg(feature = "native-back")]
/// Window event emitted after native Back has completed an animated router pop.
///
/// Apps normally do not need this. It is available for route-adjacent work
/// such as restoring scroll after the destination has rendered.
pub const NATIVE_BACK_TRANSITION_FINISHED_EVENT: &str = "g3routebacktransitionend";
#[cfg(all(feature = "native-back", target_os = "android"))]
const NATIVE_BACK_EVENT_PLACEHOLDER: &str = "__G3_NATIVE_BACK_EVENT__";
#[cfg(all(feature = "native-back", target_os = "android"))]
const NATIVE_BACK_FINISHED_EVENT_PLACEHOLDER: &str = "__G3_NATIVE_BACK_FINISHED_EVENT__";
#[cfg(all(feature = "native-back", any(target_os = "android", test)))]
const NATIVE_BACK_NAVIGATION_BRIDGE: &str = r#"
const nativeBackEvent = "__G3_NATIVE_BACK_EVENT__";
const transitionFinishedEvent = "__G3_NATIVE_BACK_FINISHED_EVENT__";
const stateKey = Symbol.for("g3-route-transitions.native-back");

window[stateKey]?.dispose?.();

const state = { pending: false };
const defer = window.queueMicrotask?.bind(window)
    ?? ((callback) => Promise.resolve().then(callback));
const onNativeBack = (event) => {
    // Give dialogs, sheets, menus, and fullscreen players the rest of the
    // current dispatch to claim this cancelable event first.
    defer(async () => {
        if (event.defaultPrevented || state.pending) return;

        state.pending = true;
        event.preventDefault();
        dioxus.send("back");

        try {
            const outcome = await dioxus.recv();
            if (outcome === "navigated") {
                window.dispatchEvent(new Event(transitionFinishedEvent));
            }
        } finally {
            state.pending = false;
        }
    });
};

window.addEventListener(nativeBackEvent, onNativeBack);
window[stateKey] = {
    dispose() {
        window.removeEventListener(nativeBackEvent, onNativeBack);
    },
};
"#;
/// Connect Android system Back from `g3-native-plugins` to
/// [`try_animated_go_back`].
///
/// Call this hook once from a layout rendered beneath `Router<Route>`. It
/// subscribes to the current route, enables native interception only while the
/// router can go back, and uses the route's generated reverse animation. At a
/// root route the callback is disabled, so Android Back exits normally.
///
/// Enable the crate's `native-back` feature to use this hook. It reuses the
/// nearest `NativePluginsProvider` when present and otherwise owns a standalone
/// Back plugin, so no handwritten Rust/JavaScript connector is required.
///
/// Higher-priority UI may claim the cancelable `g3nativeback` window event by
/// synchronously calling `preventDefault()`. See
/// [`use_native_back_navigation_with_interception`] when such UI can be open
/// without router history.
#[cfg(feature = "native-back")]
pub fn use_native_back_navigation<Route>()
where
    Route: Clone + ToString + RouteTransitions + Routable + 'static,
{
    use_native_back_navigation_with_interception::<Route>(false);
}
/// The configurable form of [`use_native_back_navigation`].
///
/// Set `intercept_without_history` while a non-route UI layer is open at the
/// root. That layer must synchronously call `preventDefault()` on the
/// `g3nativeback` event after dismissing itself. If no layer claims the event
/// and no router history exists, the hook safely passes that press back to
/// Android.
#[cfg(feature = "native-back")]
pub fn use_native_back_navigation_with_interception<Route>(intercept_without_history: bool)
where
    Route: Clone + ToString + RouteTransitions + Routable + 'static,
{
    #[cfg(target_os = "android")]
    {
        use g3_native_plugins::{BackButton, NativePlugins};
        let _current_route: Route = use_route();
        let navigator = use_navigator();
        let standalone = use_signal(BackButton::new);
        let mut back_button = try_consume_context::<NativePlugins>()
            .map(|plugins| plugins.back_button)
            .unwrap_or(standalone);
        let intercepting = navigator.can_go_back() || intercept_without_history;
        {
            let mut back_button = back_button.write();
            let _ = back_button.prepare();
            let _ = back_button.set_intercepting(intercepting);
        }
        use_future(move || async move {
            let script = NATIVE_BACK_NAVIGATION_BRIDGE
                .replace(
                    NATIVE_BACK_EVENT_PLACEHOLDER,
                    g3_native_plugins::NATIVE_BACK_EVENT,
                )
                .replace(
                    NATIVE_BACK_FINISHED_EVENT_PLACEHOLDER,
                    NATIVE_BACK_TRANSITION_FINISHED_EVENT,
                );
            let mut bridge = eval(&script);
            loop {
                if !matches!(bridge.recv::<String>().await.as_deref(), Ok("back")) {
                    break;
                }
                let navigated = try_animated_go_back::<Route>().await;
                if !navigated {
                    let _ = back_button.write().fall_through();
                }
                let outcome = if navigated { "navigated" } else { "ignored" };
                if bridge.send(outcome).is_err() {
                    break;
                }
            }
        });
    }
    #[cfg(not(target_os = "android"))]
    let _ = intercept_without_history;
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
        assert!(
            stylesheet
                .contains(
                    "html[data-route-transition-platform=\"ios\"][data-route-transition=\"cover-up\"]::view-transition-old(base)",
                ),
        );
        assert!(
            stylesheet
                .contains(
                    "html[data-route-transition-platform=\"md\"][data-route-transition=\"cover-up\"]::view-transition-old(base)",
                ),
        );
        assert!(!stylesheet.contains("route-transition-mobile-dim-base"));
        assert!(!stylesheet.contains("route-transition-mobile-undim-base"));
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
        assert!(production_source.contains("pub const ROUTE_TRANSITION_PAGE_CLASS"));
        assert!(production_source.contains("pub fn RouteTransitionBase"));
        assert!(production_source.contains("pub fn RouteTransitionCover"));
        assert!(production_source.contains("pub fn RouteTransitionSegment"));
        assert!(production_source.contains("pub fn RouteTransitionPage"));
        assert!(production_source.contains("merge_transition_class"));
    }
    #[test]
    fn full_page_snapshots_own_nested_push_motion() {
        let stylesheet = include_str!("../assets/route_transitions.css");
        assert!(stylesheet.contains(".route-transition-page"));
        assert!(stylesheet.contains("view-transition-name: page"));
        assert!(stylesheet.contains(".route-transition-page > .route-transition-base"));
        assert!(stylesheet.contains(".route-transition-page .route-transition-segment"));
        assert!(stylesheet.contains("view-transition-old(page)"));
        assert!(stylesheet.contains("view-transition-new(page)"));
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
        assert!(VIEW_TRANSITION_NAVIGATE.contains("document.startViewTransition(async () =>"),);
        assert!(VIEW_TRANSITION_NAVIGATE.contains("dioxus.send(\"navigate\")"));
        assert!(VIEW_TRANSITION_NAVIGATE.contains("const routeCommit = await dioxus.recv()"),);
        assert!(VIEW_TRANSITION_NAVIGATE.contains("routeCommit !== \"navigated\""));
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
            .split("pub async fn try_animated_go_back")
            .nth(1)
            .expect("try_animated_go_back is public");
        assert!(helper.contains("navigator.can_go_back()"));
        assert!(helper.contains("current_route.transition_back()"));
        assert!(helper.contains("run_animated_navigation(animation"));
        assert!(helper.contains("navigator.go_back()"));
        let fallback_helper = production_source
            .split("pub async fn animated_go_back")
            .nth(1)
            .expect("animated_go_back is public");
        assert!(fallback_helper.contains("try_animated_go_back::<Route>().await"));
        assert!(fallback_helper.contains("animated_navigate(fallback).await"));
    }
    #[cfg(feature = "native-back")]
    #[test]
    fn native_back_bridge_is_cancelable_prioritized_and_acknowledged() {
        let source = include_str!("lib.rs");
        assert!(NATIVE_BACK_NAVIGATION_BRIDGE.contains("event.defaultPrevented"));
        assert!(NATIVE_BACK_NAVIGATION_BRIDGE.contains("queueMicrotask"));
        assert!(NATIVE_BACK_NAVIGATION_BRIDGE.contains("dioxus.send(\"back\")"));
        assert!(NATIVE_BACK_NAVIGATION_BRIDGE.contains("await dioxus.recv()"));
        assert!(NATIVE_BACK_NAVIGATION_BRIDGE.contains("g3-route-transitions.native-back"),);
        assert_eq!(
            NATIVE_BACK_TRANSITION_FINISHED_EVENT,
            "g3routebacktransitionend"
        );
        assert!(source.contains("back_button.write().fall_through()"));
    }
    #[test]
    fn in_place_routes_replace_history_without_crossing_the_js_boundary() {
        let source = include_str!("lib.rs");
        let production_source = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source precedes tests");
        let helper = production_source
            .split("pub async fn animated_navigate")
            .nth(1)
            .expect("animated_navigate is public");
        assert!(helper.contains("current_route.replaces_history(&route)"));
        assert!(helper.contains("navigator.replace(route)"));
        assert!(helper.contains("animation == NavigationAnimation::None"));
    }
    #[test]
    fn a_cross_dissolve_never_uncovers_what_is_behind_the_page() {
        let stylesheet = include_str!("../assets/route_transitions.css");
        assert!(
            !stylesheet.contains("route-transition-fade-in"),
            "nothing may fade in during a cross-dissolve",
        );
        assert!(
            stylesheet.contains("route-transition-fade-out"),
            "the outgoing page still fades out",
        );
    }
    #[test]
    fn route_transition_css_uses_production_durations() {
        let stylesheet = include_str!("../assets/route_transitions.css");
        assert!(stylesheet.contains("--route-transition-cover-duration: 0.6s"));
        assert!(stylesheet.contains("--route-transition-push-duration: 260ms"));
        assert!(stylesheet.contains("--route-transition-fade-duration: 150ms"));
        assert!(stylesheet.contains("--route-transition-morph-duration: 350ms"));
        assert!(stylesheet.contains("--route-transition-md-sheet-dismiss-duration: 280ms"),);
        assert!(stylesheet.contains("to { transform: translateY(100vh); }"));
        assert!(!stylesheet.contains("route-transition-duration-debug"));
        assert!(!stylesheet.contains("route-transition-mobile-dim-base"));
        assert!(!stylesheet.contains("route-transition-mobile-undim-base"));
    }
    #[test]
    fn push_transitions_diverge_between_ios_parallax_and_md_shared_axis() {
        let stylesheet = include_str!("../assets/route_transitions.css");
        assert!(stylesheet.contains("route-transition-ios-push-out-left"));
        assert!(stylesheet.contains("translateX(-30%); filter: brightness(0.85)"));
        assert!(stylesheet.contains("route-transition-md-axis-out-left"));
        assert!(stylesheet.contains("route-transition-md-axis-in-left"));
    }
    #[test]
    fn fade_transitions_are_the_same_fast_cross_dissolve_on_both_platforms() {
        let stylesheet = include_str!("../assets/route_transitions.css");
        assert!(
            stylesheet.contains("html[data-route-transition=\"fade\"]::view-transition-old(root)",),
        );
        assert!(stylesheet.contains("route-transition-fade-out"));
        assert!(!stylesheet.contains("route-transition-md-fade-through"));
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
        let start = VIEW_TRANSITION_NAVIGATE
            .find("document.startViewTransition")
            .expect("the transition is started");
        assert!(
            !VIEW_TRANSITION_NAVIGATE[..start].contains("await dioxus.recv()"),
            "no round trip may precede the snapshot",
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
            "every placeholder is substituted",
        );
        assert!(
            filled
                .contains("document.documentElement.dataset.routeTransitionPlatform = platform;",),
        );
        assert!(!filled.contains("navigator.userAgent"));
    }
}
