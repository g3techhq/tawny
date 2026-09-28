# Dioxus

The app is written against **Dioxus 0.7.9** (`dioxus`, `dioxus-fullstack` and
`manganis` pinned with `=` to match the `dx` CLI).

| File | What it is |
| --- | --- |
| [patterns.md](patterns.md) | **Start here.** How the g3 apps use Dioxus: props, hooks, stores, cached data, hydration, and the tooling pitfalls we have hit. Ours; kept current. |
| [framework-agents.md](framework-agents.md) | Dioxus's own agent guide, from the framework repository. |
| [architecture/](architecture/00-OVERVIEW.md) | The framework's internal architecture notes: core, CLI, RSX, signals and stores, fullstack, renderers, hot reload, assets, router, WASM splitting, native plugins, manifests. |

`framework-agents.md` and `architecture/` are copied unchanged from
[DioxusLabs/dioxus](https://github.com/DioxusLabs/dioxus) at tag `v0.7.10`,
the newest 0.7 patch release, one ahead of the pinned 0.7.9.
They describe the framework, not this app, and each file says so in its first
line. Refresh them from upstream when Dioxus is upgraded; do not edit them
here.

Where they and `patterns.md` disagree about how to write code *in this app*,
`patterns.md` wins.
