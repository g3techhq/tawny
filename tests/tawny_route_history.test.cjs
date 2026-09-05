const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");

const source = fs.readFileSync("assets/tawny_route_history.js", "utf8");
const navigationListeners = {};
const order = [];
const root = { dataset: {}, offsetHeight: 1 };

class TestMutationObserver {
  constructor(callback) {
    this.callback = callback;
  }

  observe() {
    queueMicrotask(() => this.callback());
  }

  disconnect() {}
}

const location = { href: "https://example.test/feed" };
const context = {
  URL,
  console,
  queueMicrotask,
  setTimeout,
  clearTimeout,
  MutationObserver: TestMutationObserver,
  location,
  history: {
    pushState() {},
    replaceState() {},
  },
  navigation: {
    currentEntry: { index: 1 },
    addEventListener(name, listener) {
      navigationListeners[name] = listener;
    },
  },
  matchMedia: () => ({ matches: false }),
  addEventListener() {},
};
context.window = context;
context.document = {
  body: {},
  documentElement: root,
  querySelector() {
    return { dataset: { g3Mode: "md" } };
  },
  startViewTransition(callback) {
    order.push("snapshot");
    const updateCallbackDone = Promise.resolve().then(callback);
    return { updateCallbackDone, finished: updateCallbackDone };
  },
};

vm.runInNewContext(source, context);
assert.equal(typeof navigationListeners.navigate, "function");

let intercepted;
navigationListeners.navigate({
  navigationType: "traverse",
  canIntercept: true,
  destination: { url: "https://example.test/watch/abc", index: 2 },
  intercept(options) {
    order.push("intercept");
    intercepted = options;
  },
});

assert.deepEqual(order, ["snapshot", "intercept"]);
assert.equal(root.dataset.routeTransition, "cover-up");
assert.equal(root.dataset.routeTransitionPlatform, "md");
assert.equal(typeof intercepted.handler, "function");

intercepted.handler().then(() => {
  assert.equal(root.dataset.routeTransition, undefined);

  location.href = "https://example.test/settings";
  context.history.pushState({}, "", location.href);
  order.length = 0;
  context.navigation.currentEntry.index = 2;

  let backIntercepted;
  navigationListeners.navigate({
    navigationType: "traverse",
    canIntercept: true,
    destination: { url: "https://example.test/", index: 1 },
    intercept(options) {
      order.push("intercept");
      backIntercepted = options;
    },
  });

  assert.deepEqual(order, ["snapshot", "intercept"]);
  assert.equal(root.dataset.routeTransition, "push-right");
  assert.equal(typeof backIntercepted.handler, "function");
  return backIntercepted.handler();
});
