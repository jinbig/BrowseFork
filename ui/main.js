const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
let snapshot = null;
let showSettings = false;
let editingId = null;
let busy = false;
let displayedUrl = null;

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

function renderBrowsers() {
  const list = $("browser-list");
  list.replaceChildren();
  if (snapshot.browsers.length === 0) {
    list.append(node("p", "empty-state", "No browsers were found. Install a browser and try again."));
    return;
  }
  for (const browser of snapshot.browsers) {
    const button = node("button", "browser-button");
    button.type = "button";
    button.title = browser.name;
    if (browser.icon_data_url) {
      const icon = node("img", "browser-icon");
      icon.src = browser.icon_data_url;
      icon.alt = "";
      button.append(icon);
    } else {
      button.append(node("span", "browser-icon-fallback", browser.name.slice(0, 1)));
    }
    button.append(node("span", "browser-name", browser.name));
    button.addEventListener("click", () => choose(browser.id));
    list.append(button);
  }
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
      node("div", "rule-pattern", rule.pattern),
      node("div", "rule-meta", `${rule.mode} → ${browserName(rule.browser_id)}${rule.enabled ? "" : " · off"}`)
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
  const newLink = hasPending && snapshot.pending_url !== displayedUrl;
  document.body.classList.toggle("chooser-mode", !settingsVisible);
  $("chooser-view").hidden = settingsVisible;
  $("settings-view").hidden = !settingsVisible;
  $("settings-toggle").textContent = settingsVisible && hasPending ? "Back to link" : "Preferences";
  $("settings-toggle").hidden = !hasPending;
  if (snapshot.error) message(snapshot.error);

  if (hasPending) {
    if (newLink) {
      displayedUrl = snapshot.pending_url;
      document.body.classList.remove("keyboard-navigation");
      if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
      $("save-choice").checked = false;
      $("save-rule-fields").hidden = true;
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
    document.body.classList.remove("keyboard-navigation");
  }
  renderRuleBrowserOptions();
  renderRules();
  updateLayout();
}

function updateLayout() {
  requestAnimationFrame(() => {
    const settings = showSettings || !snapshot?.pending_url;
    invoke("set_picker_layout", {
      settings,
      contentHeight: settings ? null : Math.ceil($("shell").scrollHeight)
    }).catch((error) => message(String(error)));
  });
}

async function choose(browserId) {
  if (busy) return;
  busy = true;
  const saveRule = $("save-choice").checked
    ? { mode: $("save-mode").value, pattern: $("save-pattern").value }
    : null;
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

$("settings-toggle").addEventListener("click", () => {
  showSettings = !showSettings;
  message("");
  render();
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
  if (event.key !== "Tab") return;
  const stops = [
    ...browserButtons,
    ...($("save-card").hidden ? [] : [$("save-choice")]),
    ...($("save-card").hidden || $("save-rule-fields").hidden ? [] : [$("save-mode"), $("save-pattern")]),
    $("skip-link"),
    $("settings-toggle")
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
    mode: $("rule-mode").value,
    pattern: $("rule-pattern").value,
    browser_id: $("rule-browser").value,
    enabled: true
  };
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
