// Explicit Windows-only integration test. Never reads or resets production rules,
// defaults or waiting links. Requires a gui-tests build and an unlocked desktop.
const assert = require("node:assert/strict");
const { test } = require("node:test");
const { spawn, execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const net = require("node:net");
const { chromium } = require("@playwright/test");

const root = path.resolve(__dirname, "..");
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(predicate, label, timeout = 15000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const value = await predicate();
    if (value) return value;
    await pause(75);
  }
  throw new Error(`Timed out: ${label}`);
}
function reg(args) { return execFileSync("reg.exe", args, { encoding: "utf8", windowsHide: true }); }
function writeKey(key, name, value) { reg(["add", key, name ? "/v" : "/ve", ...(name ? [name] : []), "/t", "REG_SZ", "/d", value, "/f"]); }

function nativeWindows(pid, output) {
  // Capture actual native bounds/styles and pixels; CDP alone cannot prove a
  // top-level window is visible, active, or using its native title bar.
  const script = `
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Collections.Generic;
using System.Text;
public class BrowseForkWindowProbe {
  public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr data);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr hwnd, uint command);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int size);
  [DllImport("user32.dll", EntryPoint="GetWindowLongW")] public static extern uint GetWindowLong(IntPtr hwnd, int index);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hwnd, out Rect rect);
  [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
  public struct Rect { public int Left,Top,Right,Bottom; }
  public static List<IntPtr> Visible(uint expected) {
    var result = new List<IntPtr>();
    EnumWindows((hwnd,data) => { uint pid; GetWindowThreadProcessId(hwnd,out pid); if(pid==expected && IsWindowVisible(hwnd)) result.Add(hwnd); return true; }, IntPtr.Zero);
    return result;
  }
}
'@
[void][BrowseForkWindowProbe]::SetThreadDpiAwarenessContext([IntPtr]::new(-4))
$items = @()
foreach ($hwnd in [BrowseForkWindowProbe]::Visible(${pid})) {
  $title = New-Object System.Text.StringBuilder 256
  [void][BrowseForkWindowProbe]::GetWindowText($hwnd,$title,256)
  $rect = New-Object BrowseForkWindowProbe+Rect
  [void][BrowseForkWindowProbe]::GetWindowRect($hwnd,[ref]$rect)
  $client = New-Object BrowseForkWindowProbe+Rect
  [void][BrowseForkWindowProbe]::GetClientRect($hwnd,[ref]$client)
  $width = $rect.Right-$rect.Left; $height = $rect.Bottom-$rect.Top
  if ($width -le 0 -or $height -le 0) { continue }
  $owner = [BrowseForkWindowProbe]::GetWindow($hwnd,4)
  $items += @{title=$title.ToString(); style=[uint32][BrowseForkWindowProbe]::GetWindowLong($hwnd,-16); active=($hwnd -eq [BrowseForkWindowProbe]::GetForegroundWindow()); hiddenOwner=($owner -ne [IntPtr]::Zero -and -not [BrowseForkWindowProbe]::IsWindowVisible($owner)); width=$width; height=$height; clientWidth=($client.Right-$client.Left); clientHeight=($client.Bottom-$client.Top)}
  $bitmap = New-Object System.Drawing.Bitmap $width,$height
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  try { $graphics.CopyFromScreen($rect.Left,$rect.Top,0,0,$bitmap.Size); $bitmap.Save('${output.replaceAll("'", "''")}',[System.Drawing.Imaging.ImageFormat]::Png) }
  finally { $graphics.Dispose(); $bitmap.Dispose() }
}
ConvertTo-Json -InputObject @($items) -Compress
`;
  return JSON.parse(execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", "-"], {
    input: script, encoding: "utf8", windowsHide: true, timeout: 15000,
  }).trim());
}

test("real Windows app: native windows, repeated links, rules, literal forwarding and cancellation", {
  skip: process.platform !== "win32" ? "Requires Windows and an unlocked desktop; a Mac/WebKit pass is not a Windows pass" : false,
  timeout: 120000,
}, async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "browsefork-windows-smoke-"));
  const output = path.join(root, "target/windows/gui"); fs.mkdirSync(output, { recursive: true });
  const executable = path.resolve(process.env.BROWSEFORK_WINDOWS_EXE || path.join(root, "target/debug/browsefork.exe"));
  const fixture = path.join(path.dirname(executable), "windows-test-browser.exe");
  assert.ok(fs.existsSync(executable) && fs.existsSync(fixture), "Build cargo build -p browsefork --features gui-tests --bins first");
  const capture = path.join(directory, "captured-arguments.json");
  const registration = `BrowseFork.Test.${process.pid}`;
  const clientKey = `HKCU\\Software\\Clients\\StartMenuInternet\\${registration}`;
  const classKey = `HKCU\\Software\\Classes\\${registration}`;
  const server = net.createServer();
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  const port = server.address().port; await new Promise(resolve => server.close(resolve));
  const env = { ...process.env, BROWSEFORK_WINDOWS_TEST_DIR: directory, BROWSEFORK_WINDOWS_CAPTURE: capture,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}` };
  let child, browser;
  let logs = "";
  function launch(url) {
    const request = spawn(executable, [url], { env, windowsHide: true, stdio: "ignore" });
    request.on("error", error => { logs += String(error); });
    return request;
  }
  try {
    writeKey(clientKey, "", "BrowseFork capture-only test browser");
    writeKey(`${clientKey}\\Capabilities`, "ApplicationName", "BrowseFork capture-only test browser");
    writeKey(`${clientKey}\\Capabilities\\URLAssociations`, "http", registration);
    writeKey(`${clientKey}\\Capabilities\\URLAssociations`, "https", registration);
    writeKey(`${classKey}\\shell\\open\\command`, "", `"${fixture}" "%1"`);
    child = spawn(executable, [], { env, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
    child.stdout.on("data", value => { logs += value; }); child.stderr.on("data", value => { logs += value; });
    child.on("error", error => { logs += String(error); });
    await until(async () => { try { return (await fetch(`http://127.0.0.1:${port}/json/version`)).ok; } catch { return false; } }, "WebView2 remote debugging");
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
    const pages = () => browser.contexts().flatMap(context => context.pages());
    const settings = await until(() => pages().find(page => page.url().includes("view=settings")), "dedicated Settings WebView");
    const chooser = await until(() => pages().find(page => page.url().includes("view=chooser")), "dedicated chooser WebView");
    await settings.locator("#settings-view").waitFor({ state: "visible" });
    const windows = await until(() => {
      const windows = nativeWindows(child.pid, path.join(output, "settings.png"));
      return windows.find(window => window.title === "BrowseFork Settings") ? windows : null;
    }, "visible native Settings window");
    const frame = windows.find(window => window.title === "BrowseFork Settings");
    assert.equal(frame.hiddenOwner, true, "Settings stays off the taskbar with normal caption controls");
    assert.equal((frame.style & 0x00c00000), 0x00c00000, "Settings has its permanent native caption");
    assert.ok(frame.style & 0x00080000, "Settings has its system menu/close control");
    assert.ok(frame.style & 0x00020000, "Settings has its minimize control");
    assert.ok(frame.style & 0x00010000, "Settings has its maximize control");
    const url = "https://windows-smoke.invalid/auth?q=a%2Bb&literal=pipe|value&placeholder=%1#fragment";
    launch(url);
    await until(async () => await chooser.locator("#pending-host").textContent() === "windows-smoke.invalid", "incoming URL reaches chooser");
    await until(() => nativeWindows(child.pid, path.join(output, "chooser.png")).find(window => window.title === "BrowseFork" && window.active), "active chooser");
    const nativeChooser = nativeWindows(child.pid, path.join(output, "chooser.png")).find(window => window.title === "BrowseFork");
    assert.equal(nativeChooser.style & 0x00c00000, 0, "chooser is frameless");
    assert.equal(nativeChooser.hiddenOwner, true, "chooser stays off the taskbar from first reveal");
    assert.ok(Math.abs(nativeChooser.width - nativeChooser.clientWidth) <= 1,
      "chooser has no native shadow frame/white side gutters");
    assert.ok(Math.abs(nativeChooser.height - nativeChooser.clientHeight) <= 1,
      "chooser has no native shadow frame/white bottom gutter");
    const viewport = await chooser.evaluate(() => ({ width: innerWidth, height: innerHeight, scale: devicePixelRatio }));
    assert.ok(Math.abs(nativeChooser.clientWidth - viewport.width * viewport.scale) <= 1,
      "WebView fills the native client width at the current Windows DPI");
    assert.ok(Math.abs(nativeChooser.clientHeight - viewport.height * viewport.scale) <= 1,
      "WebView fills the native client height at the current Windows DPI");
    await chooser.getByLabel("Save choice", { exact: true }).check();
    await chooser.getByRole("button", { name: "BrowseFork capture-only test browser", exact: true }).click();
    await until(() => fs.existsSync(capture), "capture-only browser received the link");
    assert.deepEqual(JSON.parse(fs.readFileSync(capture)), [url]);
    await until(() => nativeWindows(child.pid, path.join(output, "after-choice.png")).length === 0, "chooser hides after choice");
    fs.unlinkSync(capture);
    launch(url + "-again");
    await until(() => fs.existsSync(capture), "saved rule routes a repeated link");
    assert.deepEqual(JSON.parse(fs.readFileSync(capture)), [url + "-again"]);
    assert.equal(nativeWindows(child.pid, path.join(output, "silent-rule.png")).length, 0, "a matching rule does not open Settings or the chooser");
    launch("browsefork://preferences");
    await settings.locator("#settings-view").waitFor({ state: "visible" });
    await until(() => nativeWindows(child.pid, path.join(output, "settings-before-edit.png")).some(window => window.title === "BrowseFork Settings"), "Settings reopens");
    await settings.getByRole("button", { name: "Edit rule", exact: true }).click();
    await settings.locator("#rule-pattern").fill("edited-windows.invalid");
    await settings.getByRole("button", { name: "Save rule", exact: true }).click();
    await until(async () => (await settings.locator(".rule-row").textContent()).includes("edited-windows.invalid"), "rule edit persists in Settings");
    await settings.locator("#rule-pattern").fill("delete-windows.invalid");
    await settings.getByRole("button", { name: "Save rule", exact: true }).click();
    await until(async () => await settings.locator(".rule-row").count() === 2, "second rule is added");
    await settings.locator(".rule-row").filter({ hasText: "delete-windows.invalid" }).getByRole("button", { name: "Delete rule", exact: true }).click();
    await until(async () => await settings.locator(".rule-row").count() === 1, "rule deletion completes");
    const afterEdit = nativeWindows(child.pid, path.join(output, "settings-after-edit.png")).find(window => window.title === "BrowseFork Settings");
    assert.ok(afterEdit && afterEdit.hiddenOwner);
    assert.equal(afterEdit.style & 0x00cb0000, 0x00cb0000, "native caption and controls survive rule edits");
    launch("https://unmatched-windows.invalid/older");
    await until(async () => await chooser.locator("#pending-host").textContent() === "unmatched-windows.invalid", "older unmatched URL");
    launch("https://newest-windows.invalid/newer");
    await until(async () => await chooser.locator("#pending-host").textContent() === "newest-windows.invalid", "newest unmatched URL");
    await until(async () => await chooser.locator("#queue-status").textContent() === "1 older link waiting", "older link is retained");
    await chooser.keyboard.press("Escape");
    await until(() => nativeWindows(child.pid, path.join(output, "after-escape.png")).length === 0, "Escape closes without opening a link");
    assert.deepEqual(JSON.parse(fs.readFileSync(capture)), [url + "-again"], "Escape does not launch a browser");
    const saved = JSON.parse(fs.readFileSync(path.join(directory, "settings.json")));
    assert.equal(saved.rules.length, 1);
    const pending = JSON.parse(fs.readFileSync(path.join(directory, "pending-links.json")));
    assert.deepEqual(pending.map(link => link.url), ["https://unmatched-windows.invalid/older"], "older link is preserved on disk after cancellation");
  } finally {
    if (browser) await browser.close().catch(() => {});
    if (child && child.exitCode === null) child.kill();
    for (const key of [clientKey, classKey]) { try { reg(["delete", key, "/f"]); } catch {} }
    fs.writeFileSync(path.join(output, "app.log"), logs);
    // Keep only the test directory path for diagnosis; it contains synthetic URLs.
    fs.writeFileSync(path.join(output, "fixture-path.txt"), directory);
  }
});
