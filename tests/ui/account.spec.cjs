const { test, expect } = require("@playwright/test");

test.describe("Tawny first launch", () => {
  test("requires a complete server URL before connecting", async ({ page }) => {
    await page.addInitScript(() => {
      window.localStorage.removeItem("tawny.backend-url");
    });

    await page.goto("/", { waitUntil: "domcontentloaded" });
    await expect(page.getByRole("heading", { name: "Connect to a Tawny server" })).toBeVisible();

    await page.getByRole("textbox", { name: "Server address" }).fill("tawny.example");
    await page.getByRole("button", { name: "Connect", exact: true }).click();
    await expect(
      page.getByText("Enter a full address, including http:// or https://.", { exact: true }),
    ).toBeVisible();
  });
});
