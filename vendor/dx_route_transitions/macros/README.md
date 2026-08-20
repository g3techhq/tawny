# dx-route-transitions-macros

Proc macro crate for [`dx-route-transitions`](../README.md).

Most users should depend on `dx-route-transitions` instead of this crate directly. The runtime crate re-exports the macro:

```rust,ignore
use dx_route_transitions::route_transitions;
```

The macro implements `dx_route_transitions::RouteTransitions` for a Dioxus `Routable` enum and consumes per-variant `#[transition(...)]` attributes.