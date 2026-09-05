# g3-route-transitions-macros

Proc macro crate for [`g3-route-transitions`](../README.md).

Most users should depend on `g3-route-transitions` instead of this crate directly. The runtime crate re-exports the macro:

```rust,ignore
use g3_route_transitions::route_transitions;
```

The macro implements `g3_route_transitions::RouteTransitions` for a Dioxus
`Routable` enum and consumes per-variant `#[transition(...)]` attributes. It
supports route layers (`base`, `root`, `pushed`, `cover`, `morph`), directed
`forward` edges, same-variant `replace` rules, and ordered `push` groups.
