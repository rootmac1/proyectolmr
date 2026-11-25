const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const { validateProject, validateReleaseMetadata } = require("./validate-release-metadata");

const projectRoot = path.join(__dirname, "..");

test("project version is synchronized across release metadata", () => {
  assert.deepEqual(validateProject(projectRoot), []);
});

test("reports stale lockfile and missing release notes", () => {
  const errors = validateReleaseMetadata({
    cargoToml: 'version = "1.2.3"',
    cargoLock: '[[package]]\nname = "lmr"\nversion = "1.2.2"',
    readme: "versión `1.2.3`",
    versioning: "están en `1.2.3`",
    changelog: "## 1.2.2 — previous"
  });
  assert.ok(errors.some((error) => error.includes("Cargo.lock")));
  assert.ok(errors.some((error) => error.includes("CHANGELOG.md")));
});

test("rejects a missing or malformed Cargo version", () => {
  const errors = validateReleaseMetadata({
    cargoToml: 'version = "not-a-version"',
    cargoLock: "",
    readme: "",
    versioning: "",
    changelog: ""
  });
  assert.ok(errors.some((error) => error.includes("SemVer")));
});
