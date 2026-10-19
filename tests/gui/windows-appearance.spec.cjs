const { test, expect } = require("./fixture.cjs");

test("Windows chooser has transparent smooth corners and aligned controls at display scale", async ({ page, launch }, testInfo) => {
  await launch({ platform: "windows" });
  // Apply the native host's content-sized viewport, rather than inspecting a
  // tall arbitrary browser page that conceals gaps below the chooser.
  const layout = await page.evaluate(() => window.__gui.calls.filter(call => call.command === "set_picker_layout").at(-1).args);
  await page.setViewportSize({ width: layout.contentWidth, height: layout.contentHeight + 4 });
  const screenshot = await page.screenshot({ path: testInfo.outputPath("chooser-transparent.png"), omitBackground: true });
  await testInfo.attach("windows-chooser", { body: screenshot, contentType: "image/png" });
  const pixels = await page.evaluate(async base64 => {
    const image = new Image();
    image.src = `data:image/png;base64,${base64}`;
    await image.decode();
    const canvas = document.createElement("canvas");
    canvas.width = image.width; canvas.height = image.height;
    const context = canvas.getContext("2d"); context.drawImage(image, 0, 0);
    const alpha = (x, y) => context.getImageData(x, y, 1, 1).data[3];
    const scale = image.width / innerWidth;
    const edge = context.getImageData(0, 0, Math.ceil(24 * scale), Math.ceil(24 * scale)).data;
    return {
      corners: [[0, 0], [image.width - 1, 0], [0, image.height - 1], [image.width - 1, image.height - 1]].map(([x, y]) => alpha(x, y)),
      center: alpha(Math.floor(image.width / 2), Math.floor(image.height / 2)),
      antialiased: Array.from(edge).some((value, index) => index % 4 === 3 && value > 0 && value < 255),
    };
  }, screenshot.toString("base64"));
  expect(pixels.corners).toEqual([0, 0, 0, 0]);
  expect(pixels.center).toBe(255);
  expect(pixels.antialiased).toBe(true);
  const controls = await page.evaluate(() => {
    const checkbox = document.getElementById("save-choice");
    const save = checkbox.getBoundingClientRect();
    const card = document.getElementById("save-card").getBoundingClientRect();
    const icons = [...document.querySelectorAll(".browser-icon")].map(icon => icon.getBoundingClientRect());
    return {
      checkboxAppearance: getComputedStyle(checkbox).appearance,
      checkboxCenterOffset: (save.y + save.height / 2) - (card.y + card.height / 2),
      iconWidths: icons.map(icon => icon.width),
      overflow: document.documentElement.scrollWidth > innerWidth,
      settingsIcon: Boolean(document.querySelector("#switcher-settings svg")),
    };
  });
  expect(controls.checkboxAppearance).toBe("none");
  expect(Math.abs(controls.checkboxCenterOffset)).toBeLessThanOrEqual(1);
  expect(controls.iconWidths).toEqual([56, 56, 56]);
  expect(controls.overflow).toBe(false);
  expect(controls.settingsIcon).toBe(true);
  await page.getByLabel("Save choice", { exact: true }).check();
  await expect(page.locator("#save-rule-fields")).toBeVisible();
  await page.getByLabel("Save choice", { exact: true }).focus();
  await page.keyboard.press("Space");
  await expect(page.locator("#save-rule-fields")).toBeHidden();
});
