const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function appleScript(script) {
  return execFileSync("osascript", ["-e", script], { encoding: "utf8" }).trim();
}

function visibleWindow() {
  try {
    return appleScript('tell application "System Events" to tell process "BrowseFork" to get value of attribute "AXVisible" of window 1') === "true";
  } catch {
    return false;
  }
}

function windowContains(text) {
  try {
    return appleScript('tell application "System Events" to tell process "BrowseFork" to get value of every static text of entire contents of window 1').includes(text);
  } catch {
    return false;
  }
}

test("macOS receives an unmatched link and Escape hides the chooser", { skip: process.platform !== "darwin" }, async () => {
  const app = process.env.BROWSEFORK_APP || "/Applications/BrowseFork.app";
  assert.ok(fs.existsSync(path.join(app, "Contents/Info.plist")), `App not found: ${app}`);

  try {
    appleScript('tell application "System Events" to get name of every process whose name is "BrowseFork"');
  } catch (error) {
    throw new Error(`System Events is unavailable in this session. Run from a logged-in desktop Terminal with Accessibility access. ${error.message}`);
  }

  const url = `https://browsefork-smoke-${process.pid}-${Date.now()}.invalid/`;
  const host = new URL(url).hostname;
  execFileSync("open", ["-a", app, url]);
  let delivered = false;
  for (let attempt = 0; attempt < 40; attempt += 1) {
    if (visibleWindow() && windowContains(host)) {
      delivered = true;
      break;
    }
    await wait(250);
  }
  assert.ok(delivered, `BrowseFork did not display ${host} in its chooser within 10 seconds`);

  appleScript('tell application "System Events" to tell process "BrowseFork" to key code 53');
  let hidden = false;
  for (let attempt = 0; attempt < 20; attempt += 1) {
    if (!visibleWindow()) {
      hidden = true;
      break;
    }
    await wait(250);
  }
  assert.ok(hidden, "Escape did not hide the chooser within 5 seconds");
});
