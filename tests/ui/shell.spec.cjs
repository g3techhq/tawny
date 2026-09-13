const {
  test,
  expect,
  openApp,
  expectNoHorizontalScroll,
  expectResponsiveNavigation,
} = require("./fixtures/app.cjs");

const primaryTabs = ["Feed", "Playlists", "Search", "Subscriptions"];

test.describe("Tawny application shell", () => {
  test.beforeEach(async ({ appPage }) => {
    await openApp(appPage);
  });

  test("adapts primary navigation to the viewport", async ({ appPage }) => {
    await expectResponsiveNavigation(appPage, primaryTabs);
    await expectNoHorizontalScroll(appPage);
    await expect(appPage.getByRole("tab", { name: "Feed", exact: true })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  test("navigates every primary destination and preserves selected state", async ({ appPage }) => {
    const destinations = [
      ["Playlists", /\/playlists$/],
      ["Search", /\/explore$/],
      ["Subscriptions", /\/subscriptions$/],
      ["Feed", /\/$/],
    ];

    for (const [label, route] of destinations) {
      const tab = appPage.getByRole("tab", { name: label, exact: true });
      await tab.click();
      await expect(appPage).toHaveURL(route);
      await expect(tab).toHaveAttribute("aria-selected", "true");
      await expectNoHorizontalScroll(appPage);
    }
  });

  test("opens an auxiliary screen and returns to the active section", async ({ appPage }) => {
    await appPage.getByRole("button", { name: "Settings", exact: true }).click();
    await expect(appPage).toHaveURL(/\/settings$/);
    await expect(appPage.getByText("Video speed", { exact: true })).toBeVisible();
    await expect(appPage.getByText("Duration filters", { exact: true })).toBeVisible();
    await expectNoHorizontalScroll(appPage);

    await appPage.getByRole("button", { name: "Back", exact: true }).click();
    await expect(appPage).toHaveURL(/\/$/);
  });
});
