# Docker dependency snapshots

The production image must build from a standalone Tawny checkout, while local
development intentionally patches these crates from sibling repositories in
`.cargo/config.toml`. The Dockerfile replaces that configuration with
`.cargo/config.docker.toml` and builds against the committed snapshots here.

Refresh the matching snapshot whenever Tawny adopts a new change from `g3_ui`,
`dx_route_transitions`, or `dx_native_plugins`, and include that refresh in the
same Tawny commit.

Current source revisions:

- `g3_ui`: `5c29f3605d3ee41ac8a73019e4e2d112824dc7ab`
- `dx_route_transitions`: `60d4d8f4efdce16292f3cffac0af3bb570742691`
- `dx_native_plugins`: `4cb16d6ea5a0c3b155b276b6c54dfb941bfa41e3`
