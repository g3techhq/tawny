# g3-route-transitions

Route-owned View Transition helpers for Dioxus Router.

This crate keeps route animation rules next to your `Routable` enum, then exposes navigation helpers and explicit snapshot marker components:

- `#[route_transitions]` derives transition metadata from route variants.
- `animated_navigate(route)` computes the animation and history action from the current route. In-place route updates replace history; ordinary navigation pushes it.
- `animated_go_back(fallback)` takes the outgoing snapshot before popping actual router history. The fallback is used only when no prior entry exists.
- `try_animated_go_back()` animates a real pop and reports whether history existed, which is useful for platform Back gestures.
- The optional `native-back` feature connects Android system Back from `g3-native-plugins` to `try_animated_go_back()`.
- `RouteTransitionProvider` imports the default View Transition stylesheet.
- `RouteTransitionRoot` wraps the app shell with the provider and cover marker.
- `RouteTransitionBase`, `RouteTransitionCover`, `RouteTransitionSegment`, and `RouteTransitionPage` mark snapshot regions explicitly.
- `Platform` (`Ios` / `Md`) plus `set_platform`/`get_platform`/`init_auto_platform` pick which native motion language a transition renders with.

## iOS vs Material motion

Every `NavigationAnimation` is a semantic event, not a specific animation - the stylesheet gives each one a different look depending on the current [`Platform`]:

| Animation | iOS (UIKit) | Material (M3) |
|---|---|---|
| `PushLeft` / `PushRight` | Navigation-controller push/pop: the outgoing page never fully leaves - it parallax-shifts ~30% off and dims, as if sliding back in the z-axis under the incoming page. | Shared axis (X): both pages slide the same distance and cross-fade symmetrically, no dimming. |
| `CoverUp` / `UncoverDown` | Page-sheet modal: the base page scales down slightly and gains rounded corners while it dims. | Modal bottom sheet: the base page dims under a scrim; no scale/corner-round. |
| `Fade` | Quick plain cross-dissolve, used for unrelated peer routes. | The same quick cross-dissolve; keeping the incoming page opaque avoids WebView backdrop flashes. |
| `MorphIn` / `MorphOut` | Generic scale+fade approximation of a card growing into its own route (no true shared-element geometry - this is a route-level helper, not a per-element one). | Same shape as iOS, with Material's emphasized easing/duration - an approximation of "container transform". |

Call `set_platform(Platform::Ios)` / `set_platform(Platform::Md)` once at startup, and again whenever your app's platform mode changes (e.g. a settings toggle), so `animated_navigate` renders the transition that matches. Without an explicit call, `get_platform()` falls back to `detect_platform()` (`cfg(target_os)` on native builds, user-agent sniffing on wasm).

## Install

```toml
[dependencies]
dioxus = { version = "0.7.9", features = ["router"] }
g3-route-transitions = "0.1"
```

For automatic Android system Back integration, enable `native-back`:

```toml
[dependencies]
g3-route-transitions = { version = "0.1", features = ["native-back"] }
```

The Rust crate name is `g3_route_transitions`:

```rust
use g3_route_transitions::{animated_navigate, route_transitions, RouteTransitionRoot};
```

## Define Route Metadata

Add `#[route_transitions]` to the same enum that derives `Routable`. Use `#[transition(...)]` on variants that need non-default behavior.

```rust,ignore
use dioxus::prelude::*;
use g3_route_transitions::route_transitions;

#[route_transitions]
#[derive(Clone, Routable, PartialEq)]
enum Route {
    #[transition(root, replace)]
    #[route("/items?:tab")]
    Items { tab: ItemsTab },

    #[transition(cover)]
    #[route("/items/new")]
    NewItem {},

    #[transition(pushed, replace(key = item_id), forward = ItemComments)]
    #[route("/items/:item_id?:tab")]
    ItemDetails { item_id: String, tab: ItemTab },

    #[transition(pushed)]
    #[route("/items/:item_id/comments")]
    ItemComments { item_id: String },
}
```

Transition rules:

- `base` marks a normal page. It is the default.
- `root` marks a stable application root such as a bottom-tab destination.
- `pushed` marks a full-screen page above a root. Root-to-pushed uses `PushLeft`; pushed-to-root uses `PushRight`.
- `cover` marks a sheet or modal route. Base-to-cover uses `CoverUp`; cover-to-base uses `UncoverDown`.
- `morph` marks a route that grows out of a card on a `base` route. Base-to-morph uses `MorphIn`; morph-to-base uses `MorphOut`.
- `push(group = name, order = field)` marks ordered peer routes.
- `key = field` or `key = (field_a, field_b)` scopes a push group to one logical entity.
- `forward = Route` or `forward = (RouteA, RouteB)` declares directed drill-down destinations.
- `replace` makes changes between values of the same variant skip animation and replace browser history. `replace(key = id)` applies only when the identity fields match.
- Routes with no more specific match fall back to `Fade`.

## Mark Snapshot Regions

`RouteTransitionRoot` is the common app-shell wrapper. It loads the transition CSS and marks the shell as the cover snapshot.

```rust,ignore
use g3_route_transitions::RouteTransitionRoot;

#[component]
fn App() -> Element {
    rsx! {
        RouteTransitionRoot {
            Router::<Route> {}
        }
    }
}
```

For more explicit layouts, use the marker components:

```rust,ignore
use g3_route_transitions::{RouteTransitionBase, RouteTransitionSegment};

rsx! {
    RouteTransitionBase { nav { "Tabs or base page" } }
    RouteTransitionSegment { main { Outlet::<Route> {} } }
}
```

When a page shell already contains nested base/segment markers, wrap each
routed page in `RouteTransitionPage`. It captures the header, body, and tab bar
as one viewport-sized image, which prevents independent snapshots from
overlapping or exposing an embedded WebView's background:

```rust,ignore
use g3_route_transitions::RouteTransitionPage;

rsx! {
    RouteTransitionPage {
        Header { title: "Items" }
        Body { Outlet::<Route> {} }
    }
}
```

The marker class constants are also public for libraries that need to place the marker on an existing element without adding a wrapper:

- `ROUTE_TRANSITION_BASE_CLASS`
- `ROUTE_TRANSITION_COVER_CLASS`
- `ROUTE_TRANSITION_SEGMENT_CLASS`
- `ROUTE_TRANSITION_PAGE_CLASS`

## Navigate

```rust,ignore
button {
    onclick: move |_| spawn(async move {
        animated_navigate(Route::NewItem {}).await;
    }),
    "New item"
}
```

Use the matching history helper for Back affordances. A bare
`navigator.go_back()` changes the route before the old page can be captured.

```rust,ignore
use g3_route_transitions::animated_go_back;

button {
    onclick: move |_| spawn(async move {
        animated_go_back(Route::Items { tab: Tab::All }).await;
    }),
    "Back"
}
```

The route mutation still happens inside the Rust/JavaScript acknowledgement
handshake: JavaScript captures the old WebView frame, asks Rust to push,
replace, or pop the Dioxus route, waits for the route commit, and then releases
the transition. If `document.startViewTransition` is unavailable or the user
prefers reduced motion, navigation falls back to the same router action without
animation.

## Android System Back

With the `native-back` feature enabled, call one hook in a layout beneath the
Dioxus router:

```rust,ignore
use g3_route_transitions::use_native_back_navigation;

#[component]
fn AppLayout() -> Element {
    use_native_back_navigation::<Route>();

    rsx! { Outlet::<Route> {} }
}
```

That is the complete connector. The hook prepares `g3-native-plugins`, tracks
real Dioxus router history, enables Android interception only when a pop is
possible, and calls the same animated operation used by visible Back buttons.
It reuses `NativePluginsProvider` when the app already has one and otherwise
owns the Back plugin itself.

Dialogs, sheets, fullscreen players, and other higher-priority UI can claim the
cancelable `g3nativeback` window event with `event.preventDefault()`. If such UI
can be open at the root of router history, use
`use_native_back_navigation_with_interception::<Route>(is_open)` so the native
callback remains enabled until that UI closes. After an actual route pop, the
library emits `g3routebacktransitionend` for optional work such as restoring
scroll position. If no UI claims an intercepted event and no route can be
popped, the hook passes that press back to Android instead of trapping it.
