const { test, expect, openApp, expectNoHorizontalScroll } = require("./fixtures/app.cjs");

test.describe("Tawny library interactions", () => {
  test("validates and creates a local playlist", async ({ appPage }) => {
    await openApp(appPage, "/playlists");
    await appPage.getByRole("button", { name: "New playlist", exact: true }).click();

    const dialog = appPage.getByRole("alertdialog");
    const create = dialog.getByRole("button", { name: "Create", exact: true });
    await expect(dialog.getByRole("heading", { name: "New playlist", exact: true })).toBeVisible();
    await expect(create).toBeDisabled();

    await dialog.getByLabel("Playlist name", { exact: true }).fill("UI test playlist");
    await expect(create).toBeEnabled();
    await create.click();

    await expect(dialog).toBeHidden();
    await expect(appPage.getByText("UI test playlist", { exact: true })).toBeVisible();
    await expectNoHorizontalScroll(appPage);
  });

  test("exposes search and filter controls without overflowing", async ({ appPage }) => {
    await openApp(appPage, "/explore");
    await expect(appPage.getByPlaceholder("Search videos and channels")).toBeVisible();
    for (const label of ["All", "Videos", "Channels"]) {
      await expect(appPage.getByRole("tab", { name: label, exact: true })).toBeVisible();
    }
    await expectNoHorizontalScroll(appPage);
  });
});
