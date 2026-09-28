const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
let snapshot = null;
let showSettings = false;
let editingId = null;
let busy = false;
let displayedUrl = null;
let selectedBrowserId = null;

const pickerStyle = () => snapshot?.picker_style || "switcher";
const shortcutPrefix = navigator.platform?.startsWith("Mac") ? "⌘" : "Ctrl+";

const LIST_PICKER_WIDTH = 440;
const MIN_SWITCHER_WIDTH = 320;
const MAX_PICKER_WIDTH = 760;
// Keep these dimensions in sync with the switcher tile, gap, and horizontal padding in styles.css.
const SWITCHER_TILE_WIDTH = 82;
const SWITCHER_TILE_GAP = 8;
const SWITCHER_HORIZONTAL_INSETS = 40;

function switcherWidth(browserCount) {
  const tilesWidth = browserCount * SWITCHER_TILE_WIDTH;
  const gapsWidth = Math.max(0, browserCount - 1) * SWITCHER_TILE_GAP;
  return Math.min(MAX_PICKER_WIDTH,
    Math.max(MIN_SWITCHER_WIDTH, tilesWidth + gapsWidth + SWITCHER_HORIZONTAL_INSETS));
}

function message(text) {
  $("message").textContent = text || "";
  $("message").hidden = !text;
}

function node(tag, className, text) {
  const element = document.createElement(tag);
  if (className) element.className = className;
  if (text !== undefined) element.textContent = text;
  return element;
}

async function refresh() {
  try {
    snapshot = await invoke("get_snapshot");
    render();
  } catch (error) {
    message(String(error));
  }
}

function browserName(id) {
  return snapshot.browsers.find((browser) => browser.id === id)?.name || "Unavailable browser";
}

function sourceAppName(id) {
  return snapshot.source_apps?.find((app) => app.id === id)?.name || id;
}

function renderSourceApps() {
  const list = $("source-app-options");
  list.replaceChildren();
  for (const app of snapshot.source_apps || []) {
    const option = node("option");
    option.value = app.id;
    option.label = app.name;
    list.append(option);
  }
}

function renderBrowsers() {
  const list = $("browser-list");
  list.replaceChildren();
  $("selected-browser").hidden = pickerStyle() !== "switcher";
  if (snapshot.browsers.length === 0) {
    $("selected-browser").textContent = "";
    list.append(node("p", "empty-state", "No browsers were found. Install a browser and try again."));
    return;
  }
  for (const [index, browser] of snapshot.browsers.entries()) {
    const button = node("button", "browser-button");
    button.type = "button";
    button.title = browser.name;
    button.setAttribute("aria-label", browser.name);
    if (browser.icon_data_url) {
      const icon = node("img", "browser-icon");
      icon.src = browser.icon_data_url;
      icon.alt = "";
      button.append(icon);
    } else {
      button.append(node("span", "browser-icon-fallback", browser.name.slice(0, 1)));
    }
    button.append(node("span", "browser-name", browser.name));
    if (index < 9) button.append(node("span", "browser-shortcut", `${shortcutPrefix}${index + 1}`));
    button.addEventListener("focus", () => selectBrowser(browser.id));
    button.addEventListener("mouseenter", () => selectBrowser(browser.id));
    button.addEventListener("click", () => choose(browser.id));
    list.append(button);
  }
  selectBrowser(snapshot.browsers.some((browser) => browser.id === selectedBrowserId)
    ? selectedBrowserId : snapshot.browsers[0].id);
}

function selectBrowser(id) {
  selectedBrowserId = id;
  $("selected-browser").textContent = browserName(id);
  $("browser-list").querySelectorAll("button").forEach((button, index) => {
    button.classList.toggle("is-selected", snapshot.browsers[index].id === id);
  });
}

function renderRuleBrowserOptions() {
  const select = $("rule-browser");
  const selected = select.value;
  select.replaceChildren();
  for (const browser of snapshot.browsers) {
    const option = node("option", "", browser.name);
    option.value = browser.id;
    select.append(option);
  }
  if (editingId !== null) {
    const rule = snapshot.rules.find((item) => item.id === editingId);
    if (rule && !snapshot.browsers.some((browser) => browser.id === rule.browser_id)) {
      const option = node("option", "", "Unavailable browser");
      option.value = rule.browser_id;
      select.append(option);
    }
  }
  if ([...select.options].some((option) => option.value === selected)) select.value = selected;
}

async function commitRules(rules) {
  try {
    snapshot = await invoke("replace_rules", { rules });
    message("");
    render();
  } catch (error) {
    message(String(error));
  }
}

function renderRules() {
  const list = $("rules-list");
  list.replaceChildren();
  if (snapshot.rules.length === 0) {
    list.append(node("p", "empty-state", "No rules yet. Choose a browser for a link or add one below."));
    return;
  }
  snapshot.rules.forEach((rule, index) => {
    const row = node("div", "rule-row");
    const summary = node("div", "rule-summary");
    summary.append(
      node("div", "rule-pattern", `${rule.source_app_id ? sourceAppName(rule.source_app_id) : "Any app"} · ${rule.pattern || "Any web/HTML URL"}`),
      node("div", "rule-meta", `${rule.pattern ? rule.mode : "all URLs"} → ${browserName(rule.browser_id)}${rule.enabled ? "" : " · off"}`)
    );
    const actions = node("div", "rule-actions");
    const action = (label, title, handler, disabled = false) => {
      const button = node("button", "", label);
      button.type = "button";
      button.title = title;
      button.setAttribute("aria-label", title);
      button.disabled = disabled;
      button.addEventListener("click", handler);
      actions.append(button);
    };
    action("↑", "Move rule up", () => moveRule(index, -1), index === 0);
    action("↓", "Move rule down", () => moveRule(index, 1), index === snapshot.rules.length - 1);
    action(rule.enabled ? "On" : "Off", "Toggle rule", () => commitRules(snapshot.rules.map((item) => item.id === rule.id ? { ...item, enabled: !item.enabled } : item)));
    action("Edit", "Edit rule", () => editRule(rule));
    action("×", "Delete rule", () => commitRules(snapshot.rules.filter((item) => item.id !== rule.id)));
    row.append(summary, actions);
    list.append(row);
  });
}

function moveRule(index, direction) {
  const rules = [...snapshot.rules];
  [rules[index], rules[index + direction]] = [rules[index + direction], rules[index]];
  commitRules(rules);
}

function editRule(rule) {
  editingId = rule.id;
  $("rule-form-title").textContent = "Edit rule";
  $("rule-mode").value = rule.mode;
  $("rule-source-app").value = rule.source_app_id || "";
  $("rule-pattern").value = rule.pattern;
  renderRuleBrowserOptions();
  $("rule-browser").value = rule.browser_id;
  $("cancel-edit").hidden = false;
  $("rule-pattern").focus();
}

function resetRuleForm() {
  editingId = null;
  $("rule-form").reset();
  $("rule-form-title").textContent = "Add a rule";
  $("cancel-edit").hidden = true;
  renderRuleBrowserOptions();
}

function render() {
  if (!snapshot) return;
  const hasPending = Boolean(snapshot.pending_url);
  const settingsVisible = showSettings || !hasPending;
  const switcherVisible = !settingsVisible && pickerStyle() === "switcher";
  const newLink = hasPending && snapshot.pending_url !== displayedUrl;
  document.body.classList.toggle("chooser-mode", !settingsVisible);
  document.body.classList.toggle("picker-switcher", switcherVisible);
  document.body.classList.toggle("picker-list", !settingsVisible && pickerStyle() === "list");
  $("topbar").hidden = switcherVisible;
  $("switcher-settings").hidden = !switcherVisible;
  $("chooser-view").hidden = settingsVisible;
  $("settings-view").hidden = !settingsVisible;
  $("picker-style").value = pickerStyle();
  $("settings-toggle").textContent = settingsVisible && hasPending ? "Back to link" : "Preferences";
  $("settings-toggle").hidden = !hasPending;
  $("keyboard-hint").textContent = pickerStyle() === "switcher"
    ? "Tab / ← → · Enter · Esc" : "Tab to move · Enter to open · Esc to skip";
  if (snapshot.error) message(snapshot.error);

  if (hasPending) {
    if (newLink) {
      displayedUrl = snapshot.pending_url;
      selectedBrowserId = null;
      document.body.classList.remove("keyboard-navigation");
      $("save-choice").checked = false;
      $("save-rule-fields").hidden = true;
      const sourceSelect = $("save-source-app");
      sourceSelect.replaceChildren();
      const anyApp = node("option", "", "Any app");
      anyApp.value = "";
      sourceSelect.append(anyApp);
      if (snapshot.pending_source_app_id) {
        const currentApp = node("option", "", `This app: ${sourceAppName(snapshot.pending_source_app_id)}`);
        currentApp.value = snapshot.pending_source_app_id;
        sourceSelect.append(currentApp);
      }
      sourceSelect.value = "";
      const localFile = snapshot.pending_url.startsWith("file:");
      $("save-mode").querySelector('option[value="host"]').disabled = localFile;
      $("save-mode").value = localFile ? "contains" : "host";
      $("save-pattern").value = snapshot.suggested_pattern || "";
    }
    $("save-card").hidden = !snapshot.suggested_pattern;
    if (snapshot.suggested_pattern) {
      const pending = new URL(snapshot.pending_url);
      $("pending-host").textContent = pending.protocol === "file:" ? "Local HTML page" : pending.host;
    } else {
      $("pending-host").textContent = snapshot.pending_url.startsWith("file:") ||
        snapshot.pending_url.startsWith("/") ? "Local document" : "Other link";
    }
    const olderCount = snapshot.pending_count - 1;
    $("queue-status").textContent = `${olderCount} older ${olderCount === 1 ? "link" : "links"} waiting`;
    $("queue-status").hidden = olderCount === 0;
    renderBrowsers();
  } else {
    displayedUrl = null;
    selectedBrowserId = null;
    document.body.classList.remove("keyboard-navigation");
  }
  renderRuleBrowserOptions();
  renderSourceApps();
  renderRules();
  updateLayout();
}

function updateLayout() {
  requestAnimationFrame(() => {
    const settings = showSettings || !snapshot?.pending_url;
    invoke("set_picker_layout", {
      settings,
      contentHeight: settings ? null : Math.ceil($("shell").scrollHeight),
      contentWidth: settings ? null : pickerStyle() === "switcher"
        ? switcherWidth(snapshot.browsers.length) : LIST_PICKER_WIDTH
    }).catch((error) => message(String(error)));
  });
}

async function choose(browserId) {
  if (busy) return;
  const saveRule = $("save-choice").checked
    ? { source_app_id: $("save-source-app").value || null,
        mode: $("save-mode").value, pattern: $("save-pattern").value.trim() }
    : null;
  if (saveRule && !saveRule.source_app_id && !saveRule.pattern) {
    message("Choose a source app or enter a URL pattern.");
    return;
  }
  busy = true;
  const expectedUrl = snapshot.pending_url;
  try {
    const result = await invoke("choose_browser", { browserId, expectedUrl, saveRule });
    snapshot = result.snapshot;
    $("save-choice").checked = false;
    $("save-rule-fields").hidden = true;
    $("save-mode").value = "host";
    showSettings = false;
    message(result.warning);
    render();
  } catch (error) {
    await refresh();
    message(String(error));
  } finally {
    busy = false;
  }
}

async function skipCurrentLink() {
  if (busy) return;
  busy = true;
  try {
    snapshot = await invoke("skip_link", { expectedUrl: snapshot.pending_url });
    message("");
    render();
  } catch (error) {
    await refresh();
    message(String(error));
  } finally {
    busy = false;
  }
}

$("skip-link").addEventListener("click", skipCurrentLink);

$("save-choice").addEventListener("change", (event) => {
  $("save-rule-fields").hidden = !event.target.checked;
  updateLayout();
});

function toggleSettings() {
  showSettings = !showSettings;
  message("");
  render();
}

$("settings-toggle").addEventListener("click", toggleSettings);
$("switcher-settings").addEventListener("click", toggleSettings);

$("picker-style").addEventListener("change", async (event) => {
  try {
    snapshot = await invoke("set_picker_style", { style: event.target.value });
    message("");
    render();
  } catch (error) {
    $("picker-style").value = pickerStyle();
    message(String(error));
  }
});

document.addEventListener("keydown", (event) => {
  if (!snapshot?.pending_url || $("chooser-view").hidden) return;
  if (event.key === "Escape") {
    event.preventDefault();
    skipCurrentLink();
    return;
  }
  if (event.key === "Enter" && document.activeElement === $("save-choice")) {
    event.preventDefault();
    $("save-choice").click();
    return;
  }
  const browserButtons = [...$("browser-list").querySelectorAll("button")];
  if (pickerStyle() === "switcher" && (event.metaKey || event.ctrlKey) &&
      /^[1-9]$/.test(event.key)) {
    const browser = snapshot.browsers[Number(event.key) - 1];
    if (!browser) return;
    event.preventDefault();
    choose(browser.id);
    return;
  }
  if (pickerStyle() === "switcher" && ["ArrowLeft", "ArrowRight"].includes(event.key) &&
      document.activeElement !== $("save-source-app") && document.activeElement !== $("save-mode") && document.activeElement !== $("save-pattern")) {
    if (!browserButtons.length) return;
    event.preventDefault();
    document.body.classList.add("keyboard-navigation");
    const current = snapshot.browsers.findIndex((browser) => browser.id === selectedBrowserId);
    const direction = event.key === "ArrowRight" ? 1 : -1;
    browserButtons[(current + direction + browserButtons.length) % browserButtons.length].focus();
    return;
  }
  if (pickerStyle() === "switcher" && event.key === "Enter" && selectedBrowserId &&
      ![$("save-source-app"), $("save-mode"), $("save-pattern"), $("skip-link"), $("settings-toggle"), $("switcher-settings")]
        .includes(document.activeElement)) {
    event.preventDefault();
    choose(selectedBrowserId);
    return;
  }
  if (event.key !== "Tab") return;
  const stops = [
    ...browserButtons,
    ...($("save-card").hidden ? [] : [$("save-choice")]),
    ...($("save-card").hidden || $("save-rule-fields").hidden ? [] : [$("save-source-app"), $("save-mode"), $("save-pattern")]),
    $("skip-link"),
    pickerStyle() === "switcher" ? $("switcher-settings") : $("settings-toggle")
  ];
  if (!stops.length) return;
  event.preventDefault();
  document.body.classList.add("keyboard-navigation");
  const index = stops.indexOf(document.activeElement);
  const next = index < 0 ? (event.shiftKey ? stops.length - 1 : 0) :
    (index + (event.shiftKey ? -1 : 1) + stops.length) % stops.length;
  stops[next].focus();
});

$("cancel-edit").addEventListener("click", resetRuleForm);

$("rule-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!$("rule-browser").value) {
    message("Choose an installed browser first.");
    return;
  }
  const values = {
    source_app_id: $("rule-source-app").value.trim() || null,
    mode: $("rule-mode").value,
    pattern: $("rule-pattern").value.trim(),
    browser_id: $("rule-browser").value,
    enabled: true
  };
  if (!values.source_app_id && !values.pattern) {
    message("Choose a source app or enter a URL pattern.");
    return;
  }
  const rules = [...snapshot.rules];
  if (editingId === null) {
    values.id = Math.max(0, ...rules.map((rule) => rule.id)) + 1;
    rules.unshift(values);
  } else {
    const index = rules.findIndex((rule) => rule.id === editingId);
    if (index < 0) return;
    rules[index] = { ...rules[index], ...values };
  }
  try {
    snapshot = await invoke("replace_rules", { rules });
    message("");
    resetRuleForm();
    render();
  } catch (error) {
    message(String(error));
  }
});

listen("browsefork-state-changed", () => {
  showSettings = false;
  refresh();
});
listen("browsefork-open-preferences", () => {
  showSettings = true;
  render();
});
refresh();
