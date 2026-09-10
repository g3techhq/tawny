// Browser history traversal animation.
//
// App-initiated Back uses `animated_go_back`, which can capture the outgoing
// page before asking Dioxus to pop its history. Browser toolbar/swipe Back does
// not pass through Rust, so Chromium's Navigation API is used to start that
// same snapshot during the cancellable `navigate` event, before the traversal
// commits. `popstate` remains a best-effort fallback for older engines.
(() => {
  "use strict";

  const routePath = (route) => {
    try {
      return new URL(route, window.location.href).pathname;
    } catch (_) {
      return String(route).split(/[?#]/, 1)[0];
    }
  };

  const routeLayer = (route) => {
    const rawPath = routePath(route);
    const path = rawPath.replace(/\/+$/, "") || "/";
    if (path.startsWith("/watch/")) return "cover";
    if (
      path === "/queue" ||
      path === "/history" ||
      path === "/settings" ||
      path.startsWith("/channel/") ||
      (path.startsWith("/playlists/") && path !== "/playlists/")
    ) {
      return "pushed";
    }
    if (path === "/" || path === "/subscriptions" || path === "/playlists" || path === "/explore") {
      return "root";
    }
    return "base";
  };

  const transitionPlatform = () => {
    const mode = document.querySelector?.("[data-g3-mode]")?.dataset?.g3Mode;
    if (mode === "ios" || mode === "md") return mode;

    const userAgent = window.navigator?.userAgent?.toLowerCase?.() || "";
    const platform = window.navigator?.platform?.toLowerCase?.() || "";
    const maxTouchPoints = window.navigator?.maxTouchPoints || 0;
    const ipad =
      userAgent.includes("ipad") ||
      (platform.includes("mac") && maxTouchPoints > 1 && userAgent.includes("safari"));
    return userAgent.includes("iphone") || userAgent.includes("ipod") || ipad ? "ios" : "md";
  };

  const routeKey = (value) => {
    try {
      const url = new URL(value, window.location.href);
      return `${url.pathname}${url.search}${url.hash}`;
    } catch (_) {
      return String(value);
    }
  };

  const animationFor = (from, to, isBack) => {
    const leaving = routeLayer(from);
    const entering = routeLayer(to);

    // Mirrors RouteTransitions::transition_back: the page being dismissed
    // owns reverse motion even when its deep-link fallback is not the actual
    // history destination.
    if (isBack && leaving === "cover") return "uncover-down";
    if (isBack && leaving === "pushed") return "push-right";

    if (leaving !== "cover" && entering === "cover") return "cover-up";
    if (leaving === "cover" && entering !== "cover") return "uncover-down";
    if (leaving === "root" && entering === "pushed") return "push-left";
    if (leaving === "pushed" && entering === "root") return "push-right";
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
      const observer =
        typeof MutationObserver === "undefined" ? null : new MutationObserver(finish);
      observer?.observe?.(document.body ?? document.documentElement, {
        childList: true,
        subtree: true,
      });
      const ceiling = window.setTimeout(finish, 120);
      if (!observer) finish();
    });

  // g3-ui reuses one scroll container for every route, so nothing resets
  // scrollTop the way a document-level scroller would: a new page opened at
  // whatever offset the previous one had been left at. Resetting from here,
  // at the moment the route commits, puts the reset after the outgoing frame
  // is captured and before the incoming one is, so the new page is snapshotted
  // already at the top and there is no visible jump. Restoring it afterwards
  // instead is what produced the old scroll-then-snap.
  const resetBodyScroll = () => {
    const scroller = document.querySelector?.(".g3-body-content");
    if (!scroller) return;
    try {
      scroller.scrollTop = 0;
    } catch (_) {}
  };

  let lastRoute = routeKey(window.location.href);

  // Keep the source side current for pushes, which do not emit popstate.
  for (const method of ["pushState", "replaceState"]) {
    const original = history[method];
    history[method] = function patched(...args) {
      const previous = lastRoute;
      const result = original.apply(this, args);
      lastRoute = routeKey(window.location.href);
      // Compare paths, not full keys: a query-only update is the same page
      // reconfiguring itself and should keep its place.
      if (routePath(previous) !== routePath(lastRoute)) resetBodyScroll();
      return result;
    };
  }

  const canAnimate = () =>
    Boolean(document.startViewTransition) &&
    !window.matchMedia?.("(prefers-reduced-motion: reduce)")?.matches &&
    !document.documentElement.dataset.routeTransition;

  const startTraversalTransition = (from, to, isBack = false) => {
    if (from === to || !canAnimate()) return null;
    const root = document.documentElement;
    root.dataset.routeTransition = animationFor(from, to, isBack);
    root.dataset.routeTransitionPlatform = transitionPlatform();
    // The class granting the outgoing snapshot its view-transition-name must
    // be computed before the old state is captured.
    void root.offsetHeight;

    const clear = () => {
      delete root.dataset.routeTransition;
      delete root.dataset.routeTransitionPlatform;
    };
    try {
      // Browser Back never calls pushState, so the scroll reset is attached to
      // the traversal callback instead - still inside the transition, so it is
      // captured rather than seen.
      const transition = document.startViewTransition(async () => {
        await routeRendered();
        resetBodyScroll();
      });
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
      const currentIndex = navigationApi.currentEntry?.index;
      const destinationIndex = event.destination?.index;
      const isBack =
        Number.isInteger(currentIndex) &&
        Number.isInteger(destinationIndex) &&
        destinationIndex < currentIndex;
      const transition = startTraversalTransition(from, to, isBack);
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
