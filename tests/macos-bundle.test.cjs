const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

test("macOS bundle registers web links and local HTML documents", { skip: process.platform !== "darwin" }, () => {
  const bundle = path.join(__dirname, "../target/release/bundle/macos/BrowseFork.app");
  const plist = path.join(bundle, "Contents/Info.plist");
  assert.ok(fs.existsSync(plist), "Build the release app with npm run build first");
  const result = spawnSync("plutil", ["-convert", "json", "-o", "-", plist], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  const info = JSON.parse(result.stdout);
  assert.equal(info.CFBundlePackageType, "APPL");
  assert.equal(info.LSUIElement, true, "BrowseFork should stay out of the Dock");
  const schemes = new Set(info.CFBundleURLTypes.flatMap((entry) => entry.CFBundleURLSchemes));
  for (const scheme of ["http", "https", "browsefork"]) {
    assert.ok(schemes.has(scheme), `Missing ${scheme} URL handler`);
  }
  const extensions = new Set(info.CFBundleDocumentTypes.flatMap((entry) => entry.CFBundleTypeExtensions || []));
  for (const extension of ["html", "htm", "xhtml"]) {
    assert.ok(extensions.has(extension), `Missing ${extension} document handler`);
  }
  assert.ok(fs.existsSync(path.join(bundle, "Contents/MacOS", info.CFBundleExecutable)));
});
