"use strict";

// The timeline is drawn as one CSS gradient rather than as elements layered
// over the range input, because the range thumb lives inside the input and any
// overlay above the track covers it. That makes the gradient builder the whole
// of the drawing logic - and it is pure, so it can be tested without a browser.
//
// The functions are private to the controls IIFE, so they are lifted out of the
// source rather than imported. That keeps the test honest: it exercises the code
// that ships, and it fails loudly if either function is renamed or removed
// rather than silently testing a copy that has drifted.

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { test } = require("node:test");

const SOURCE = fs.readFileSync(
  path.join(__dirname, "..", "assets", "tawny_player_controls.js"),
  "utf8",
);

function lift(name) {
  const at = SOURCE.indexOf(`function ${name}(`);
  assert.notEqual(at, -1, `${name} is gone from tawny_player_controls.js`);
  // Walk braces to the end of the declaration.
  let depth = 0;
  let index = SOURCE.indexOf("{", at);
  for (; index < SOURCE.length; index++) {
    if (SOURCE[index] === "{") depth++;
    else if (SOURCE[index] === "}") {
      depth--;
      if (depth === 0) break;
    }
  }
  return SOURCE.slice(at, index + 1);
}

const { gradientFrom, clampPercent } = (() => {
  const factory = new Function(
    `${lift("clampPercent")}\n${lift("gradientFrom")}\nreturn { gradientFrom, clampPercent };`,
  );
  return factory();
})();

/** Pull `[color, from, to]` triples back out of a gradient string. */
function parse(gradient) {
  const inner = gradient.replace(/^linear-gradient\(to right, /, "").replace(/\)$/, "");
  return inner.split(", ").map((stop) => {
    const match = /^(.*) ([\d.]+)% ([\d.]+)%$/.exec(stop);
    assert.ok(match, `unparsable stop: ${stop}`);
    return [match[1], Number(match[2]), Number(match[3])];
  });
}

test("clampPercent keeps every stop inside the bar", () => {
  assert.equal(clampPercent(-10), 0);
  assert.equal(clampPercent(130), 100);
  assert.equal(clampPercent(Number.NaN), 0);
  assert.equal(clampPercent(42.5), 42.5);
});

test("the gradient covers the whole bar with no holes", () => {
  const stops = parse(gradientFrom([[0, 100, "grey"]]));
  assert.equal(stops[0][1], 0);
  assert.equal(stops[stops.length - 1][2], 100);
  for (let i = 0; i < stops.length - 1; i++) {
    assert.equal(stops[i][2], stops[i + 1][1], "a gap opened between stops");
  }
});

test("a later span paints over an earlier one", () => {
  // This is the whole reason spans exist rather than raw stops: a gradient has
  // no notion of one stop covering another.
  const stops = parse(
    gradientFrom([
      [0, 100, "base"],
      [0, 50, "played"],
    ]),
  );
  const at = (percent) => stops.find(([, from, to]) => percent >= from && percent < to)?.[0];
  assert.equal(at(25), "played");
  assert.equal(at(75), "base");
});

test("a sponsor colour survives under the played fill", () => {
  const stops = parse(
    gradientFrom([
      [0, 100, "base"],
      [0, 60, "played"],
      [20, 40, "#00d400"],
    ]),
  );
  const at = (percent) => stops.find(([, from, to]) => percent >= from && percent < to)?.[0];
  assert.equal(at(10), "played");
  assert.equal(at(30), "#00d400", "the segment must show through the fill");
  assert.equal(at(50), "played");
  assert.equal(at(80), "base");
});

test("gaps are transparent, so the background really shows through", () => {
  // Not a background-coloured tick: the old chapter markers were opaque
  // #05080c, which only looked like a gap against one particular backdrop.
  const stops = parse(
    gradientFrom([
      [0, 100, "base"],
      [49.5, 50.5, "transparent"],
    ]),
  );
  const gap = stops.find(([color]) => color === "transparent");
  assert.ok(gap, "no transparent slice was emitted");
  assert.equal(gap[1], 49.5);
  assert.equal(gap[2], 50.5);
});

test("a gap cuts through a segment colour, not just the track", () => {
  // The ask: gaps at the start and end of every sponsor section too.
  const stops = parse(
    gradientFrom([
      [0, 100, "base"],
      [20, 40, "#00d400"],
      [19.5, 20.5, "transparent"],
      [39.5, 40.5, "transparent"],
    ]),
  );
  const at = (percent) => stops.find(([, from, to]) => percent >= from && percent < to)?.[0];
  assert.equal(at(20), "transparent", "no gap at the segment start");
  assert.equal(at(30), "#00d400");
  assert.equal(at(40), "transparent", "no gap at the segment end");
});

test("zero-width spans do not emit empty stops", () => {
  const stops = parse(
    gradientFrom([
      [0, 100, "base"],
      [50, 50, "nothing"],
    ]),
  );
  assert.ok(
    stops.every(([, from, to]) => to > from),
    "an empty stop reached the gradient",
  );
  assert.ok(stops.every(([color]) => color !== "nothing"));
});

test("spans reaching past the bar are clamped rather than dropped", () => {
  const stops = parse(
    gradientFrom([
      [0, 100, "base"],
      [-20, 10, "intro"],
      [95, 140, "outro"],
    ]),
  );
  assert.equal(stops[0][0], "intro");
  assert.equal(stops[0][1], 0);
  assert.equal(stops[stops.length - 1][0], "outro");
  assert.equal(stops[stops.length - 1][2], 100);
});
