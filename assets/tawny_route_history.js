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

  const startTransition = (from, to) => {
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
      return transition.finished;
    } catch (_) {
      clear();
      return Promise.resolve();
    }
  };

  const shouldAnimate = (from, to) =>
    from !== to &&
    Boolean(document.startViewTransition) &&
    !window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches &&
    // A transition already running owns the attributes; starting a second one
    // would make the browser skip both.
    !document.documentElement.dataset.routeTransition;

  // The Navigation API is the only hook that runs *before* a back or forward
  // commits. popstate fires afterwards, so the snapshot was always taken of
  // the page the browser had already swapped in - which is why the animation
  // appeared to replay on top of the destination. Intercepting lets the
  // snapshot be taken while the old route is still on screen, and holds the
  // navigation open until the animation finishes.
  if (window.navigation && typeof window.navigation.addEventListener === "function") {
    window.navigation.addEventListener("navigate", (event) => {
      if (!event.canIntercept || event.hashChange || event.downloadRequest !== null) return;
      // Pushes come from the app and are already animated by the Rust side.
      if (event.navigationType !== "traverse") return;
      const from = window.location.pathname;
      const to = new URL(event.destination.url).pathname;
      if (!shouldAnimate(from, to)) return;
      lastPath = to;
      event.intercept({
        scroll: "manual",
        handler: () => startTransition(from, to),
      });
    });
    return;
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
