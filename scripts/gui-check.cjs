const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const { spawn } = require("node:child_process");
const root = path.resolve(__dirname, "..");
const directories = ["ui", "src-tauri/src", "src-tauri/capabilities", "crates/browsefork-core/src", "tests/gui",
  "crates/tauri-plugin-browsefork-android/android/src", "src-tauri/gen/android/app/src/main"];
const files = ["package.json", "package-lock.json", "Cargo.toml", "Cargo.lock", "playwright.config.cjs", "UI_REQUIREMENTS.md", "AGENTS.md",
  "src-tauri/tauri.conf.json", "src-tauri/tauri.android.conf.json", "src-tauri/tauri.macos.conf.json", "src-tauri/tauri.windows.conf.json", "src-tauri/windows/hooks.nsh", "src-tauri/Cargo.toml", "src-tauri/build.rs",
  "tests/ui.test.cjs", "tests/windows-smoke.cjs", "tests/android-smoke.cjs", "tests/android-startup.py", "tests/android-video-frames.swift", "scripts/android.sh", "tests/macos-browser-discovery.test.cjs", "tests/macos-browser-discovery.m", "tests/macos-presentation.m", "tests/macos-rendering.m", "tests/tauri-settings.test.cjs", "tests/tauri-settings-probe.js", "scripts/gui-check.cjs"];
const reportPath = path.join(root, "target/gui-tests/latest.json");
const WATCH_DEBOUNCE_MS = 300;
function inputs() {
  const result = [...files];
  function visit(directory) {
    for (const entry of fs.readdirSync(path.join(root, directory), { withFileTypes: true })) {
      if (entry.name.startsWith(".")) continue;
      const relative = `${directory}/${entry.name}`;
      if (entry.isDirectory()) visit(relative); else if (entry.isFile()) result.push(relative);
    }
  }
  directories.forEach(visit);
  return [...new Set(result)].sort();
}
function fingerprint() {
  const hash = crypto.createHash("sha256");
  for (const file of inputs()) {
    hash.update(file + "\0");
    hash.update(fs.existsSync(path.join(root, file)) ? fs.readFileSync(path.join(root, file)) : "<missing>");
  }
  return hash.digest("hex");
}
function stage(name, executable, args) {
  return new Promise((resolve) => {
    const started = performance.now();
    const env = { ...process.env, PLAYWRIGHT_BROWSERS_PATH: path.join(root, ".gui-tools/browsers") };
    delete env.FORCE_COLOR;
    const child = spawn(executable, args, { cwd: root, env, stdio: ["ignore", "pipe", "pipe"] });
    let output = "";
    child.stdout.on("data", (data) => { output += data; });
    child.stderr.on("data", (data) => { output += data; });
    child.on("error", (error) => { output += error.message; });
    child.on("close", (code) => {
      const result = { name, status: code === 0 ? "passed" : "failed", durationMs: Math.round(performance.now() - started), exitCode: code, output };
      console.log(`${code === 0 ? "PASS" : "FAIL"} ${name} (${(result.durationMs / 1000).toFixed(1)}s)`);
      if (code !== 0) console.error(output);
      resolve(result);
    });
  });
}
async function run(mode = "all") {
  const started = performance.now();
  const sourceHash = fingerprint();
  let stages;
  if (mode === "browser") {
    stages = [await stage("Real WebKit and Chromium GUI", process.execPath, ["node_modules/@playwright/test/cli.js", "test"])];
  } else {
    stages = await Promise.all([
      stage("UI interaction regressions", process.execPath, ["--test", "tests/ui.test.cjs"]),
      stage("Real WebKit and Chromium GUI", process.execPath, ["node_modules/@playwright/test/cli.js", "test"]),
      stage("Rust rules and window regressions", "cargo", ["test", "--workspace", "--quiet"]).then(async (rust) => {
        if (process.platform !== "darwin" || rust.status !== "passed") return rust;
        const native = await stage("Real Tauri Settings event loop", process.execPath, ["--test", "tests/tauri-settings.test.cjs"]);
        return { ...native, name: "Rust and real Tauri Settings event loop", output: rust.output + native.output,
          durationMs: rust.durationMs + native.durationMs };
      }),
      process.platform === "darwin"
        ? stage("Native macOS rendering and presentation", process.execPath, ["--test", "tests/macos-browser-discovery.test.cjs"])
        : Promise.resolve({ name: "Native macOS rendering and presentation", status: "not_applicable", output: "macOS only" }),
    ]);
  }
  const unchanged = sourceHash === fingerprint();
  const passed = unchanged && stages.every((item) => item.status === "passed" || item.status === "not_applicable");
  const report = { status: passed ? "passed" : unchanged ? "failed" : "stale", fullSuite: mode === "all", sourceHash,
    completedAt: new Date().toISOString(), durationMs: Math.round(performance.now() - started),
    nativePixelVerification: process.platform === "darwin"
      ? "Real Tauri Settings deletion/concurrent native callbacks and offscreen AppKit button bitmaps passed. Native factory/activation changes also require an installed-app check."
      : process.platform === "win32"
        ? "Windows registry, icon and ownership tests are included in the Rust stage. Installed-window appearance and transitions still require a live Windows check."
        : "macOS native rendering not applicable on this host; verify it on macOS before claiming native UI fixes.", stages };
  fs.mkdirSync(path.dirname(reportPath), { recursive: true });
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
  console.log(`${passed ? "PASS" : "FAIL"} GUI gate: ${report.status}, ${(report.durationMs / 1000).toFixed(1)}s. Report: target/gui-tests/latest.json`);
  if (!unchanged) console.error("GUI sources changed during testing. Rerun the gate.");
  return passed;
}
function status() {
  if (!fs.existsSync(reportPath)) throw new Error("No GUI test report. Run npm run test:gui.");
  const report = JSON.parse(fs.readFileSync(reportPath, "utf8"));
  if (report.status !== "passed" || !report.fullSuite || report.sourceHash !== fingerprint())
    throw new Error("GUI gate is missing, failed, partial, or stale. Run npm run test:gui after the final GUI edit.");
  console.log(`PASS: GUI tests cover the current sources (${(report.durationMs / 1000).toFixed(1)}s, ${report.completedAt}).`);
  console.log(report.nativePixelVerification);
}
async function watch() {
  let running = false, requested = false, timer;
  async function schedule() {
    if (running) { requested = true; return; }
    running = true;
    do { requested = false; await run(); } while (requested);
    running = false;
  }
  function changed() { clearTimeout(timer); timer = setTimeout(schedule, WATCH_DEBOUNCE_MS); }
  const watchers = directories.map((dir) => fs.watch(path.join(root, dir), { recursive: true }, changed));
  watchers.push(fs.watch(root, (_, file) => { if (file && files.includes(String(file))) changed(); }));
  for (const dir of ["src-tauri", "tests", "scripts"]) watchers.push(fs.watch(path.join(root, dir), (_, file) => {
    if (file && files.includes(`${dir}/${file}`)) changed();
  }));
  process.on("SIGINT", () => { watchers.forEach((watcher) => watcher.close()); process.exit(0); });
  console.log("Watching GUI sources; runs are serialized and changes are debounced.");
  await schedule();
}
(async () => {
  const mode = process.argv[2] || "all";
  if (mode === "install") {
    const webkit = await stage("Install WebKit", process.execPath, ["node_modules/@playwright/test/cli.js", "install", "webkit"]);
    const chromium = await stage("Install Chromium", process.execPath, ["node_modules/@playwright/test/cli.js", "install", "chromium", "--only-shell"]);
    process.exitCode = webkit.status === "passed" && chromium.status === "passed" ? 0 : 1;
  } else if (mode === "status") status();
  else if (mode === "watch") await watch();
  else process.exitCode = await run(mode) ? 0 : 1;
})().catch((error) => { console.error(error.message); process.exitCode = 1; });
