const { test, expect } = require("./fixture.cjs");

test("named Chrome profiles show initials, fit icons, and route the exact selected profile", async ({ page, launch }, testInfo) => {
  await page.setViewportSize({ width: 400, height: 460 });
  await launch({ platform: testInfo.project.name === "webkit" ? "macos" : "windows", profileNames: ["A", "S"] });
  const a = page.getByRole("button", { name: "Google Chrome · A", exact: true });
  const s = page.getByRole("button", { name: "Google Chrome · S", exact: true });
  await expect(a.locator(".browser-profile-label")).toHaveText("A");
  await expect(s.locator(".profile-badge")).toHaveText("S");
  const layout = await page.evaluate(() => {
    const strip = document.getElementById("browser-list").getBoundingClientRect();
    return [...document.querySelectorAll(".has-profile")].map(button => {
      const icon = button.querySelector(".browser-icon").getBoundingClientRect();
      const label = button.querySelector(".browser-profile-label").getBoundingClientRect();
      const badge = button.querySelector(".profile-badge").getBoundingClientRect();
      return icon.left >= strip.left && badge.right <= strip.right && badge.bottom <= label.top && label.bottom <= strip.bottom;
    });
  });
  expect(layout).toEqual([true, true]);
  const iconTops = await page.locator(".browser-icon").evaluateAll(icons => icons.map(icon => icon.getBoundingClientRect().top));
  expect(Math.max(...iconTops) - Math.min(...iconTops)).toBeLessThanOrEqual(1);
  await page.keyboard.press("Tab"); await page.keyboard.press("Tab"); await page.keyboard.press("Tab");
  await expect(s).toBeFocused();
  await expect(page.locator("#selected-browser")).toHaveText("Google Chrome · S");
  await page.getByLabel("Save choice", { exact: true }).check();
  await testInfo.attach("profiles", { body: await page.screenshot(), contentType: "image/png" });
  await s.click();
  await expect.poll(() => page.evaluate(() => window.__gui.calls.find(call => call.command === "choose_browser")?.args.browserId)).toBe("chrome-profile-1");
  await page.evaluate(() => window.__gui.nextLink("https://another.invalid/"));
  await expect(page.locator("#selected-browser")).toHaveText("Google Chrome · S");
});

test("Settings names and saves explicit profiles without inventing work or personal categories", async ({ page, launch }) => {
  await page.setViewportSize({ width: 680, height: 760 });
  await launch({ platform: "windows", role: "settings", profileNames: ["A", "S"] });
  await expect(page.locator("#rule-browser option")).toHaveText(["Brave Browser", "Google Chrome (last used profile)", "Google Chrome · A", "Google Chrome · S", "Safari"]);
  await page.locator("#rule-browser").selectOption("chrome-profile-1");
  await page.locator("#rule-pattern").fill("profile.invalid");
  await page.getByRole("button", { name: "Save rule", exact: true }).click();
  await expect(page.locator(".rule-row")).toContainText("Google Chrome · S");
  expect(await page.evaluate(() => window.__gui.state().rules[0].browser_id)).toBe("chrome-profile-1");
});


test("profile display modes persist independently of rules and waiting links", async ({ page, launch }) => {
  await page.setViewportSize({ width: 680, height: 900 });
  await launch({ role: "chooser", dedicated: false, profileNames: ["Work", "Personal"] });
  await page.evaluate(() => window.__TAURI__.core.invoke("replace_rules", { rules: [{ id: 1, mode: "host", pattern: "kept.invalid", browser_id: "chrome-profile-0", enabled: true }] }));
  const before = await page.evaluate(() => window.__gui.state());
  await page.locator("#switcher-settings").click();
  const mode = page.getByLabel("Chrome / Chromium profiles", { exact: true });
  await expect(mode).toHaveValue("all");
  await mode.selectOption("single");
  await expect.poll(() => page.evaluate(() => window.__gui.state().chooser_browser_ids)).toEqual(["browser-0", "browser-1", "browser-2"]);
  await mode.selectOption("selected");
  const work = page.getByRole("checkbox", { name: "Google Chrome · Work", exact: true });
  const personal = page.getByRole("checkbox", { name: "Google Chrome · Personal", exact: true });
  await expect(work).toBeChecked(); await expect(personal).toBeChecked();
  await mode.focus(); await page.keyboard.press("Tab"); await expect(work).toBeFocused();
  await work.uncheck();
  await expect.poll(() => page.evaluate(() => window.__gui.state().chooser_browser_ids)).toEqual(["browser-0", "chrome-profile-1", "browser-2"]);
  await personal.focus(); await page.keyboard.press("Space");
  await expect(page.locator("#message")).toContainText("Select at least one profile");
  await expect(personal).toBeChecked();
  await expect(personal).toBeFocused();
  const after = await page.evaluate(() => window.__gui.state());
  expect(after.pending_url).toBe(before.pending_url);
  expect(after.pending_count).toBe(before.pending_count);
  expect(after.rules).toEqual(before.rules);
  await page.locator("#settings-toggle").click();
  await expect(page.locator("#browser-list button")).toHaveCount(3);
  await expect(page.locator("#browser-list .browser-profile-label")).toHaveText(["Personal"]);
  await page.keyboard.press("Meta+2");
  await expect.poll(() => page.evaluate(() => window.__gui.calls.find(call => call.command === "choose_browser")?.args.browserId)).toBe("chrome-profile-1");
});

test("saved single mode shows normal Chrome and selected missing profiles keep browser usable", async ({ page, launch }) => {
  await launch({ profileNames: ["A", "S"], profilePreferences: { mode: "single", selected_profile_ids: ["chrome-profile-1"] } });
  await expect(page.locator("#browser-list button")).toHaveCount(3);
  await expect(page.getByRole("button", { name: "Google Chrome", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Google Chrome", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__gui.calls.find(call => call.command === "choose_browser")?.args.browserId)).toBe("browser-1");
});

test("saved selected mode routes shortcuts only through visible profiles and handles deletion", async ({ page, launch }) => {
  await launch({ profileNames: ["Work", "Personal"], profilePreferences: { mode: "selected", selected_profile_ids: ["chrome-profile-1"] } });
  await expect(page.locator("#browser-list .browser-profile-label")).toHaveText(["Personal"]);
  await page.evaluate(() => window.__gui.removeProfiles());
  await expect(page.getByRole("button", { name: "Google Chrome", exact: true })).toBeVisible();
  await expect(page.locator("#browser-list button")).toHaveCount(3);
});

test("Android does not offer unsupported Google-account display preferences", async ({ page, launch }) => {
  await launch({ role: "settings", platform: "android" });
  await expect(page.locator("#profile-preferences")).toBeHidden();
});
