# Changelog

All notable changes to `g3-route-transitions` are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- Added the optional `native-back` integration with `g3-native-plugins`,
  including hooks that connect Android system Back directly to animated Dioxus
  router history without app-specific Rust/JavaScript bridges.
- Added `try_animated_go_back`, which preserves the operating system's root
  behavior when no router history exists.
- Back animation selection now depends only on the current route layer rather
  than a fallback route that may not be the actual history destination.
- Added `root` and `pushed` route layers plus directed `forward` edges for
  navigation-stack transitions.
- Added `replace` route metadata so same-page query/filter updates replace
  browser history without animating.
- Added layer-aware back animations while continuing to pop the browser's
  actual previous entry through the Rust/JavaScript acknowledgement bridge.
- Added `RouteTransitionPage` for one stable full-viewport snapshot around
  shells with nested transition markers.
- Made iOS and Material peer fades the same faster cross-dissolve, and made
  Material sheet dismissal faster with full downward travel.

## [0.1.0] - 2026-08-20

Initial release.

- Route-owned View Transition helpers for Dioxus Router: `animated_navigate`,
  the `RouteTransitions` trait, and the `route_transitions` macro.
- Eight navigation animations (fade, push, cover/uncover, morph) that each
  render with iOS or Material motion depending on the active `Platform`.
- `RouteTransitionProvider` and `RouteTransitionRoot` for wiring the
  stylesheet and layer classes.
- Honors `prefers-reduced-motion`, falling back to an immediate navigation.
