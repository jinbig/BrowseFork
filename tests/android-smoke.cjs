const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const root = path.resolve(__dirname, "..");
const serial = process.env.BROWSEFORK_ANDROID_SERIAL || "emulator-5554";
const appId = "com.browsefork.app";
const browserId = "com.browsefork.testbrowser";
const component = `${appId}/.MainActivity`;
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const quote = (value) => "'" + value.replaceAll("'", "'\\''") + "'";

function adb(...args) {
  const result = spawnSync(path.join(root, "scripts/android.sh"), ["adb", "-s", serial, ...args], {
    cwd: root, encoding: "utf8", timeout: 20000,
  });
  assert.equal(result.status, 0, result.stderr || result.error?.message || `adb exited ${result.status}`);
  return result.stdout.trim();
}

async function until(callback, label, timeout = 20000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    try { const value = await callback(); if (value) return value; }
    catch (error) { last = error; }
    await pause(150);
  }
  throw new Error(`Timed out: ${label}${last ? ` (${last.message})` : ""}`);
}

async function connectWebview() {
  return until(async () => {
    const pid = adb("shell", "pidof", appId).split(" ")[0];
    if (!pid) return null;
    adb("forward", "tcp:9229", `localabstract:webview_devtools_remote_${pid}`);
    const pages = await (await fetch("http://127.0.0.1:9229/json/list")).json();
    const page = pages.find((item) => item.type === "page" && item.url.includes("tauri"));
    if (!page) return null;
    const socket = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((resolve, reject) => {
      socket.addEventListener("open", resolve, { once: true });
      socket.addEventListener("error", reject, { once: true });
    });
    let next = 0;
    const waiting = new Map();
    socket.addEventListener("message", ({ data }) => {
      const response = JSON.parse(data);
      const request = waiting.get(response.id);
      if (request) {
        waiting.delete(response.id);
        clearTimeout(request.timer);
        response.error ? request.reject(new Error(response.error.message)) : request.resolve(response.result);
      }
    });
    const call = (method, params) => new Promise((resolve, reject) => {
      const id = ++next;
      const timer = setTimeout(() => { waiting.delete(id); reject(new Error(`CDP timeout: ${method}`)); }, 15000);
      waiting.set(id, { resolve, reject, timer });
      socket.send(JSON.stringify({ id, method, params }));
    });
    const evaluate = async (expression) => {
      const result = await call("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
      if (result.exceptionDetails) throw new Error(result.exceptionDetails.text);
      return result.result.value;
    };
    return { evaluate, snapshot: () => evaluate("window.__TAURI__.core.invoke('get_snapshot')"), close: () => socket.close() };
  }, "BrowseFork debug WebView");
}

function openLink(url, explicit = false, extra = []) {
  adb("shell", "am", "start", "-a", "android.intent.action.VIEW", "-c", "android.intent.category.BROWSABLE",
    "-d", quote(url), ...(explicit ? ["-n", component] : []), ...extra);
}

function launcher() {
  adb("shell", "am", "start", "-n", component, "-a", "android.intent.action.MAIN", "-c", "android.intent.category.LAUNCHER");
}

test("Android emulator: native links, picker, saved rules, Back, shares, and process restoration", {
  skip: process.env.BROWSEFORK_ANDROID_SMOKE !== "1", timeout: 180000,
}, async (t) => {
  // This suite resets its two test apps and changes the emulator's browser role. Refuse real phones.
  assert.match(serial, /^emulator-\d+$/);
  assert.equal(adb("shell", "getprop", "ro.kernel.qemu"), "1");
  const apk = path.join(root, "src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk");
  const fixture = path.join(root, "tests/android-browser/build/outputs/apk/debug/test-browser-debug.apk");
  assert.ok(fs.existsSync(apk), "Run npm run android:build first");
  assert.ok(fs.existsSync(fixture), "Build :test-browser:assembleDebug first");
  adb("install", "-r", apk);
  adb("install", "-r", fixture);
  adb("shell", "pm", "clear", appId);
  adb("shell", "pm", "clear", browserId);
  launcher();
  let client = await connectWebview();
  t.after(() => { client.close(); adb("forward", "--remove", "tcp:9229"); });
  await until(async () => (await client.snapshot()).ready, "native bootstrap");
  const stage = async (name, callback) => {
    let failure;
    await t.test(name, async () => { try { await callback(); } catch (error) { failure = error; throw error; } });
    if (failure) throw failure;
  };

  await stage("launcher opens Preferences and discovers installed browsers with icons", async () => {
    const state = await client.snapshot();
    assert.equal(state.platform, "android");
    assert.equal(state.presentation_requested, true);
    assert.ok(state.browsers.some((browser) => browser.id === browserId && browser.icon_data_url.startsWith("data:image/png;base64,")));
    assert.ok(state.browsers.every((browser) => browser.id !== appId));
    assert.equal(await client.evaluate("document.getElementById('settings-view').hidden"), false);
  });

  await stage("native default browser request offers BrowseFork and the role can be assigned", async () => {
    await client.evaluate("window.__TAURI__.core.invoke('request_default_browser')");
    const ui = await until(() => {
      adb("shell", "uiautomator", "dump", "/sdcard/browsefork-role.xml");
      const xml = adb("shell", "cat", "/sdcard/browsefork-role.xml");
      return xml.includes("BrowseFork") && xml.includes("Test Browser") && xml;
    }, "system browser role dialog");
    assert.match(ui, /BrowseFork/);
    adb("shell", "input", "keyevent", "4");
    adb("shell", "cmd", "role", "add-role-holder", "android.app.role.BROWSER", appId);
    assert.equal(adb("shell", "cmd", "role", "get-role-holders", "android.app.role.BROWSER"), appId);
  });

  const first = "https://route.invalid/login?state=a%2Bb&token=test#fragment";
  const second = "https://second.invalid/path";
  await stage("cold and repeated warm links reach one chooser; a newer link keeps the older one", async () => {
    client.close();
    adb("shell", "am", "force-stop", appId);
    openLink(first);
    client = await connectWebview();
    await until(async () => (await client.snapshot()).pending_url === first, "cold link");
    const beforeRepeat = (await client.snapshot()).presentation_revision;
    openLink(first);
    await until(async () => {
      const state = await client.snapshot();
      return state.pending_count === 1 && state.presentation_revision > beforeRepeat;
    }, "repeat link");
    openLink(second);
    await until(async () => {
      const state = await client.snapshot();
      return state.pending_url === second && state.pending_count === 2;
    }, "newer link queue");
    adb("shell", "input", "keyevent", "4");
    await until(async () => (await client.snapshot()).pending_url === first, "Back skips only the newest link");
    launcher();
    await until(async () => !(await client.evaluate("document.getElementById('chooser-view').hidden")), "restored older chooser");
  });

  await stage("touch Save choice persists a host rule and forwards the full URL", async () => {
    await client.evaluate("document.getElementById('save-choice').click(); document.querySelector('button[title=\"Test Browser\"]').click()");
    await until(() => adb("shell", "run-as", browserId, "cat", "files/last-url.txt") === first, "chosen browser receives full URL");
    const state = await client.snapshot();
    assert.equal(state.pending_count, 0);
    assert.ok(state.rules.some((rule) => rule.pattern === "route.invalid" && rule.browser_id === browserId));
    openLink("https://route.invalid/automatic");
    await until(() => adb("shell", "run-as", browserId, "cat", "files/last-url.txt") === "https://route.invalid/automatic", "automatic rule dispatch");
    assert.equal((await client.snapshot()).pending_count, 0);
  });

  await stage("other URI schemes delivered directly ask for a browser without saving an invalid rule", async () => {
    const url = "browsefork-test://callback/opaque?state=a%2Bb";
    openLink(url, true);
    await until(async () => (await client.snapshot()).pending_url === url, "custom URI chooser");
    await until(async () => await client.evaluate("document.getElementById('save-card').hidden"), "unsupported rule controls hidden");
    await client.evaluate("document.querySelector('button[title=\"Test Browser\"]').click()");
    await until(() => adb("shell", "run-as", browserId, "cat", "files/last-url.txt") === url, "custom URI forwarding");
  });

  await stage("source app rules apply only when the incoming intent provides that sender", async () => {
    const state = await client.snapshot();
    const rules = [{ id: 100, source_app_id: "com.browsefork.testsource", mode: "host", pattern: "", browser_id: browserId, enabled: true }, ...state.rules];
    await client.evaluate(`window.__TAURI__.core.invoke('replace_rules', { rules: ${JSON.stringify(rules)} })`);
    openLink("https://source.invalid/known", false,
      ["--eu", "android.intent.extra.REFERRER", "android-app://com.browsefork.testsource"]);
    await until(() => adb("shell", "run-as", browserId, "cat", "files/last-url.txt") === "https://source.invalid/known", "source rule routing");
    openLink("https://source.invalid/unknown");
    await until(async () => (await client.snapshot()).pending_url === "https://source.invalid/unknown", "unknown sender falls back to chooser");
    assert.equal((await client.snapshot()).pending_source_app_id, null);
    await client.evaluate("document.getElementById('skip-link').click()");
    await until(async () => (await client.snapshot()).pending_count === 0, "unknown sender skipped");
  });

  await stage("shared text supplies a URL and Android process death preserves the waiting queue", async () => {
    adb("shell", "am", "start", "-n", component, "-a", "android.intent.action.SEND", "-t", "text/plain",
      "--es", "android.intent.extra.TEXT", quote("Shared title https://share.invalid/a"));
    await until(async () => (await client.snapshot()).pending_url === "https://share.invalid/a", "shared URL");
    openLink("https://queue.invalid/newer");
    await until(async () => (await client.snapshot()).pending_count === 2, "two waiting links");
    client.close();
    adb("shell", "am", "force-stop", appId);
    launcher();
    client = await connectWebview();
    const state = await until(async () => {
      const value = await client.snapshot();
      return value.ready && value.pending_count === 2 && value;
    }, "queue restoration");
    assert.equal(state.pending_url, "https://queue.invalid/newer");
    assert.ok(state.rules.some((rule) => rule.pattern === "route.invalid"));
    await client.evaluate("document.getElementById('skip-link').click()");
    await until(async () => (await client.snapshot()).pending_url === "https://share.invalid/a", "skip preserves older share");
  });
});
