const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { extractReleaseNotes } = require("./extract-release-notes");

const changelog = fs.readFileSync(path.join(__dirname, "..", "CHANGELOG.md"), "utf8");

test("extracts only the changelog section matching the release tag", () => {
  const notes = extractReleaseNotes(changelog, "v0.1.0-alpha.9");
  assert.ok(notes.startsWith("## 0.1.0-alpha.9 —"));
  assert.ok(notes.includes("paquetes portables"));
  assert.ok(!notes.includes("## 0.1.0-alpha.8"));
});

test("rejects missing tags and changelog sections", () => {
  assert.throws(() => extractReleaseNotes(changelog, "0.1.0-alpha.9"), /comenzar con v/);
  assert.throws(() => extractReleaseNotes(changelog, "v9.9.9"), /sección única/);
});
