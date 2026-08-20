# dx-route-transitions

Route-owned View Transition helpers for Dioxus Router.

This crate keeps route animation rules next to your `Routable` enum, then exposes navigation helpers and explicit snapshot marker components:

- `#[route_transitions]` derives transition metadata from route variants.
- `animated_navigate(route)` computes the animation from the current route and pushes the next route.
- `animated_go_back(fallback)` takes the outgoing snapshot before popping router history, using the fallback route both to select the reverse animation and when no prior entry exists.
- `RouteTransitionProvider` imports the default View Transition stylesheet.
- `RouteTransitionRoot` wraps the app shell with the provider and cover marker.
- `RouteTransitionBase`, `RouteTransitionCover`, and `RouteTransitionSegment` mark named snapshot regions explicitly.
- `Platform` (`Ios` / `Md`) plus `set_platform`/`get_platform`/`init_auto_platform` pick which native motion language a transition renders with.

## iOS vs Material motion

Every `NavigationAnimation` is a semantic event, not a specific animation - the stylesheet gives each one a different look depending on the current [`Platform`]:

| Animation | iOS (UIKit) | Material (M3) |
|---|---|---|
| `PushLeft` / `PushRight` | Navigation-controller push/pop: the outgoing page never fully leaves - it parallax-shifts ~30% off and dims, as if sliding back in the z-axis under the incoming page. | Shared axis (X): both pages slide the same distance and cross-fade symmetrically, no dimming. |
| `CoverUp` / `UncoverDown` | Page-sheet modal: the base page scales down slightly and gains rounded corners while it dims. | Modal bottom sheet: the base page dims under a scrim; no scale/corner-round. |
| `Fade` | Quick plain cross-dissolve, used for unrelated peer routes. | Fade through: the outgoing page fades/shrinks out, then the incoming page fades/grows in - a sequential, not simultaneous, hand-off. |
| `MorphIn` / `MorphOut` | Generic scale+fade approximation of a card growing into its own route (no true shared-element geometry - this is a route-level helper, not a per-element one). | Same shape as iOS, with Material's emphasized easing/duration - an approximation of "container transform". |

Call `set_platform(Platform::Ios)` / `set_platform(Platform::Md)` once at startup, and again whenever your app's platform mode changes (e.g. a settings toggle), so `animated_navigate` renders the transition that matches. Without an explicit call, `get_platform()` falls back to `detect_platform()` (`cfg(target_os)` on native builds, user-agent sniffing on wasm).

## Install

```toml
[dependencies]
dioxus = { version = "0.7.9", features = ["router"] }
dx-route-transitions = "0.1"
```

The Rust crate name is `dx_route_transitions`:

```rust
use dx_route_transitions::{animated_navigate, route_transitions, RouteTransitionRoot};
```

## Define Route Metadata

Add `#[route_transitions]` to the same enum that derives `Routable`. Use `#[transition(...)]` on variants that need non-default behavior.

```rust,ignore
use dioxus::prelude::*;
use dx_route_transitions::route_transitions;

#[route_transitions]
#[derive(Clone, Routable, PartialEq)]
enum Route {
    #[transition(base, push(group = tabs, order = tab))]
    #[route("/items?:tab")]
    Items { tab: ItemsTab },

    #[transition(cover)]
    #[route("/items/new")]
    NewItem {},

    #[transition(cover, push(group = item_details, key = item_id, order = tab))]
    #[route("/items/:item_id?:tab")]
    ItemDetails { item_id: String, tab: ItemTab },
}
```

Transition rules:

- `base` marks a normal page. It is the default.
- `cover` marks a sheet or modal route. Base-to-cover uses `CoverUp`; cover-to-base uses `UncoverDown`.
- `morph` marks a route that grows out of a card on a `base` route. Base-to-morph uses `MorphIn`; morph-to-base uses `MorphOut`.
- `push(group = name, order = field)` marks ordered peer routes.
- `key = field` or `key = (field_a, field_b)` scopes a push group to one logical entity.
- Routes with no more specific match fall back to `Fade`.

## Mark Snapshot Regions

`RouteTransitionRoot` is the common app-shell wrapper. It loads the transition CSS and marks the shell as the cover snapshot.

```rust,ignore
use dx_route_transitions::RouteTransitionRoot;

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
use dx_route_transitions::{RouteTransitionBase, RouteTransitionSegment};

rsx! {
    RouteTransitionBase { nav { "Tabs or base page" } }
    RouteTransitionSegment { main { Outlet::<Route> {} } }
}
```

The marker class constants are also public for libraries that need to place the marker on an existing element without adding a wrapper:

- `ROUTE_TRANSITION_BASE_CLASS`
- `ROUTE_TRANSITION_COVER_CLASS`
- `ROUTE_TRANSITION_SEGMENT_CLASS`

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
use dx_route_transitions::animated_go_back;

button {
    onclick: move |_| spawn(async move {
        animated_go_back(Route::Items { tab: Tab::All }).await;
    }),
    "Back"
}
```

If the browser does not support `document.startViewTransition` or the user prefers reduced motion, navigation falls back to a normal router push.
