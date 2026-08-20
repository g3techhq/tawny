// Browser history traversal animation.
//
// App-initiated Back uses `animated_go_back`, which can capture the outgoing
// page before asking Dioxus to pop its history. Browser toolbar/swipe Back does
// not pass through Rust, so Chromium's Navigation API is used to start that
// same snapshot during the cancellable `navigate` event, before the traversal
// commits. `popstate` remains a best-effort fallback for older engines.
(() => {
  "use strict";

  const isSheet = (route) => {
    try {
      return new URL(route, window.location.href).pathname.startsWith("/watch/");
    } catch (_) {
      return String(route).startsWith("/watch/");
    }
  };

  // Mirrors Tawny's explicit set_platform(Platform::Ios): peer routes use a
  // plain cross-dissolve instead of Material's sequential fade-through.
  const PLATFORM = "ios";

  const routeKey = (value) => {
    try {
      const url = new URL(value, window.location.href);
      return `${url.pathname}${url.search}${url.hash}`;
    } catch (_) {
      return String(value);
    }
  };

  const animationFor = (from, to) => {
    const leaving = isSheet(from);
    const entering = isSheet(to);
    if (!leaving && entering) return "cover-up";
    if (leaving && !entering) return "uncover-down";
    return "fade";
  };

  const routeRendered = () =>
    new Promise((resolve) => {
      let settled = false;
      const finish = () => {
        if (settled) return;
        settled = true;
        window.clearTimeout(ceiling);
        observer?.disconnect?.();
        resolve();
      };
      const observer = typeof MutationObserver === "undefined"
        ? null
        : new MutationObserver(finish);
      observer?.observe?.(document.body ?? document.documentElement, {
        childList: true,
        subtree: true,
      });
      const ceiling = window.setTimeout(finish, 120);
      if (!observer) finish();
    });

  let lastRoute = routeKey(window.location.href);

  // Keep the source side current for pushes, which do not emit popstate.
  for (const method of ["pushState", "replaceState"]) {
    const original = history[method];
    history[method] = function patched(...args) {
      const result = original.apply(this, args);
      lastRoute = routeKey(window.location.href);
      return result;
    };
  }

  const canAnimate = () =>
    Boolean(document.startViewTransition) &&
    !window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches &&
    !document.documentElement.dataset.routeTransition;

  const startTraversalTransition = (from, to) => {
    if (from === to || !canAnimate()) return null;
    const root = document.documentElement;
    root.dataset.routeTransition = animationFor(from, to);
    root.dataset.routeTransitionPlatform = PLATFORM;
    // The class granting the outgoing snapshot its view-transition-name must
    // be computed before the old state is captured.
    void root.offsetHeight;

    const clear = () => {
      delete root.dataset.routeTransition;
      delete root.dataset.routeTransitionPlatform;
    };
    try {
      const transition = document.startViewTransition(() => routeRendered());
      transition.finished.then(clear, clear);
      return transition;
    } catch (_) {
      clear();
      return null;
    }
  };

  const navigationApi = window.navigation;
  if (navigationApi?.addEventListener) {
    navigationApi.addEventListener("navigate", (event) => {
      if (event.navigationType !== "traverse" || !event.canIntercept) return;
      const to = routeKey(event.destination?.url ?? window.location.href);
      const from = lastRoute;
      lastRoute = to;
      const transition = startTraversalTransition(from, to);
      if (!transition) return;

      // `intercept` commits the history traversal normally (and therefore lets
      // Dioxus receive popstate), while its handler keeps navigation pending
      // until the transition update has observed the new route.
      try {
        event.intercept({
          handler: async () => {
            try {
              await transition.updateCallbackDone;
            } catch (_) {}
          },
        });
      } catch (_) {}
    });
    return;
  }

  window.addEventListener("popstate", () => {
    const from = lastRoute;
    const to = routeKey(window.location.href);
    lastRoute = to;
    startTraversalTransition(from, to);
  });
})();
