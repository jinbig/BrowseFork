const { test, expect } = require("./fixture.cjs");
test.use({ hasTouch: true });

test("Android taps on every side dismiss the popup without opening a browser", async ({ page, launch }) => {
  await page.setViewportSize({ width: 400, height: 860 });
  await launch({ platform: "android" });
  for (const side of ["top", "right", "bottom", "left"]) {
    const popup = await page.locator("#shell").boundingBox();
    const point = { top: [200, popup.y / 2], right: [popup.x + popup.width + 4, 430],
      bottom: [200, popup.y + popup.height + 4], left: [popup.x / 2, 430] }[side];
    await page.touchscreen.tap(...point);
    await expect(page.locator("#chooser-view")).toBeHidden();
    await expect.poll(() => page.evaluate(() => window.__gui.state().presentation_requested)).toBe(null);
    expect(await page.evaluate(() => window.__gui.calls.some(call => call.command === "choose_browser"))).toBe(false);
    await page.evaluate(() => window.__gui.nextLink("https://outside-check.invalid/"));
    await expect(page.locator("#chooser-view")).toBeVisible();
  }
  const dismissals = await page.evaluate(() => window.__gui.calls.filter(call => call.command === "skip_link"));
  expect(dismissals).toHaveLength(4);
  expect(dismissals[0].args.expectedUrl).toContain("example.invalid");
});

test("inside controls, dragging out, and full-screen Android Settings do not dismiss", async ({ page, launch }) => {
  await page.setViewportSize({ width: 400, height: 860 });
  await launch({ platform: "android" });
  await page.getByLabel("Save choice", { exact: true }).tap();
  await expect(page.locator("#save-rule-fields")).toBeVisible();
  const inside = await page.locator("#pending-host").boundingBox();
  await page.mouse.move(inside.x + inside.width / 2, inside.y + inside.height / 2);
  await page.mouse.down(); await page.mouse.move(2, 2); await page.mouse.up();
  await expect(page.locator("#chooser-view")).toBeVisible();
  await page.locator("#switcher-settings").tap();
  await expect(page.locator("#settings-view")).toBeVisible();
  await page.touchscreen.tap(2, 2);
  await expect(page.locator("#settings-view")).toBeVisible();
  expect(await page.evaluate(() => window.__gui.calls.some(call => call.command === "skip_link"))).toBe(false);
});
