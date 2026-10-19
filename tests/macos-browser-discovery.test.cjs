const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const crypto = require("node:crypto");
const test = require("node:test");
const root = path.resolve(__dirname, "..");
const compilerFlags = ["-fobjc-arc", "-framework", "AppKit", "-framework", "QuartzCore", "-framework", "WebKit", "-framework", "Carbon"];
function runNative(source, expectedOutput) {
  const compiler = spawnSync("clang", ["--version"], { encoding: "utf8", timeout: 10000 });
  assert.equal(compiler.status, 0, compiler.stderr || compiler.error?.message || "clang unavailable");
  const sdk = spawnSync("xcrun", ["--show-sdk-path"], { encoding: "utf8", timeout: 10000 });
  assert.equal(sdk.status, 0, sdk.stderr || "macOS SDK unavailable");
  const hash = crypto.createHash("sha256")
    .update(fs.readFileSync(path.join(__dirname, source)))
    .update(fs.readFileSync(path.join(root, "src-tauri/src/platform/macos.m")))
    .update(JSON.stringify([compilerFlags, compiler.stdout, sdk.stdout, os.release(), process.arch, process.env.SDKROOT || "", process.env.DEVELOPER_DIR || ""]))
    .digest("hex");
  const directory = path.join(root, "target/gui-tests/native", hash);
  const executable = path.join(directory, "probe");
  if (!fs.existsSync(executable)) {
    fs.mkdirSync(directory, { recursive: true });
    const temporary = `${executable}.${process.pid}.tmp`;
    const compile = spawnSync("clang", [...compilerFlags, path.join(__dirname, source), "-o", temporary], { encoding: "utf8", timeout: 30000 });
    assert.equal(compile.status, 0, compile.stderr || compile.error?.message || "Native probe compilation failed");
    fs.renameSync(temporary, executable);
  }
  const result = spawnSync(executable, [], { encoding: "utf8", timeout: 10000 });
  assert.equal(result.status, 0, result.stderr || result.error?.message || "Native probe failed");
  assert.match(result.stdout, expectedOutput);
}
test("macOS browser discovery deduplicates update copies by bundle ID", { skip: process.platform !== "darwin" }, () => {
  runNative("macos-browser-discovery.m", /9 browser discovery regression cases passed[\s\S]*2 native profile launch regression cases passed/);
});
test("macOS reveal and Settings repaint cancel stale callbacks without stealing focus", { skip: process.platform !== "darwin" }, () => {
  runNative("macos-presentation.m", /8 native presentation regression cases passed/);
});
test("real AppKit Settings button pixels survive a real WebKit rule update", { skip: process.platform !== "darwin" }, () => {
  runNative("macos-rendering.m", /Real AppKit title-bar pixels passed before and after a WebKit rule update/);
});
