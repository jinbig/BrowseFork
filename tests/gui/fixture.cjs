const { test: base, expect } = require("@playwright/test");
const fs = require("node:fs");
const path = require("node:path");
const ui = path.join(__dirname, "../../ui");
const test = base.extend({
  launch: async ({ page }, use) => {
    await page.route("**/*", (route) => {
      const url = new URL(route.request().url());
      const file = { "/index.html": "index.html", "/main.js": "main.js", "/styles.css": "styles.css" }[url.pathname];
      if (url.hostname !== "browsefork.test" || !file) return route.fulfill({ status: 404, body: "" });
      return route.fulfill({ contentType: file.endsWith("js") ? "text/javascript" : file.endsWith("css") ? "text/css" : "text/html", body: fs.readFileSync(path.join(ui, file)) });
    });
    await use(async ({ role = "chooser", platform = "macos", browserCount = 3 } = {}) => {
      await page.addInitScript(({ role, platform, browserCount }) => {
        const names = ["Brave Browser", "Google Chrome", "Safari"];
        const browsers = Array.from({ length: browserCount }, (_, index) => {
          const name = names[index] || `Browser ${index + 1}`;
          const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96"><rect width="96" height="96" rx="20" fill="${["#e94b25", "#238de7", "#36b1ee"][index % 3]}"/><text x="48" y="65" text-anchor="middle" font-size="50" fill="white">${index + 1}</text></svg>`;
          return { id: `browser-${index}`, name, icon_data_url: `data:image/svg+xml,${encodeURIComponent(svg)}` };
        });
        let state = {
          platform, ready: true, presentation_revision: 1,
          presentation_requested: role === "settings", is_default_browser: false,
          pending_url: role === "settings" ? null : "https://example.invalid/private?token=test-secret#fragment",
          pending_source_app_id: null, pending_count: role === "settings" ? 0 : 1,
          suggested_pattern: role === "settings" ? null : "example.invalid", browsers, source_apps: [], rules: [], error: null,
        };
        const listeners = new Map();
        const calls = [];
        const emit = (name) => { for (const fn of listeners.get(name) || []) fn({ payload: null }); };
        window.__gui = {
          calls,
          state: () => structuredClone(state),
          nextLink(url) {
            state = { ...state, pending_url: url, pending_count: 1, suggested_pattern: new URL(url).host,
              presentation_requested: false, presentation_revision: state.presentation_revision + 1 };
            emit("browsefork-state-changed");
          },
          refresh() { emit("browsefork-platform-changed"); },
        };
        window.__TAURI__ = {
          event: { listen(name, fn) { listeners.set(name, [...(listeners.get(name) || []), fn]); return Promise.resolve(() => {}); } },
          core: { async invoke(command, args) {
            calls.push({ command, args, selectedAtCall: document.getElementById("selected-browser")?.textContent });
            if (command === "get_snapshot") return structuredClone(state);
            if (command === "set_picker_layout" || command === "request_default_browser") return;
            if (command === "set_view") {
              state = { ...state, presentation_requested: args.settings, presentation_revision: state.presentation_revision + 1 };
              emit(args.settings ? "browsefork-open-preferences" : "browsefork-state-changed"); return;
            }
            if (command === "replace_rules") {
              state = { ...state, rules: args.rules }; emit("browsefork-rules-changed"); return structuredClone(state);
            }
            if (command === "choose_browser" || command === "skip_link" || command === "dismiss_window") {
              if (command === "dismiss_window" && !args.expectedUrl) {
                state = { ...state, presentation_requested: null };
              } else {
                state = { ...state, pending_url: null, pending_count: 0, suggested_pattern: null, presentation_requested: null };
              }
              const snapshot = structuredClone(state);
              return command === "choose_browser" ? { snapshot, warning: null } : snapshot;
            }
            throw new Error(`Unexpected GUI command: ${command}`);
          } },
        };
      }, { role, platform, browserCount });
      await page.goto(`http://browsefork.test/index.html?view=${role}`);
      await expect(page.locator(role === "settings" ? "#settings-view" : "#chooser-view")).toBeVisible();
    });
  },
});
module.exports = { test, expect };
