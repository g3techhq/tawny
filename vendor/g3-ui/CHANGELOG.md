# Changelog

All notable changes to `g3-ui` are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- Fix the responsive `Navbar` rail layout when the `transitions` feature is enabled.
- Add `NavbarTabDesktopPlacement` for grouping profile/settings tabs at the bottom of the desktop rail.
- Place header toolbars inline with the title and actions on wide app shells.
- Keep Material header tab indicators flush with the header's bottom edge on wide shells, matching compact ones. The wide-shell bottom inset is iOS-only, whose pill sits deliberately clear of the edge.
- Keep compact desktop headers on the two-row layout so titles remain readable and iOS segments retain bottom spacing.
- Add Mobile and Desktop viewport modes with compact- and wide-shell previews to every playground demo, selected from the header beside the design-language toggle.
- Add typed overlay, push, reveal, and persistent menu side-sheet behaviors for both left and right placements.
- Make sheet backdrops fill and dim the complete app shell, dismiss immediately from mouse/touch input, and keep contained playground demos from locking the outer page scroll.
- Make the site-wide playground drawer a persistent left menu on wide shells, falling back to a dismissible overlay once the playground page itself is phone width, keep that rail open when a demo is picked while still dismissing the phone overlay, and prevent placement classes from constraining backdrop coverage.
- Keep push sheets dimmed and dismissible at every width, add flat menus that resize rather than clip the page, and cast reveal elevation from the moving page onto the sheet.
- Keep pages scrollable beside menus, use square menu edges, and align push and overlay scrim opacity.
- Center desktop demo/source surfaces and flatten the playground component drawer styling.
- Simplify shell labels and use a persistent menu-style playground drawer.
- Give wide app shells compact floating bottom sheets, aligned select menus, centred toasts, and roomier modal dialogs.
- Update the full app demo to show secondary navigation at the bottom of the desktop rail.
- Separate menu side sheets from the page they sit beside with a hairline edge and a soft shadow, so a flush header no longer reads as one surface.
- Put each playground demo's controls above its preview so the case is set up before it is read, and keep a wide control group inside the card instead of spilling out of it.
- Make mode and theme switchable at runtime: `G3Mode` now carries a `Signal<ComponentMode>` and the ambient theme is published as a `Signal<Theme>`, so changing either re-renders components that are already mounted. Previously both were read once per component and never again, and `AppWrapper` read its own published value back instead of an outer one. **Breaking:** `G3Mode.mode` changed type; construct it from a signal.
- Add `use_ambient_theme` for reading the theme an enclosing `AppWrapper` or `G3ThemeProvider` set.
- Replace the JavaScript first-paint guard with `AssetOptions::css().with_static_head(true)`, which puts the stylesheet `<link>` in the document head at build time and lets the browser block first paint on it. The old guard hid the shell behind `visibility: hidden` and `transition: none !important` until a polled round trip reported the stylesheet had applied; when that round trip never completed the guard never lifted, which left every transition in the app dead - swipe rows snapped back instead of animating. `AppWrapper` still links the stylesheet at runtime as well, because desktop and mobile bundles only collect assets something links at runtime - a statically-headed asset alone never reaches them. **Breaking:** `G3PreloadStyle` and the `g3-preload` class are gone.
- Stop the swipe action colour bleeding through as a hairline along the bottom of the last swipeable row: the dragged content composites separately from the actions beneath it, and on a fractionally-tall row the two rasterize to different device pixels. Rows above the last were only ever covered by their own divider.
- Hold a swiped row's action colour underneath until the row has slid back over it. The row reported itself closed the instant the drag was released, which hid the actions immediately and left the row animating home across bare card.
- Link the playground stylesheet into the head at build time. Loading it at runtime left a window where the library stylesheet had applied but the playground's had not, so the device frame rendered unstyled while the app booted. The page background during that window is white, so it reads as the browser's own blank page rather than a colour of its own.
- Drop the compact-shell/wide-shell chips above each playground preview; the header toggle already names the width.

## [0.1.0] - 2026-08-20

Initial release.

- 25 mobile-first components with iOS and Material Design variants of each.
- Configurable `Theme` of 17 CSS custom-property tokens, with built-in light
  and dark themes.
- Optional `transitions` feature integrating `g3-route-transitions`.
- Safe-area insets and `prefers-reduced-motion` handled in the stylesheet.
