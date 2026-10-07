const { test, expect } = require("./fixture.cjs");

test("compact chooser shows only the host and fits three sharp icons", async ({ page, launch }) => {
  await launch();
  await expect(page.locator("#topbar")).toBeHidden();
  await expect(page.locator("#pending-host")).toHaveText("example.invalid");
  await expect(page.locator("body")).not.toContainText("test-secret");
  await expect(page.locator("#browser-list button")).toHaveCount(3);
  const layout = await page.evaluate(() => {
    const strip = document.getElementById("browser-list");
    const bounds = strip.getBoundingClientRect();
    return { scroll: strip.scrollWidth > strip.clientWidth,
      icons: [...strip.querySelectorAll("img")].map((image) => {
        const rect = image.getBoundingClientRect();
        return image.complete && image.naturalWidth > 0 && rect.left >= bounds.left && rect.right <= bounds.right && rect.top >= bounds.top && rect.bottom <= bounds.bottom;
      }) };
  });
  expect(layout.scroll).toBe(false);
  expect(layout.icons).toEqual([true, true, true]);
  await expect(page.locator(".browser-button.is-selected")).toHaveCount(1);
});

test("Tab reaches expanded Save choice controls and Escape never opens the link", async ({ page, launch }) => {
  await launch();
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "Brave Browser", exact: true })).toBeFocused();
  await page.keyboard.press("Tab"); await page.keyboard.press("Tab"); await page.keyboard.press("Tab");
  await expect(page.getByLabel("Save choice", { exact: true })).toBeFocused();
  await page.keyboard.press("Space");
  await expect(page.locator("#save-rule-fields")).toBeVisible();
  await page.keyboard.press("Tab"); await expect(page.locator("#save-source-app")).toBeFocused();
  await page.keyboard.press("Tab"); await expect(page.locator("#save-mode")).toBeFocused();
  await page.keyboard.press("Tab"); await expect(page.locator("#save-pattern")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect.poll(() => page.evaluate(() => window.__gui.state().pending_count)).toBe(0);
  expect(await page.evaluate(() => window.__gui.calls.some((call) => call.command === "choose_browser"))).toBe(false);
});

test("keyboard browser choice forwards the complete URL and restores selection before reopening", async ({ page, launch }) => {
  await launch();
  await page.keyboard.press("Meta+2");
  await expect.poll(() => page.evaluate(() => window.__gui.state().pending_count)).toBe(0);
  const choice = await page.evaluate(() => window.__gui.calls.find((call) => call.command === "choose_browser"));
  expect(choice.args.browserId).toBe("browser-1");
  expect(choice.args.expectedUrl).toBe("https://example.invalid/private?token=test-secret#fragment");
  await page.evaluate(() => window.__gui.nextLink("https://example.invalid/private?token=test-secret#fragment"));
  await expect(page.locator("#selected-browser")).toHaveText("Google Chrome");
  const prepared = await page.evaluate(() => window.__gui.calls.filter((call) => call.command === "set_picker_layout").at(-1).selectedAtCall);
  expect(prepared).toBe("Google Chrome");
  await page.getByRole("button", { name: "Brave Browser", exact: true }).focus();
  await expect(page.locator("#selected-browser")).toHaveText("Google Chrome");
});

test("Settings can add, edit, reorder, toggle, and delete rules without showing the chooser", async ({ page, launch }) => {
  await page.setViewportSize({ width: 680, height: 760 });
  await launch({ role: "settings" });
  await expect(page.locator("#picker-style")).toHaveCount(0);
  for (const pattern of ["first.invalid", "second.invalid"]) {
    await page.locator("#settings-view").getByLabel("Pattern", { exact: true }).fill(pattern);
    await page.getByRole("button", { name: "Save rule", exact: true }).click();
    await expect(page.locator("#settings-view")).toBeVisible();
    await expect(page.locator("#chooser-view")).toBeHidden();
  }
  await expect(page.locator(".rule-row")).toHaveCount(2);
  await page.locator(".rule-row").first().getByRole("button", { name: "Move rule down", exact: true }).click();
  await expect(page.locator(".rule-row").first()).toContainText("first.invalid");
  await page.locator(".rule-row").first().getByRole("button", { name: "Edit rule", exact: true }).click();
  await page.locator("#settings-view").getByLabel("Pattern", { exact: true }).fill("edited.invalid");
  await page.getByRole("button", { name: "Save rule", exact: true }).click();
  await page.locator(".rule-row").first().getByRole("button", { name: "Toggle rule", exact: true }).click();
  await expect(page.locator(".rule-row").first()).toContainText("off");
  await page.locator(".rule-row").first().getByRole("button", { name: "Delete rule", exact: true }).click();
  await expect(page.locator(".rule-row")).toHaveCount(1);
  await expect(page.locator("#settings-view")).toBeVisible();
  expect(await page.evaluate(() => window.__gui.calls.filter((call) => call.command === "set_picker_layout").every((call) => call.args.settings))).toBe(true);
});

test("Settings rejects an all-app/all-URL rule", async ({ page, launch }) => {
  await launch({ role: "settings" });
  await page.getByRole("button", { name: "Save rule", exact: true }).click();
  await expect(page.locator("#message")).toContainText("Choose a source app or enter a URL pattern");
  expect(await page.evaluate(() => window.__gui.state().rules.length)).toBe(0);
});

test("many browsers scroll instead of clipping inaccessible choices", async ({ page, launch }) => {
  await launch({ browserCount: 12 });
  const last = page.getByRole("button", { name: "Browser 12", exact: true });
  await last.scrollIntoViewIfNeeded();
  const rect = await last.boundingBox();
  expect(rect.x).toBeGreaterThanOrEqual(0);
  expect(rect.x + rect.width).toBeLessThanOrEqual(360);
  await last.click();
  const choice = await page.evaluate(() => window.__gui.calls.find((call) => call.command === "choose_browser"));
  expect(choice.args.browserId).toBe("browser-11");
});

test("Android touch viewport has readable labels and no horizontal overflow", async ({ page, launch }) => {
  await page.setViewportSize({ width: 360, height: 800 });
  await launch({ platform: "android" });
  await expect(page.locator("#keyboard-hint")).toBeHidden();
  await expect(page.locator(".browser-name").first()).toBeVisible();
  const layout = await page.evaluate(() => ({
    overflow: document.documentElement.scrollWidth > innerWidth,
    icons: [...document.querySelectorAll(".browser-icon")].map((image) => image.getBoundingClientRect().width),
  }));
  expect(layout.overflow).toBe(false);
  expect(layout.icons).toEqual([64, 64, 64]);
  await page.getByLabel("Save choice", { exact: true }).check();
  await expect(page.locator("#save-rule-fields").getByLabel("Pattern", { exact: true })).toBeVisible();
});
