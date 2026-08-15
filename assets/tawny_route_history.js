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

  // What the page looks like right now, as something to compare against.
  //
  // The route's own element is the signal: every page renders its own `main`,
  // and the watch page is the only one carrying the cover marker.
  const routeFingerprint = () => {
    const main = document.querySelector("main");
    const sheet = document.querySelector(".route-transition-cover");
    return `${main ? main.className : ""}|${sheet ? "sheet" : ""}`;
  };

  // The router re-renders asynchronously, so the transition callback has to
  // hold the snapshot open until the new route is actually in the DOM.
  //
  // Waiting for *any* mutation was not enough: the player ticks its clock and
  // toasts come and go, so the first mutation frequently had nothing to do
  // with the route, and the new snapshot was taken of the old page.
  const routeRendered = (before) =>
    new Promise((resolve) => {
      let settled = false;
      const finish = () => {
        if (settled) return;
        settled = true;
        observer.disconnect();
        window.clearTimeout(timer);
        resolve();
      };
      const check = () => {
        if (routeFingerprint() !== before) finish();
      };
      const observer = new MutationObserver(check);
      observer.observe(document.body ?? document.documentElement, {
        childList: true,
        subtree: true,
        attributes: true,
      });
      // Never hold a navigation open indefinitely: a route that renders
      // identically still has to complete.
      const timer = window.setTimeout(finish, 600);
      check();
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
    // Captured before the call, so the wait inside the callback is comparing
    // against the page that is still on screen.
    const before = routeFingerprint();
    try {
      const transition = document.startViewTransition(() => routeRendered(before));
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
      // Snapshot here, synchronously, rather than inside the intercept
      // handler. The handler runs later — after popstate, which is what the
      // router listens to — so by then the new route has already rendered and
      // been painted. That is the whole bug: the page appeared first and the
      // animation then played over the top of it. Nothing has reacted to the
      // traversal yet at this point, so the outgoing page is still on screen.
      const finished = startTransition(from, to);
      event.intercept({ scroll: "manual", handler: () => finished });
    });
    return;
  }

  // Fallback for engines without the Navigation API. This one cannot help
  // being late — popstate fires after the traversal has committed — so the
  // animation is best-effort there.
  window.addEventListener("popstate", () => {
    const from = lastPath;
    const to = window.location.pathname;
    lastPath = to;
    if (!shouldAnimate(from, to)) return;
    startTransition(from, to);
  });
})();
