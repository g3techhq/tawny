const { test, expect, openApp, expectNoHorizontalScroll } = require("./fixtures/app.cjs");

test.describe("Tawny library interactions", () => {
  test("validates and creates a local playlist", async ({ appPage }) => {
    await openApp(appPage, "/playlists");
    await appPage.getByRole("button", { name: "New playlist", exact: true }).first().click();

    const dialog = appPage.getByRole("dialog");
    const create = dialog.getByRole("button", { name: "Create", exact: true });
    await expect(dialog.getByRole("heading", { name: "New playlist", exact: true })).toBeVisible();
    await expect(create).toBeDisabled();

    await dialog.getByLabel("Playlist name", { exact: true }).fill("UI test playlist");
    await expect(create).toBeEnabled();
    await create.click();

    await expect(dialog).toBeHidden();
    await expect(appPage.getByText("UI test playlist", { exact: true })).toBeVisible();

    await appPage.getByText("UI test playlist", { exact: true }).click();
    await expect(appPage.locator("header").getByText("0 videos", { exact: true })).toBeVisible();
    await expectNoHorizontalScroll(appPage);
  });

  test("starts with useful duration defaults and a differentiated light theme", async ({
    appPage,
  }) => {
    await openApp(appPage, "/settings");

    await expect(appPage.getByLabel("Video · Short max (minutes)")).toHaveValue("10");
    await expect(appPage.getByLabel("Video · Medium max (minutes)")).toHaveValue("35");

    await appPage.getByRole("radio", { name: "Light", exact: true }).click();
    await expect
      .poll(() =>
        appPage
          .locator(".g3-app-shell")
          .evaluate((element) =>
            getComputedStyle(element).getPropertyValue("--g3-color-bg").trim(),
          ),
      )
      .toBe("#d8c5a8");

    const themeLayers = await appPage.locator(".g3-app-shell").evaluate((element) => {
      const styles = getComputedStyle(element);
      return [
        "--g3-color-bg",
        "--g3-color-bg-secondary",
        "--g3-color-card",
        "--g3-color-surface",
        "--g3-color-control",
      ].map((token) => styles.getPropertyValue(token).trim());
    });
    expect(new Set(themeLayers).size).toBe(themeLayers.length);
    expect(themeLayers).not.toContain("#ffffff");

    const sponsorCard = appPage.locator(".g3-card").filter({ hasText: "SponsorBlock" }).first();
    const surfaces = await sponsorCard.evaluate((card) => ({
      card: getComputedStyle(card).backgroundColor,
      item: getComputedStyle(card.querySelector(".g3-list .g3-item")).backgroundColor,
    }));
    expect(surfaces.item).not.toBe(surfaces.card);
    await expectNoHorizontalScroll(appPage);
  });

  test("replaces a desktop action toast and disables desktop card swipes", async ({
    appPage,
  }, testInfo) => {
    test.skip(testInfo.project.name !== "desktop-chromium", "desktop pointer behavior");
    await openApp(appPage);

    // A mouse reaches the swipe actions through the row's actions button.
    const row = appPage.locator(".g3-swipe-item").first();
    const showActions = row.getByRole("button", { name: "Show actions" });
    const action = row.locator(".g3-swipe-actions-end .g3-swipe-action");
    await expect
      .poll(() => row.evaluate((element) => element.style.getPropertyValue("--g3-swipe-offset")))
      .toBe("0px");

    await row.hover();
    await showActions.click();
    await action.click();
    const toast = appPage.locator(".g3-toast");
    await expect(toast).toHaveAttribute("data-state", "open");
    const firstToast = await toast.elementHandle();

    await row.hover();
    await showActions.click();
    await action.click();
    await expect.poll(() => firstToast.evaluate((element) => element.isConnected)).toBe(false);

    const box = await row.boundingBox();
    await appPage.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await appPage.mouse.down();
    await appPage.mouse.move(box.x + box.width / 2 + 160, box.y + box.height / 2);
    await appPage.mouse.up();
    await expect
      .poll(() => row.evaluate((element) => element.style.getPropertyValue("--g3-swipe-offset")))
      .toBe("0px");
  });

  test("prevents native dragging of desktop card thumbnails", async ({ appPage }, testInfo) => {
    test.skip(testInfo.project.name !== "desktop-chromium", "desktop pointer behavior");
    await openApp(appPage);

    const thumbnails = appPage.locator(".g3-card .g3-card-media img");
    const thumbnail = thumbnails.first();
    await expect(thumbnail).toBeVisible();
    await expect
      .poll(() => thumbnail.evaluate((image) => getComputedStyle(image).webkitUserDrag))
      .toBe("none");

    await thumbnail.evaluate((image) => {
      image.dataset.dragStarted = "false";
      image.addEventListener("dragstart", () => {
        image.dataset.dragStarted = "true";
      });
    });
    const box = await thumbnail.boundingBox();
    await appPage.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await appPage.mouse.down();
    await appPage.mouse.move(box.x + box.width / 2 + 120, box.y + box.height / 2, { steps: 8 });
    await appPage.mouse.up();
    await expect(thumbnail).toHaveAttribute("data-drag-started", "false");
  });

  test("repeated player edge clicks accumulate ten-second seeks", async ({ appPage }, testInfo) => {
    test.skip(testInfo.project.name !== "desktop-chromium", "desktop pointer behavior");
    await openApp(appPage);

    await appPage.locator(".g3-card .g3-card-action").first().click();
    const player = appPage.locator("#tawny-player");
    const media = appPage.locator("#tawny-player-media");
    await expect(player).toBeVisible();
    await expect(media).toBeVisible();
    await expect(appPage.getByRole("button", { name: "PiP", exact: true })).toHaveClass(
      /g3-btn-sm/,
    );

    await media.evaluate((video) => {
      // Keep this deterministic without replacing the actual player DOM: a
      // remote media response is irrelevant to testing its input wiring.
      Object.defineProperty(video, "duration", { configurable: true, value: 120 });
      Object.defineProperty(video, "currentTime", {
        configurable: true,
        writable: true,
        value: 60,
      });
      window.TawnyPlayerControls.attach(video);
    });

    const box = await player.boundingBox();
    await appPage.mouse.dblclick(box.x + 50, box.y + box.height / 2);
    await expect.poll(() => media.evaluate((video) => video.currentTime)).toBe(50);

    await appPage.waitForTimeout(600);
    await appPage.mouse.dblclick(box.x + box.width - 50, box.y + box.height / 2);
    await expect.poll(() => media.evaluate((video) => video.currentTime)).toBe(60);

    // Some embedded browsers emit two ordinary clicks without incrementing
    // click.detail or producing dblclick. Pointer presses must still seek.
    await appPage.waitForTimeout(600);
    await appPage.evaluate(
      ({ x, y }) => {
        const target = document.elementFromPoint(x, y);
        for (let index = 0; index < 2; index += 1) {
          target.dispatchEvent(
            new PointerEvent("pointerdown", {
              bubbles: true,
              clientX: x,
              clientY: y,
              pointerId: 42,
              pointerType: "touch",
            }),
          );
          target.dispatchEvent(
            new PointerEvent("pointerup", {
              bubbles: true,
              clientX: x,
              clientY: y,
              pointerId: 42,
              pointerType: "touch",
            }),
          );
          target.dispatchEvent(
            new MouseEvent("click", {
              bubbles: true,
              clientX: x,
              clientY: y,
              detail: 1,
            }),
          );
        }
      },
      { x: box.x + 50, y: box.y + box.height / 2 },
    );
    await expect.poll(() => media.evaluate((video) => video.currentTime)).toBe(50);

    const clickEdge = async (x, count) => {
      await appPage.waitForTimeout(600);
      for (let index = 0; index < count; index += 1) {
        await appPage.mouse.click(x, box.y + box.height / 2);
        await appPage.waitForTimeout(60);
      }
    };

    await clickEdge(box.x + box.width - 50, 3);
    await expect.poll(() => media.evaluate((video) => video.currentTime)).toBe(70);
    await expect(appPage.locator(".player-seek-feedback-right span")).toHaveText("20 seconds");

    await clickEdge(box.x + 50, 4);
    await expect.poll(() => media.evaluate((video) => video.currentTime)).toBe(40);
    await expect(appPage.locator(".player-seek-feedback-left span")).toHaveText("30 seconds");
  });

  test("exposes search and filter controls without overflowing", async ({ appPage }) => {
    await openApp(appPage, "/explore");
    await expect(appPage.getByPlaceholder("Search videos and channels")).toBeVisible();
    for (const label of ["All", "Videos", "Channels"]) {
      await expect(appPage.getByRole("radio", { name: label, exact: true })).toBeVisible();
    }
    await expectNoHorizontalScroll(appPage);
  });
});
