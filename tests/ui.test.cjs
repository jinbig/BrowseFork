const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

const source = fs.readFileSync(path.join(__dirname, "../ui/main.js"), "utf8");

function chooser({ framesEnabled = true } = {}) {
  const calls = [];
  const listeners = new Map();
  const elements = new Map();
  let snapshot = {
    presentation_revision: 1,
    pending_url: "https://ya.ru/private/page",
    pending_source_app_id: "com.tinyspeck.slackmacgap",
    pending_count: 1,
    suggested_pattern: "ya.ru",
    picker_style: "switcher",
    browsers: [
      { id: "brave", name: "Brave", icon_data_url: null },
      { id: "safari", name: "Safari", icon_data_url: null },
    ],
    rules: [],
    error: null,
  };

  class FakeElement {
    constructor(tagName = "div", id = "") {
      this.tagName = tagName.toUpperCase();
      this.id = id;
      this.children = [];
      this.handlers = new Map();
      this.hidden = false;
      this.checked = false;
      this.disabled = false;
      this.value = "";
      this.textContent = "";
      this.scrollHeight = 390;
      const classes = new Set();
      this.classList = {
        add: (name) => classes.add(name),
        remove: (name) => classes.delete(name),
        contains: (name) => classes.has(name),
        toggle: (name, force) => {
          if (force ?? !classes.has(name)) classes.add(name);
          else classes.delete(name);
        },
      };
    }

    get options() {
      return this.children.filter((child) => child.tagName === "OPTION");
    }

    append(...children) {
      this.children.push(...children);
    }

    replaceChildren(...children) {
      this.children = children;
    }

    querySelector(selector) {
      if (selector === 'option[value="host"]') {
        return this.options.find((option) => option.value === "host") || null;
      }
      throw new Error(`Unexpected selector: ${selector}`);
    }

    querySelectorAll(selector) {
      if (selector === "button") return this.children.filter((child) => child.tagName === "BUTTON");
      throw new Error(`Unexpected selector: ${selector}`);
    }

    addEventListener(type, callback) {
      const callbacks = this.handlers.get(type) || [];
      callbacks.push(callback);
      this.handlers.set(type, callbacks);
    }

    dispatch(type, event = {}) {
      for (const callback of this.handlers.get(type) || []) callback({ target: this, ...event });
    }

    click() {
      if (this.tagName === "INPUT" && this.id === "save-choice") {
        this.checked = !this.checked;
        this.dispatch("change");
      }
      this.dispatch("click");
    }

    focus() {
      document.activeElement = this;
      this.dispatch("focus");
    }

    blur() {
      if (document.activeElement === this) document.activeElement = null;
    }

    setAttribute() {}
    reset() {}
  }

  const document = {
    activeElement: null,
    body: new FakeElement("body"),
    getElementById(id) {
      if (!elements.has(id)) elements.set(id, new FakeElement("div", id));
      return elements.get(id);
    },
    createElement(tag) {
      return new FakeElement(tag);
    },
    addEventListener(type, callback) {
      this.handlers.set(type, callback);
    },
    handlers: new Map(),
  };
  const $ = (id) => document.getElementById(id);
  for (const id of ["chooser-view", "settings-view", "save-rule-fields"]) $(id).hidden = true;
  $("save-choice").tagName = "INPUT";
  $("save-mode").tagName = "SELECT";
  $("save-source-app").tagName = "SELECT";
  $("save-mode").value = "host";
  for (const value of ["host", "contains", "regex"]) {
    const option = new FakeElement("option");
    option.value = value;
    $("save-mode").append(option);
  }
  $("rule-browser").tagName = "SELECT";

  const invoke = async (command, args) => {
    calls.push({ command, args });
    if (command === "get_snapshot") return snapshot;
    if (command === "set_picker_style") {
      snapshot = { ...snapshot, picker_style: args.style };
      return snapshot;
    }
    if (command === "dismiss_window") {
      if (args.expectedRevision !== snapshot.presentation_revision) return snapshot;
      snapshot = { ...snapshot, presentation_revision: snapshot.presentation_revision + 1 };
      if (!args.expectedUrl) return snapshot;
    }
    if (command === "choose_browser" || command === "skip_link" || command === "dismiss_window") {
      snapshot = { ...snapshot, pending_url: null, pending_count: 0, suggested_pattern: null };
      return command === "choose_browser" ? { snapshot, warning: null } : snapshot;
    }
    if (command === "replace_rules") return { ...snapshot, rules: args.rules };
    return undefined;
  };
  const context = {
    document,
    navigator: { platform: "MacIntel" },
    window: { __TAURI__: { core: { invoke }, event: { listen: (name, callback) => listeners.set(name, callback) } } },
    HTMLElement: FakeElement,
    URL,
    requestAnimationFrame: (callback) => { if (framesEnabled) callback(); },
  };
  vm.runInNewContext(source, context, { filename: "ui/main.js" });
  return {
    $,
    calls,
    document,
    key(key, shiftKey = false, modifiers = {}) {
      let prevented = false;
      document.handlers.get("keydown")({ key, shiftKey, ...modifiers, preventDefault() { prevented = true; } });
      return prevented;
    },
    setSnapshot(value, event = "browsefork-state-changed") {
      snapshot = value;
      listeners.get(event)();
    },
    inactive(payload = { expected_revision: snapshot.presentation_revision,
      expected_url: document.getElementById("chooser-view").hidden ? null : snapshot.pending_url }) {
      listeners.get("browsefork-window-inactive")({ payload });
    },
  };
}

const settle = () => new Promise((resolve) => setImmediate(resolve));

test("Tab focuses one browser at a time and reaches the expanded save controls", async () => {
  const app = chooser();
  await settle();
  const buttons = app.$("browser-list").querySelectorAll("button");
  assert.equal(app.$("pending-host").textContent, "ya.ru");
  assert.equal(buttons.length, 2);
  assert.equal(app.document.body.classList.contains("picker-switcher"), true);
  assert.equal(app.$("topbar").hidden, true);
  assert.equal(app.$("switcher-settings").hidden, false);
  assert.equal(app.$("selected-browser").textContent, "Brave");
  assert.equal(app.document.body.classList.contains("keyboard-navigation"), false);
  assert.equal(app.key("Tab"), true);
  assert.equal(app.document.activeElement, buttons[0]);
  assert.equal(app.document.body.classList.contains("keyboard-navigation"), true);
  app.key("Tab");
  assert.equal(app.document.activeElement, buttons[1]);
  app.key("Tab");
  assert.equal(app.document.activeElement, app.$("save-choice"));
  app.key("Enter");
  assert.equal(app.$("save-choice").checked, true);
  assert.equal(app.$("save-rule-fields").hidden, false);
  app.key("Tab");
  assert.equal(app.document.activeElement, app.$("save-source-app"));
  app.key("Tab");
  assert.equal(app.document.activeElement, app.$("save-mode"));
  app.key("Tab");
  assert.equal(app.document.activeElement, app.$("save-pattern"));
  app.key("Tab", true);
  assert.equal(app.document.activeElement, app.$("save-mode"));
});

test("switcher arrows change the highlighted browser and Enter opens it", async () => {
  const app = chooser();
  await settle();
  assert.equal(app.key("ArrowRight"), true);
  assert.equal(app.$("selected-browser").textContent, "Safari");
  assert.equal(app.key("Enter"), true);
  await settle();
  const chosen = app.calls.find((call) => call.command === "choose_browser");
  assert.equal(chosen.args.browserId, "safari");
  assert.equal(chosen.args.saveRule, null);
});

test("switcher command-number shortcut opens the matching browser", async () => {
  const app = chooser();
  await settle();
  assert.equal(app.key("2", false, { metaKey: true }), true);
  await settle();
  assert.equal(app.calls.find((call) => call.command === "choose_browser").args.browserId, "safari");
});

test("picker preference switches to the original list and persists the selection", async () => {
  const app = chooser();
  await settle();
  app.$("switcher-settings").click();
  assert.equal(app.$("topbar").hidden, false);
  assert.equal(app.$("switcher-settings").hidden, true);
  app.$("picker-style").value = "list";
  app.$("picker-style").dispatch("change");
  await settle();
  assert.equal(app.calls.find((call) => call.command === "set_picker_style").args.style, "list");
  assert.equal(app.$("picker-style").value, "list");
  app.$("settings-toggle").click();
  assert.equal(app.document.body.classList.contains("picker-list"), true);
  assert.equal(app.$("selected-browser").hidden, true);
  assert.equal(app.$("pending-host").textContent, "ya.ru");
});

test("a new link lays out the chooser with the new presentation revision", async () => {
  const app = chooser();
  await settle();
  assert.equal(app.calls.find((call) => call.command === "set_picker_layout").args.presentationRevision, 1);
  app.setSnapshot({
    presentation_revision: 2,
    pending_url: "https://new-link.invalid/",
    pending_count: 1,
    suggested_pattern: "new-link.invalid",
    picker_style: "switcher",
    browsers: [{ id: "safari", name: "Safari", icon_data_url: null }],
    rules: [],
    error: null,
  });
  await settle();
  const layout = app.calls.filter((call) => call.command === "set_picker_layout").at(-1).args;
  assert.equal(layout.presentationRevision, 2);
  assert.equal(layout.settings, false);
  assert.equal(layout.displayedUrl, "https://new-link.invalid/");
  assert.equal(app.$("pending-host").textContent, "new-link.invalid");
});

test("hidden-window layout does not wait for animation frames", async () => {
  const app = chooser({ framesEnabled: false });
  await settle();
  const layout = app.calls.find((call) => call.command === "set_picker_layout");
  assert.equal(layout.args.presentationRevision, 1);
  assert.equal(layout.args.settings, false);
});

test("opening Preferences refreshes its presentation revision before layout", async () => {
  const app = chooser();
  await settle();
  app.setSnapshot({
    presentation_revision: 2,
    pending_url: null,
    pending_count: 0,
    suggested_pattern: null,
    picker_style: "switcher",
    browsers: [{ id: "safari", name: "Safari", icon_data_url: null }],
    rules: [],
    error: null,
  }, "browsefork-open-preferences");
  await settle();
  const layout = app.calls.filter((call) => call.command === "set_picker_layout").at(-1).args;
  assert.equal(layout.presentationRevision, 2);
  assert.equal(layout.settings, true);
  assert.equal(app.$("settings-view").hidden, false);
});

test("choosing a browser sends the current URL and requested rule", async () => {
  const app = chooser();
  await settle();
  app.$("save-choice").click();
  app.$("save-mode").value = "regex";
  app.$("save-pattern").value = "^https://ya\\.ru/private/";
  app.$("browser-list").querySelectorAll("button")[1].click();
  await settle();
  assert.deepEqual(
    JSON.parse(JSON.stringify(app.calls.find((call) => call.command === "choose_browser").args)),
    {
      browserId: "safari",
      expectedUrl: "https://ya.ru/private/page",
      saveRule: { source_app_id: null, mode: "regex", pattern: "^https://ya\\.ru/private/" },
    }
  );
});

test("Escape skips the pending link without opening a browser", async () => {
  const app = chooser();
  await settle();
  assert.equal(app.key("Escape"), true);
  await settle();
  assert.deepEqual(
    JSON.parse(JSON.stringify(app.calls.find((call) => call.command === "dismiss_window").args)),
    { expectedUrl: "https://ya.ru/private/page", expectedRevision: 1 }
  );
  assert.equal(app.calls.some((call) => call.command === "choose_browser"), false);
});

test("losing focus dismisses the chooser without opening its link", async () => {
  const app = chooser();
  await settle();
  app.inactive();
  await settle();
  assert.equal(app.calls.find((call) => call.command === "dismiss_window").args.expectedUrl,
    "https://ya.ru/private/page");
  assert.equal(app.calls.some((call) => call.command === "choose_browser"), false);
});

test("losing focus in Preferences keeps the pending link", async () => {
  const app = chooser();
  await settle();
  app.$("switcher-settings").click();
  app.inactive();
  await settle();
  assert.equal(app.calls.find((call) => call.command === "dismiss_window").args.expectedUrl, null);
  assert.equal(app.$("chooser-view").hidden, false);
});

test("late focus loss cannot dismiss a newly arrived link", async () => {
  const app = chooser();
  await settle();
  const oldWindow = { expected_revision: 1, expected_url: "https://ya.ru/private/page" };
  app.setSnapshot({
    presentation_revision: 2,
    pending_url: "https://new-link.invalid/",
    pending_count: 2,
    suggested_pattern: "new-link.invalid",
    picker_style: "switcher",
    browsers: [{ id: "safari", name: "Safari", icon_data_url: null }],
    rules: [],
    error: null,
  });
  await settle();
  app.inactive(oldWindow);
  await settle();
  const dismissal = app.calls.find((call) => call.command === "dismiss_window");
  assert.equal(dismissal.args.expectedUrl, "https://ya.ru/private/page");
  assert.equal(dismissal.args.expectedRevision, 1);
  assert.equal(app.$("pending-host").textContent, "new-link.invalid");
  assert.equal(app.$("chooser-view").hidden, false);
});

test("focus loss during browser launch does not trigger dismissal", async () => {
  const app = chooser();
  await settle();
  app.$("browser-list").querySelectorAll("button")[0].click();
  app.inactive();
  await settle();
  assert.equal(app.calls.some((call) => call.command === "dismiss_window"), false);
});

test("app-only saved choice includes the detected source app", async () => {
  const app = chooser();
  await settle();
  app.$("save-choice").click();
  app.$("save-source-app").value = "com.tinyspeck.slackmacgap";
  app.$("save-pattern").value = "";
  app.$("browser-list").querySelectorAll("button")[0].click();
  await settle();
  const choice = app.calls.find((call) => call.command === "choose_browser");
  assert.deepEqual(JSON.parse(JSON.stringify(choice.args.saveRule)), {
    source_app_id: "com.tinyspeck.slackmacgap", mode: "host", pattern: ""
  });
});

test("both Any conditions are rejected before saving", async () => {
  const app = chooser();
  await settle();
  app.$("save-choice").click();
  app.$("save-pattern").value = "";
  app.$("browser-list").querySelectorAll("button")[0].click();
  await settle();
  assert.equal(app.calls.some((call) => call.command === "choose_browser"), false);
  assert.match(app.$("message").textContent, /source app or enter a URL pattern/);

  app.$("switcher-settings").click();
  app.$("rule-source-app").value = "";
  app.$("rule-pattern").value = "";
  app.$("rule-browser").value = "brave";
  app.$("rule-form").dispatch("submit", { preventDefault() {} });
  await settle();
  assert.equal(app.calls.some((call) => call.command === "replace_rules"), false);
});

test("a new local HTML link resets the previous choice and proposes a folder rule", async () => {
  const app = chooser();
  await settle();
  app.$("save-choice").click();
  app.setSnapshot({
    pending_url: "file:///private/tmp/vpn/login.html",
    pending_count: 1,
    suggested_pattern: "file:///private/tmp/vpn/",
    browsers: [{ id: "safari", name: "Safari", icon_data_url: null }],
    rules: [],
    error: null,
  });
  await settle();
  assert.equal(app.$("pending-host").textContent, "Local HTML page");
  assert.equal(app.$("save-choice").checked, false);
  assert.equal(app.$("save-rule-fields").hidden, true);
  assert.equal(app.$("save-mode").value, "contains");
  assert.equal(app.$("save-mode").querySelector('option[value="host"]').disabled, true);
  assert.equal(app.$("save-pattern").value, "file:///private/tmp/vpn/");
});
