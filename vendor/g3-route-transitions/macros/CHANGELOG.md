# Changelog

All notable changes to `g3-route-transitions-macros` are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- Added `root` and `pushed` route layers.
- Added directed `forward` route relationships.
- Added unkeyed and identity-keyed `replace` rules for in-place routes.
- Generated layer-aware back-transition behavior.

## [0.1.0] - 2026-08-20

Initial release.

- `route_transitions` attribute macro, generating a `RouteTransitions`
  implementation from per-route animation declarations.

This crate is an implementation detail of `g3-route-transitions` and carries
no stability guarantee of its own.
