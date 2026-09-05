# Docker dependency snapshots

The production image must build from a standalone Tawny checkout, while local
development intentionally patches these crates from sibling repositories in
`.cargo/config.toml`. The Dockerfile replaces that configuration with
`.cargo/config.docker.toml` and builds against the committed snapshots here.

Refresh the matching snapshot whenever Tawny adopts a new change from `g3-ui`,
`g3-route-transitions`, or `g3-native-plugins`, and include that refresh in the
same Tawny commit.

Current source revisions:

- `g3-ui`: `d5a250ce991f0595f490cb17574fd77f9319f8fe`
- `g3-route-transitions`: `d79cf692f4d587e5af55d3af46e8558647127cf6`
- `g3-native-plugins`: `8b74fc6fcd28c778633752bf2bfc15b1f32231b8`
