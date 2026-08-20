# g3-ui

Mobile-first [Dioxus](https://dioxuslabs.com/) components inspired by [Ionic](https://ionicframework.com/).

`g3-ui` is a flat, `G3`-prefixed component library for building responsive apps (bottom-tab
navigation that becomes a desktop rail, sheets, cards, swipeable lists) that compile to web (WASM), desktop (Wry), and native
mobile targets through Dioxus. Every interactive component ships with iOS/Material Design (MD)
adaptive styling, CSS-variable theming, and WAI-ARIA attributes out of the box. The Rust crate name
is `g3_ui`.

## Install

```toml
[dependencies]
dioxus = { version = "0.7.9", features = ["router"] }
g3-ui = "0.1"
```

Enable the optional route-transition integration when your app also uses `dx-route-transitions`:

```toml
[dependencies]
g3-ui = { version = "0.1", features = ["transitions"] }
dx-route-transitions = "0.1"
```

The local checkout includes `.cargo/config.toml` to patch `dx-route-transitions` to the sibling
`../dx_route_transitions` repo while developing both crates together.

## Quick Start

`G3AppWrapper` loads the component stylesheet, resolves the platform mode (iOS vs. MD), and applies
theme tokens to the root shell. Everything else nests inside it.

```rust,ignore
use dioxus::prelude::*;
use g3_ui::{G3AppWrapper, G3Body, G3Button, G3Card, G3Header, Theme};

#[component]
fn App() -> Element {
    rsx! {
        G3AppWrapper { theme: Theme::default_light(),
            G3Header { title: "Games" }
            G3Body {
                G3Card { title: "Pending game",
                    "Invite players and choose a format."
                }
                G3Button { onclick: |_| {}, "Create game" }
            }
        }
    }
}
```

For broad imports, use the prelude:

```rust,ignore
use g3_ui::prelude::*;
```

## Component Names

Every component exports two names: a concise name (`Button`, `Card`, `Sheet`) and a `G3`-prefixed
alias (`G3Button`, `G3Card`, `G3Sheet`). **Prefer the `G3`-prefixed names in downstream apps** —
they avoid collisions with local components and match what the rest of this document uses.

## Components

| Category | Components |
| --- | --- |
| App shell & layout | `G3AppWrapper`, `G3Header`, `G3Body`, `G3Navbar`, `G3NavbarTab`, `G3NavbarTabBar` |
| Actions | `G3Button`, `G3Fab`, `G3FabButton`, `G3FabList`, `G3FabContainer`, `G3InfoButton`, `G3SheetButton` |
| Form inputs | `G3Field`, `G3Select`, `G3Checkbox`, `G3Radio`, `G3RadioGroup`, `G3Toggle` |
| Content & data display | `G3Card`, `G3List`, `G3Item`, `G3ItemDivider`, `G3SwipeItem`, `G3SwipeAction`, `G3Line`, `G3Avatar`, `G3Badge`, `G3Chip`, `G3Progress`, `G3Skeleton` |
| Disclosure & grouping | `G3AccordionGroup`, `G3AccordionItem`, `G3SegmentGroup`, `G3SegmentButton` |
| Overlays | `G3Sheet`, `G3Modal`, `G3ConfirmModal`, `G3Toast` |
| Feedback | `G3Spinner`, `G3Refresher` |

See [docs.rs/g3-ui](https://docs.rs/g3-ui) for the full prop reference, or run the [playground](#playground)
to browse every component live.

## Controlling Component State

Stateful components own a `Signal<T>` you pass in — the component reads and writes it directly, so
there's no separate `value`/`onchange` round trip to wire up yourself:

```rust,ignore
let mut checked = use_signal(|| false);
let mut name = use_signal(String::new);

rsx! {
    G3Toggle { checked }
    G3Field { label: "Name", value: name }
}
```

Every stateful component (`G3Checkbox`, `G3Toggle`, `G3Field`, `G3Select`, `G3Sheet`, `G3Modal`,
`G3ConfirmModal`, `G3Toast`, `G3AccordionGroup`, `G3SegmentGroup`, `G3RadioGroup`) follows this
pattern. An optional `onchange` callback is available on most of them if you need a side effect
(analytics, validation, syncing to a store) beyond the signal update the component already makes
for you.

## Modes and Themes

Components support Ionic-style `ios` and `md` modes. The mode is resolved once when a component
initializes (prop → `G3ThemeProvider`/`G3AppWrapper` context → global default) and does not change
reactively afterwards — this mirrors Ionic's own `mode` semantics.

```rust,ignore
use g3_ui::{ComponentMode, G3AppWrapper};

rsx! {
    G3AppWrapper { mode: ComponentMode::Ios, "..." }
}
```

Theme colors are CSS custom properties generated from a `Theme` value. `Theme::default_light()` and
`Theme::default_dark()` are the two built-in presets; every field is public, so build a custom theme
with struct-update syntax or the `with_focused` helper:

```rust,ignore
let theme = Theme::default_light().with_focused("#22c55e");

let custom = Theme {
    bg: "#0b1020".into(),
    card: "#151b2e".into(),
    ..Theme::default_dark()
};
```

Pass a `Theme` to `G3AppWrapper` (or `G3ThemeProvider` if you're not using the app shell) to apply
it to everything nested inside. The theme is written as CSS custom properties on the shell element,
so switching it at runtime is reactive — drive the `theme` prop from a signal and the whole tree
re-themes without a remount:

```rust,ignore
let dark = use_signal(|| false);
let theme = if dark() { Theme::default_dark() } else { Theme::default_light() };

rsx! {
    G3AppWrapper { theme,
        G3Toggle { checked: dark }   // flip to re-theme live
        // ...
    }
}
```

## Route Transition Integration

With the `transitions` feature enabled, `G3AppWrapper` loads the `dx-route-transitions` stylesheet
provider and marks the shell with the cover snapshot class. `G3Navbar` marks persistent tab/navigation
layouts with the base snapshot class. `G3Body` marks its scrollable content with the segment snapshot
class for push transitions.

Without the feature, `g3-ui` does not depend on `dx-route-transitions` and does not emit
route-transition marker classes.

## Responsive App Shell

`G3AppWrapper` measures its own available width with a CSS container query. At `48rem` and wider,
the same `G3Navbar` tree automatically moves `G3NavbarTabBar` from the bottom edge to a compact left
navigation rail. `G3Header` keeps its start, title, end, and optional segmented-toolbar slots across
both layouts, while desktop spacing and scrollbars are applied automatically. No resize listener or
duplicate desktop navigation state is required.

Because the query follows the shell rather than the browser viewport, a narrow embedded app remains
in its mobile layout even when it is displayed inside a wide desktop page. Override
`--g3-navbar-rail-width` on `G3Navbar` if an application needs a wider desktop rail.

## Playground

An interactive component gallery lives in `playground/` inside this repository. It renders every
registered component with live controls so you can preview iOS/MD styling side by side.

```powershell
cd playground
dx serve
```

To just type-check the playground without launching a dev server:

```powershell
cargo check --manifest-path playground/Cargo.toml
```

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
