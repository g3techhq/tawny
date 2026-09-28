# Styling

g3-ui draws most of Tawny: the iOS and Material looks, dark mode, the rail,
safe areas, focus rings and ARIA. What it does not draw is Tawny's own: video
cards with swipe actions, the persistent player and its stage, the mini-player
above the navigation, and the playlist action bar. Those live in
`tailwind.css`.

## Where a style goes

In order of preference:

1. **A g3-ui component or prop** ([g3-ui.md](g3-ui.md)).
2. **An app component** that already owns the look: `VideoCard`/`VideoGrid`,
   the player, `PageHeader`.
3. **Tailwind utilities for layout** in `rsx!`.
4. **A rule in `tailwind.css`**, only with a reason it cannot be one of the
   above, and a comment giving it.

## Rules

- **No colour literals** outside `tawny_theme` in `src/app.rs`. Use the
  theme's tokens (`var(--g3-color-accent)` ..).
- **No viewport-width media queries.** Use
  `@container g3-app-shell (width >= 48rem)`, so a rule changes at the same
  width as the shell. Input-capability queries (`@media (hover: hover)`) are
  fine.
- **Do not add rules that restyle g3-ui's own classes** (`.g3-btn`,
  `.g3-card-title` ..). Some exist today, in the video card and the playlist
  action bar; they are debt, and a change that touches them should move the
  need into a g3-ui prop or say why it cannot.

## Theming

`tawny_theme(appearance)` builds the g3-ui `Theme`:

- **Dark**: near-black blue grounds with an amber accent (`#f5a524`) and dark
  text on it, every elevation a visible step.
- **Light**: "Tawny in daylight": warm parchment and sage layers with a deeper
  amber (`#a9530b`) that carries white text, rather than a white system theme
  with an orange accent.

The appearance and the component mode (iOS or Material) are per-device
settings. The theme is written as custom properties on the shell, so a change
re-themes the running app in place.

## Tailwind

Tailwind v4 runs inside `dx`: `Dioxus.toml` points it at `tailwind.css` and
writes `assets/tailwind.css`. `@source` scans `src/`. The generated file is
committed; a `dx serve` that regenerates it with fewer classes is not a change
to commit.
