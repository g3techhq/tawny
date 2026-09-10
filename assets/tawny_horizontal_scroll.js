// Vertical wheel over a horizontally-scrolling strip scrolls it sideways.
//
// The filter row overflows its width by design and hides its scrollbar, so on a
// desktop there was no way to reach the chips past the right edge: a mouse only
// reports vertical deltas, and shift+wheel is not something a reader will guess.
// Touch already pans these natively, so this only has to cover the pointer case.
(() => {
  "use strict";

  const SELECTOR = ".group-filter-row";
  const LINE_HEIGHT = 16;
  const PAGE_WIDTH_RATIO = 0.9;

  // Wheel deltas arrive in one of three units; treating a 3-line scroll as 3
  // pixels would look like nothing happened at all.
  const pixelsFor = (event) => {
    if (event.deltaMode === 1) return event.deltaY * LINE_HEIGHT;
    if (event.deltaMode === 2) return event.deltaY * PAGE_WIDTH_RATIO;
    return event.deltaY;
  };

  const onWheel = (event) => {
    const strip = event.target?.closest?.(SELECTOR);
    if (!strip) return;
    // A genuine horizontal gesture (trackpad) already works; leave it alone.
    if (Math.abs(event.deltaX) > Math.abs(event.deltaY)) return;
    if (strip.scrollWidth <= strip.clientWidth) return;

    const delta = pixelsFor(event);
    const target = strip.scrollLeft + delta;
    const limit = strip.scrollWidth - strip.clientWidth;
    // Let the page keep the scroll once the strip is against a stop, so a wheel
    // over the filters cannot trap the feed.
    if ((delta < 0 && strip.scrollLeft <= 0) || (delta > 0 && strip.scrollLeft >= limit)) {
      return;
    }
    event.preventDefault();
    strip.scrollLeft = Math.max(0, Math.min(limit, target));
  };

  // Not passive: the whole point is to take the gesture from the page.
  document.addEventListener("wheel", onWheel, { passive: false });
})();
