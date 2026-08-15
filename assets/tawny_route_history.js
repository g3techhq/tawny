// Browser back/forward animation.
//
// `animated_navigate` only wraps navigations the app initiates, so the toolbar
// buttons and swipe-back went straight to a bare route swap with no view
// transition. Popstate has no "intended destination" to hand the Rust side, so
// the animation is derived here from the paths involved.
//
// This registers before the router mounts, which matters: popstate listeners
// run in registration order, and the snapshot has to be taken while the old
// route is still on screen.
(() => {
  // Mirrors the route roles in `app.rs`: the watch page is the only
  // `#[transition(cover)]`, everything else is a peer that cross-fades.
  const isSheet = (path) => path.startsWith("/watch/");

  // Mirrors `set_platform` in `app.rs`.
  const PLATFORM = "ios";

  const animationFor = (from, to) => {
    const leaving = isSheet(from);
    const entering = isSheet(to);
    if (!leaving && entering) return "cover-up";
    if (leaving && !entering) return "uncover-down";
    return "fade";
  };

  const nextFrame = () =>
    new Promise((resolve) => {
      const raf = window.requestAnimationFrame ?? ((cb) => window.setTimeout(cb, 16));
      raf(() => resolve());
    });

  // The router re-renders asynchronously after popstate, so the transition
  // callback has to hold the snapshot open until the new route is in the DOM.
  const routeRendered = () =>
    new Promise((resolve) => {
      let settled = false;
      const finish = () => {
        if (settled) return;
        settled = true;
        observer.disconnect();
        resolve();
      };
      const observer = new MutationObserver(finish);
      observer.observe(document.body ?? document.documentElement, {
        childList: true,
        subtree: true,
        attributes: true,
      });
      window.setTimeout(finish, 120);
      nextFrame().then(nextFrame).then(finish);
    });

  let lastPath = window.location.pathname;

  // Pushes bypass popstate, but they still move the path this handler
  // compares against on the next back.
  for (const method of ["pushState", "replaceState"]) {
    const original = history[method];
    history[method] = function patched(...args) {
      const result = original.apply(this, args);
      lastPath = window.location.pathname;
      return result;
    };
  }

  window.addEventListener("popstate", () => {
    const from = lastPath;
    const to = window.location.pathname;
    lastPath = to;

    if (from === to) return;
    if (!document.startViewTransition) return;
    if (window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches) return;
    // A transition already running owns the attributes; starting a second one
    // would make the browser skip both.
    if (document.documentElement.dataset.routeTransition) return;

    const root = document.documentElement;
    root.dataset.routeTransition = animationFor(from, to);
    root.dataset.routeTransitionPlatform = PLATFORM;

    const clear = () => {
      delete root.dataset.routeTransition;
      delete root.dataset.routeTransitionPlatform;
    };

    try {
      const transition = document.startViewTransition(() => routeRendered());
      transition.finished.then(clear, clear);
    } catch (_) {
      clear();
    }
  });
})();
