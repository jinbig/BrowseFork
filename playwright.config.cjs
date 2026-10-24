const path = require("node:path");
process.env.PLAYWRIGHT_BROWSERS_PATH ||= path.join(__dirname, ".gui-tools/browsers");
const { defineConfig } = require("@playwright/test");
module.exports = defineConfig({
  testDir: "./tests/gui",
  testMatch: "**/*.spec.cjs",
  fullyParallel: true,
  workers: 2,
  retries: 0,
  forbidOnly: true,
  timeout: 10000,
  expect: { timeout: 2000 },
  reporter: "list",
  outputDir: "target/gui-tests/browser-results",
  projects: [
    { name: "webkit", use: { browserName: "webkit" } },
    ...[1, 1.5, 2].map(scale => ({
      name: `windows-chromium-${scale * 100}pct`,
      testMatch: ["**/windows-appearance.spec.cjs", "**/profiles.spec.cjs", "**/android-dismiss.spec.cjs"],
      use: { browserName: "chromium", deviceScaleFactor: scale },
    })),
  ],
  use: {
    browserName: "webkit",
    viewport: { width: 360, height: 460 },
    deviceScaleFactor: 2,
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
});
