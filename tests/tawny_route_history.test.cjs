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
  destination: { url: "https://example.test/watch/abc" },
  intercept(options) {
    order.push("intercept");
    intercepted = options;
  },
});

assert.deepEqual(order, ["snapshot", "intercept"]);
assert.equal(root.dataset.routeTransition, "cover-up");
assert.equal(root.dataset.routeTransitionPlatform, "ios");
assert.equal(typeof intercepted.handler, "function");

intercepted.handler().then(() => {
  assert.equal(root.dataset.routeTransition, undefined);
});
