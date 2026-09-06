const { test: base, expect } = require("@playwright/test");

const test = base.extend({
  appPage: async ({ page }, use, testInfo) => {
    const pageErrors = [];
    const consoleErrors = [];

    page.on("pageerror", (error) => pageErrors.push(error.stack || error.message));
    page.on("console", (message) => {
      if (message.type() === "error") consoleErrors.push(message.text());
    });
    await page.emulateMedia({ reducedMotion: "reduce" });

    await use(page);

    if (consoleErrors.length) {
      await testInfo.attach("browser-console-errors", {
        body: consoleErrors.join("\n"),
        contentType: "text/plain",
      });
    }
    expect(pageErrors, `Uncaught browser errors:\n${pageErrors.join("\n")}`).toEqual([]);
  },
});

async function openApp(page, path = "/") {
  await page.goto(path, { waitUntil: "domcontentloaded" });
  await expect(page.locator(".g3-app-shell")).toBeVisible();
}

async function expectNoHorizontalScroll(page) {
  const sizes = await page.evaluate(() => ({
    viewport: document.documentElement.clientWidth,
    document: document.documentElement.scrollWidth,
    body: document.body.scrollWidth,
  }));
  expect(Math.max(sizes.document, sizes.body)).toBeLessThanOrEqual(sizes.viewport + 1);
}

async function expectResponsiveNavigation(page, labels) {
  const viewport = page.viewportSize();
  const tablist = page.getByRole("tablist", { name: "Primary navigation" });
  await expect(tablist).toBeVisible();

  const tablistBox = await tablist.boundingBox();
  const boxes = [];
  for (const label of labels) {
    const tab = page.getByRole("tab", { name: label, exact: true });
    await expect(tab).toBeVisible();
    boxes.push(await tab.boundingBox());
  }

  expect(viewport).not.toBeNull();
  expect(tablistBox).not.toBeNull();
  expect(boxes.every(Boolean)).toBe(true);

  if (viewport.width >= 768) {
    expect(tablistBox.width).toBeLessThan(100);
    expect(
      Math.max(...boxes.map((box) => box.x)) - Math.min(...boxes.map((box) => box.x)),
    ).toBeLessThan(8);
  } else {
    expect(tablistBox.width).toBeGreaterThan(viewport.width * 0.8);
    expect(
      Math.max(...boxes.map((box) => box.y)) - Math.min(...boxes.map((box) => box.y)),
    ).toBeLessThan(8);
  }
}

module.exports = { test, expect, openApp, expectNoHorizontalScroll, expectResponsiveNavigation };
