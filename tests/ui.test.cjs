const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

const source = fs.readFileSync(path.join(__dirname, "../ui/main.js"), "utf8");

function chooser({ framesEnabled = true, platform = "macos", ready = true, fontsReady = Promise.resolve(), windowRole = null } = {}) {
  const calls = [];
  const listeners = new Map();
  const elements = new Map();
  let snapshot = {
    platform,
    ready,
    presentation_requested: false,
    is_default_browser: false,
    presentation_revision: 1,
    pending_url: "https://ya.ru/private/page",
    pending_source_app_id: "com.tinyspeck.slackmacgap",
    pending_count: 1,
    suggested_pattern: "ya.ru",
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
      if (selector === "input") return this.children.flatMap(child => child.children).filter(child => child.tagName === "INPUT");
      if (selector === "img") return this.children.flatMap((child) => child.children).filter((child) => child.tagName === "IMG");
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

    getBoundingClientRect() { return { width: 320, height: 200 }; }
    decode() { return Promise.resolve(); }
    setAttribute() {}
    reset() {}
  }

  const document = {
    fonts: { ready: fontsReady },
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
    calls.push({ command, args,
      selectionAtCall: command === "set_picker_layout" ? $("selected-browser").textContent : null,
      focusAtCall: command === "set_picker_layout" ? document.activeElement?.title : null });
    if (command === "get_snapshot") return snapshot;
    if (command === "set_view") {
      snapshot = { ...snapshot, presentation_requested: args.settings, presentation_revision: snapshot.presentation_revision + 1 };
      listeners.get(args.settings ? "browsefork-open-preferences" : "browsefork-state-changed")?.();
      return;
    }
    if (command === "dismiss_window") {
      if (args.expectedRevision !== snapshot.presentation_revision) return snapshot;
      snapshot = { ...snapshot, presentation_revision: snapshot.presentation_revision + 1 };
      if (!args.expectedUrl) return snapshot;
    }
    if (command === "choose_browser" || command === "skip_link" || command === "dismiss_window") {
      snapshot = { ...snapshot, pending_url: null, pending_count: 0, suggested_pattern: null,
        presentation_requested: null };
      return command === "choose_browser" ? { snapshot, warning: null } : snapshot;
    }
    if (command === "replace_rules") {
      snapshot = { ...snapshot, rules: args.rules };
      listeners.get("browsefork-rules-changed")?.();
      return snapshot;
    }
    return undefined;
  };
  const windowHandlers = new Map();
  const context = {
    document,
    navigator: { platform: platform === "windows" ? "Win32" : "MacIntel" },
    window: { addEventListener: (name, callback) => windowHandlers.set(name, callback), location: { href: windowRole ? `tauri://localhost/index.html?view=${windowRole}` : "tauri://localhost/" }, __TAURI__: { core: { invoke }, event: { listen: (name, callback) => listeners.set(name, callback) } } },
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
    getSnapshot() { return snapshot; },
    readyToPresent(revision) { return context.window.browseforkReadyToPresent(revision); },
    resize() { windowHandlers.get("resize")?.(); },
    setSnapshot(value, event = "browsefork-state-changed", push = false) {
      snapshot = value;
      listeners.get(event)({ payload: push ? value : null });
    },
    inactive(payload = { expected_revision: snapshot.presentation_revision,
      expected_url: document.getElementById("chooser-view").hidden ? null : snapshot.pending_url }) {
      listeners.get("browsefork-window-inactive")({ payload });
    },
  };
}

const settle = () => new Promise((resolve) => setImmediate(resolve));

test("Android pushes ready link snapshots without another IPC and ignores older pushes", async () => {
  const app = chooser({ platform: "android" });
  await settle();
  const original = app.getSnapshot();
  const reads = app.calls.filter(call => call.command === "get_snapshot").length;
  const next = { ...original, pending_url: "https://fast-push.invalid/", suggested_pattern: "fast-push.invalid",
    presentation_revision: original.presentation_revision + 1 };
  app.setSnapshot(next, "browsefork-state-changed", true);
  await settle();
  assert.equal(app.$("pending-host").textContent, "fast-push.invalid");
  assert.equal(app.calls.filter(call => call.command === "get_snapshot").length, reads);
  app.setSnapshot(original, "browsefork-state-changed", true);
  await settle();
  assert.equal(app.$("pending-host").textContent, "fast-push.invalid");
});

test("Android waits for its native bootstrap before presenting any screen", async () => {
  const app = chooser({ platform: "android", ready: false });
  await settle();
  assert.equal(app.$("loading").hidden, false);
  assert.equal(app.$("chooser-view").hidden, true);
  assert.equal(app.$("settings-view").hidden, true);
  assert.equal(app.calls.some((call) => call.command === "set_picker_layout"), false);
});

test("Android requests compact content height, hides keyboard hints, and keeps save choice", async () => {
  const app = chooser({ platform: "android" });
  await settle();
  assert.equal(app.document.body.classList.contains("mobile-android"), true);
  assert.equal(app.$("keyboard-hint").hidden, true);
  assert.equal(app.$("save-card").hidden, false);
  const layout = app.calls.find((call) => call.command === "set_picker_layout");
  assert.equal(layout.args.contentWidth, null);
  assert.equal(layout.args.contentHeight, app.$("shell").scrollHeight);
  const buttons = app.$("browser-list").querySelectorAll("button");
  assert.equal(buttons[0].children.some((child) => child.textContent === "Ctrl+1" || child.textContent === "⌘1"), false);
  app.$("save-choice").click();
  app.$("save-pattern").value = "ya.ru";
  buttons[0].click();
  await settle();
  assert.equal(app.calls.find((call) => call.command === "choose_browser").args.saveRule.pattern, "ya.ru");
});

test("Android sizes its shared popup without a native resize IPC loop", async () => {
  const app = chooser({ platform: "android" });
  await settle();
  app.$("shell").scrollHeight = 360;
  app.$("save-choice").click();
  await settle();
  assert.equal(app.calls.filter(call => call.command === "set_picker_layout").at(-1).args.contentHeight, 360);
  app.$("switcher-settings").click();
  await settle();
  const settings = app.calls.filter(call => call.command === "set_picker_layout").at(-1);
  assert.equal(settings.args.settings, true);
  assert.equal(settings.args.contentHeight, null);
  const before = app.calls.length;
  app.resize();
  await settle();
  assert.equal(app.calls.length, before);
});

test("Android Preferences requests the browser role through the native command", async () => {
  const app = chooser({ platform: "android" });
  await settle();
  app.$("switcher-settings").click();
  assert.equal(app.$("android-setup").hidden, false);
  assert.match(app.$("rule-source-app").placeholder, /package ID/);
  app.$("make-default-browser").click();
  await settle();
  assert.equal(app.calls.filter((call) => call.command === "request_default_browser").length, 1);
});

test("Windows uses dedicated Settings, Default Apps setup, and Ctrl browser shortcuts", async () => {
  const app = chooser({ platform: "windows", windowRole: "chooser" });
  await settle();
  assert.equal(app.$("chooser-view").hidden, false);
  assert.equal(app.$("topbar").hidden, true);
  assert.equal(app.$("browser-list").children[0].children.at(-1).textContent, "Ctrl+1");
  app.$("switcher-settings").click();
  await settle();
  assert.equal(app.calls.filter(call => call.command === "set_view").at(-1).args.settings, true);
  assert.equal(app.$("settings-view").hidden, true, "chooser never becomes the Settings document");
  assert.equal(await app.readyToPresent(app.getSnapshot().presentation_revision), false);
  const settings = chooser({ platform: "windows", windowRole: "settings" });
  await settle();
  settings.setSnapshot({ ...settings.getSnapshot(), presentation_requested: true }, "browsefork-open-preferences");
  await settle();
  assert.equal(settings.$("android-setup").hidden, false);
  assert.match(settings.$("rule-source-app").placeholder, /executable path/);
  assert.match(settings.$("setup-note").textContent, /best effort/);
  settings.$("make-default-browser").click();
  await settle();
  assert.ok(settings.calls.some(call => call.command === "request_default_browser"));
});

test("Windows does not request native reveal until fonts and icons are ready", async () => {
  let releaseFonts;
  const app = chooser({ platform: "windows", windowRole: "chooser", fontsReady: new Promise(resolve => { releaseFonts = resolve; }) });
  await settle();
  assert.equal(app.calls.some(call => call.command === "set_picker_layout"), false);
  releaseFonts();
  await settle();
  assert.equal(app.calls.filter(call => call.command === "set_picker_layout").at(-1).focusAtCall, "Brave");
  app.key("2", false, { ctrlKey: true });
  await settle();
  assert.equal(app.calls.find(call => call.command === "choose_browser").args.browserId, "safari");
});

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

test("a browser chosen by click is restored before presenting repeated and different links", async () => {
  const app = chooser({ framesEnabled: false });
  await settle();
  app.$("browser-list").querySelectorAll("button")[1].click();
  await settle();
  for (const url of ["https://ya.ru/private/page", "https://other.invalid/path"]) {
    app.setSnapshot({ ...app.getSnapshot(), presentation_revision: app.getSnapshot().presentation_revision + 1,
      pending_url: url, pending_count: 1, suggested_pattern: new URL(url).host });
    await settle();
    const presentation = app.calls.filter((call) => call.command === "set_picker_layout").at(-1);
    assert.equal(presentation.selectionAtCall, "Safari");
    assert.equal(presentation.focusAtCall, "Safari");
    assert.equal(app.$("browser-list").querySelectorAll("button").filter((button) => button.classList.contains("is-selected")).length, 1);
    app.key("Escape");
    await settle();
  }
});

test("restored selection ignores stationary hover and focus survives a state refresh", async () => {
  const app = chooser();
  await settle();
  app.key("ArrowRight");
  app.key("Escape");
  await settle();
  app.setSnapshot({ ...app.getSnapshot(), presentation_revision: app.getSnapshot().presentation_revision + 1,
    pending_url: "https://ya.ru/private/page", pending_count: 1, suggested_pattern: "ya.ru" });
  await settle();
  let buttons = app.$("browser-list").querySelectorAll("button");
  // Reproduce WebKit's delayed focus restoration after showing the native window.
  buttons[0].focus();
  assert.equal(app.document.activeElement, buttons[1]);
  assert.equal(app.$("selected-browser").textContent, "Safari");
  buttons[0].dispatch("mouseenter");
  buttons[0].dispatch("mousemove", { movementX: 0, movementY: 0, screenX: 100, screenY: 100 });
  assert.equal(app.$("selected-browser").textContent, "Safari");
  app.setSnapshot({ ...app.getSnapshot() }, "browsefork-platform-changed");
  await settle();
  buttons = app.$("browser-list").querySelectorAll("button");
  assert.equal(app.document.activeElement, buttons[1]);
  assert.equal(app.$("selected-browser").textContent, "Safari");
  app.key("Tab");
  assert.equal(app.document.activeElement, buttons[1]);
  app.key("Tab");
  assert.equal(app.document.activeElement, app.$("save-choice"));
  buttons[0].dispatch("mousemove", { movementX: 1, movementY: 0, screenX: 101, screenY: 100 });
  assert.equal(app.$("selected-browser").textContent, "Brave");
});

test("Android restores a browser chosen by shortcut when the same link reopens", async () => {
  const app = chooser({ platform: "android" });
  await settle();
  app.key("2", false, { ctrlKey: true });
  await settle();
  app.setSnapshot({ ...app.getSnapshot(), presentation_requested: false,
    presentation_revision: app.getSnapshot().presentation_revision + 1,
    pending_url: "https://ya.ru/private/page", pending_count: 1, suggested_pattern: "ya.ru" });
  await settle();
  const presentation = app.calls.filter((call) => call.command === "set_picker_layout").at(-1);
  assert.equal(presentation.selectionAtCall, "Safari");
  assert.equal(presentation.focusAtCall, "Safari");
});

test("reopening and refreshing keeps browser tiles instead of recreating their images", async () => {
  const app = chooser();
  await settle();
  const originalButtons = app.$("browser-list").querySelectorAll("button");
  app.key("ArrowRight");
  app.key("Escape");
  await settle();
  app.setSnapshot({ ...app.getSnapshot(), presentation_revision: app.getSnapshot().presentation_revision + 1,
    pending_url: "https://other.invalid/", pending_count: 1, suggested_pattern: "other.invalid" });
  await settle();
  const reopenedButtons = app.$("browser-list").querySelectorAll("button");
  assert.equal(reopenedButtons[0], originalButtons[0]);
  assert.equal(reopenedButtons[1], originalButtons[1]);
  assert.equal(app.$("selected-browser").textContent, "Safari");
  app.setSnapshot({ ...app.getSnapshot() }, "browsefork-platform-changed");
  await settle();
  assert.equal(app.$("browser-list").querySelectorAll("button")[1], originalButtons[1]);
});

test("native presentation waits for assets and cancels a frame when a newer link arrives", async () => {
  let releaseFonts;
  const fontsReady = new Promise((resolve) => { releaseFonts = resolve; });
  const app = chooser({ fontsReady });
  await settle();
  let finished = false;
  const preparation = app.readyToPresent(1).then((value) => { finished = true; return value; });
  await settle();
  assert.equal(finished, false);
  app.setSnapshot({ ...app.getSnapshot(), presentation_revision: 2,
    pending_url: "https://newer.invalid/", suggested_pattern: "newer.invalid" });
  await settle();
  releaseFonts();
  assert.equal(await preparation, false);
  assert.equal(await app.readyToPresent(2), true);
});

test("a removed remembered browser falls back to an available browser before showing", async () => {
  const app = chooser();
  await settle();
  app.key("ArrowRight");
  app.key("Escape");
  await settle();
  app.setSnapshot({ ...app.getSnapshot(), presentation_revision: app.getSnapshot().presentation_revision + 1,
    pending_url: "https://other.invalid/", pending_count: 1, suggested_pattern: "other.invalid",
    browsers: [{ id: "brave", name: "Brave", icon_data_url: null }] });
  await settle();
  const presentation = app.calls.filter((call) => call.command === "set_picker_layout").at(-1);
  assert.equal(presentation.selectionAtCall, "Brave");
  assert.equal(presentation.focusAtCall, "Brave");
  app.key("Enter");
  await settle();
  assert.equal(app.calls.find((call) => call.command === "choose_browser").args.browserId, "brave");
});

test("switcher command-number shortcut opens the matching browser", async () => {
  const app = chooser();
  await settle();
  assert.equal(app.key("2", false, { metaKey: true }), true);
  await settle();
  assert.equal(app.calls.find((call) => call.command === "choose_browser").args.browserId, "safari");
});

test("Settings has no chooser-style option and old list preferences still show the switcher", async () => {
  const html = fs.readFileSync(path.join(__dirname, "../ui/index.html"), "utf8");
  assert.doesNotMatch(html, /picker-style|picker-preference|Browser list/);
  const app = chooser();
  await settle();
  app.setSnapshot({ ...app.getSnapshot(), picker_style: "list" });
  await settle();
  assert.equal(app.document.body.classList.contains("picker-switcher"), true);
  assert.equal(app.document.body.classList.contains("picker-list"), false);
  assert.equal(app.$("selected-browser").hidden, false);
  app.$("switcher-settings").click();
  assert.equal(app.$("settings-view").hidden, false);
  assert.equal(app.$("topbar").hidden, false);
  app.$("settings-toggle").click();
  assert.equal(app.document.body.classList.contains("picker-switcher"), true);
  assert.equal(app.$("topbar").hidden, true);
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

test("saving, toggling and deleting a rule keeps Settings active while a link is waiting", async () => {
  const app = chooser();
  await settle();
  app.$("switcher-settings").click();
  app.$("rule-source-app").value = "";
  app.$("rule-mode").value = "host";
  app.$("rule-pattern").value = "rule-edit.invalid";
  app.$("rule-browser").value = "safari";
  app.$("rule-form").dispatch("submit", { preventDefault() {} });
  await settle();
  const assertSettings = () => {
    assert.equal(app.$("settings-view").hidden, false);
    assert.equal(app.$("chooser-view").hidden, true);
    assert.equal(app.calls.filter((call) => call.command === "set_picker_layout").at(-1).args.settings, true);
    assert.equal(app.getSnapshot().pending_url, "https://ya.ru/private/page");
  };
  assertSettings();
  assert.equal(app.getSnapshot().rules.length, 1);
  app.$("rules-list").children[0].children[1].children[2].click();
  await settle();
  assertSettings();
  assert.equal(app.getSnapshot().rules[0].enabled, false);
  app.$("rules-list").children[0].children[1].children[4].click();
  await settle();
  assertSettings();
  assert.equal(app.getSnapshot().rules.length, 0);
});

test("macOS chooser hands Settings to its dedicated window without changing chooser decorations", async () => {
  const app = chooser({ windowRole: "chooser" });
  await settle();
  app.$("switcher-settings").click();
  await settle();
  assert.equal(app.calls.filter((call) => call.command === "set_view").at(-1).args.settings, true);
  assert.equal(app.$("settings-view").hidden, true);
  assert.equal(await app.readyToPresent(app.getSnapshot().presentation_revision), false);
  app.setSnapshot({ ...app.getSnapshot(), presentation_requested: false, presentation_revision: 3 });
  await settle();
  assert.equal(app.$("chooser-view").hidden, false);
  assert.equal(app.$("settings-view").hidden, true);
});

test("dedicated macOS Settings stays a Settings page across rule saves and returns links to the chooser", async () => {
  const capabilities = JSON.parse(fs.readFileSync(path.join(__dirname, "../src-tauri/capabilities/default.json"), "utf8"));
  assert.ok(capabilities.windows.includes("settings"), "Settings must be allowed to subscribe to app events");
  const app = chooser({ windowRole: "settings" });
  await settle();
  app.setSnapshot({ ...app.getSnapshot(), presentation_requested: true, presentation_revision: 2 }, "browsefork-open-preferences");
  await settle();
  assert.equal(app.$("settings-view").hidden, false);
  assert.equal(app.$("chooser-view").hidden, true);
  app.$("rule-mode").value = "host";
  app.$("rule-pattern").value = "dedicated-settings.invalid";
  app.$("rule-browser").value = "safari";
  app.$("rule-form").dispatch("submit", { preventDefault() {} });
  await settle();
  assert.equal(app.$("settings-view").hidden, false);
  assert.equal(app.calls.filter((call) => call.command === "set_picker_layout").at(-1).args.settings, true);
  app.$("settings-toggle").click();
  await settle();
  assert.equal(app.calls.filter((call) => call.command === "set_view").at(-1).args.settings, false);
  assert.equal(app.$("settings-view").hidden, true);
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

test("legacy Chrome rules stay named and editable when explicit profiles replace its tile", async () => {
  const app = chooser({ windowRole: "settings" });
  app.setSnapshot({
    presentation_requested: true,
    pending_url: null,
    browsers: [{ id: "chrome-profile-a", name: "Chrome · A", icon_data_url: null,
      profile: { name: "A", initials: "A", avatar_data_url: null, browser_id: "chrome", browser_name: "Chrome" } }],
    rules: [{ id: 1, browser_id: "chrome", pattern: "legacy.invalid", mode: "host", enabled: true }],
  });
  await settle();
  const row = app.$("rules-list").children[0];
  assert.match(row.children[0].children[1].textContent, /Chrome \(last used profile\)/);
  row.children[1].children[3].click();
  const legacy = app.$("rule-browser").options.find(option => option.value === "chrome");
  assert.equal(legacy.textContent, "Chrome (last used profile)");
});
