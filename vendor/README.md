# Docker dependency snapshots

The production image must build from a standalone Tawny checkout, while local
development intentionally patches these crates from sibling repositories in
`.cargo/config.toml`. The Dockerfile replaces that configuration with
`.cargo/config.docker.toml` and builds against the committed snapshots here.

Refresh the matching snapshot whenever Tawny adopts a new change from `g3_ui`,
`dx_route_transitions`, or `dx_native_plugins`, and include that refresh in the
same Tawny commit.

Current source revisions:

- `g3_ui`: `64da199659cefe89663288c7157f34262bb62a7c`
- `dx_route_transitions`: `2c1550465c5558de192e046f49c63b8b01456f5e`
- `dx_native_plugins`: `9d71f4adf5784fc55aeef64cd188d69491d2923d`
