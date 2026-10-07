const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");
const root = path.resolve(__dirname, "..");

test("real Tauri event loop survives concurrent Settings deletion and native callbacks", {
  skip: process.platform !== "darwin", timeout: 180000,
}, () => {
  const build = spawnSync("cargo", ["build", "--quiet", "-p", "browsefork", "--features", "gui-tests,custom-protocol", "--bin", "gui-regression"], {
    cwd: root, encoding: "utf8", timeout: 150000,
  });
  assert.equal(build.status, 0, `Probe build failed: ${build.error || ""}\n${build.stderr}`);
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "browsefork-gui-probe-"));
  try {
    const probe = spawnSync(path.join(root, "target/debug/gui-regression"), [], {
      cwd: root, encoding: "utf8", timeout: 20000,
      env: { ...process.env, BROWSEFORK_GUI_PROBE_DIR: directory },
    });
    assert.equal(probe.status, 0, `Real event-loop probe failed/hung: ${probe.error || ""}\n${probe.stdout}\n${probe.stderr}`);
    assert.match(probe.stdout, /PASS real Tauri Settings IPC/);
    const saved = JSON.parse(fs.readFileSync(path.join(directory, "settings.json"), "utf8"));
    assert.deepEqual(saved.rules, [], "Last deletion must also persist on disk");
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
