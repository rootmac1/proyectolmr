const test = require("node:test");
const assert = require("node:assert/strict");
const { validatePackageFiles } = require("./validate-crate-package.js");

const validFiles = [
  "Cargo.toml",
  "Cargo.toml.orig",
  "LICENSE",
  "README.md",
  "build.rs",
  "src/main.rs",
  "src/trade_bridge.c",
  "modules/manifest.json",
];

test("accepts a minimal public compiler package", () => {
  const result = validatePackageFiles(validFiles);
  assert.deepEqual(result.missing, []);
  assert.deepEqual(result.unexpected, []);
});

test("rejects local databases, binaries, user programs and build output", () => {
  const result = validatePackageFiles([
    ...validFiles,
    "dist/lmr-windows.zip",
    "partidos_setka.db",
    "app_dashboard_gui.exe",
    "reporte_personal.lmr",
    "lmr-vscode/package.json",
  ]);
  assert.deepEqual(result.unexpected, [
    "dist/lmr-windows.zip",
    "partidos_setka.db",
    "app_dashboard_gui.exe",
    "reporte_personal.lmr",
    "lmr-vscode/package.json",
  ]);
});

test("rejects paths that escape the package roots", () => {
  const result = validatePackageFiles([...validFiles, "docs/../secret.txt"]);
  assert.deepEqual(result.unexpected, ["docs/../secret.txt"]);
});

test("requires compiler sources and its runtime module catalog", () => {
  const result = validatePackageFiles(["Cargo.toml", "LICENSE"]);
  assert.deepEqual(result.missing, [
    "Cargo.toml.orig",
    "README.md",
    "build.rs",
    "src/main.rs",
    "src/trade_bridge.c",
    "modules/manifest.json",
  ]);
});
