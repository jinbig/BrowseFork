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
  use: {
    browserName: "webkit",
    viewport: { width: 360, height: 460 },
    deviceScaleFactor: 2,
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
});
