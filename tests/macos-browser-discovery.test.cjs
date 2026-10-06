const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");

test("macOS browser discovery deduplicates update copies by bundle ID", { skip: process.platform !== "darwin" }, () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "browsefork-discovery-"));
  try {
    const executable = path.join(directory, "browser-discovery-test");
    const compile = spawnSync("clang", ["-fobjc-arc", path.join(__dirname, "macos-browser-discovery.m"),
      "-framework", "AppKit", "-framework", "QuartzCore", "-framework", "Carbon", "-o", executable], { encoding: "utf8" });
    assert.equal(compile.status, 0, compile.stderr || compile.error?.message);
    const result = spawnSync(executable, [], { encoding: "utf8", timeout: 10000 });
    assert.equal(result.status, 0, result.stderr || result.error?.message);
    assert.match(result.stdout, /9 browser discovery regression cases passed/);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
