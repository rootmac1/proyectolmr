const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const { validatePackage, validateManifest } = require("./validate-module-package");

const sampleRoot = path.join(__dirname, "..", "examples", "community");

test("the community example satisfies the module package contract", () => {
  assert.deepEqual(validatePackage(sampleRoot), []);
});

test("rejects undeclared exports and invalid versions", () => {
  const manifest = {
    schema_version: "1.0.0",
    package: {
      name: "Greeter",
      version: "latest",
      summary: "test",
      license: "MIT",
      minimum_lmr_version: "0.1.0",
      platforms: ["all"]
    },
    files: ["Greeter.lmr"],
    dependencies: [],
    exports: [{kind: "function", name: "missing", signature: "missing(): int"}],
    native_bindings: []
  };
  const errors = validateManifest(sampleRoot, manifest);
  assert.ok(errors.some((error) => error.includes("package.version")));
  assert.ok(errors.some((error) => error.includes("exportación missing")));
});

test("rejects module file paths that escape the package", () => {
  const manifest = {
    schema_version: "1.0.0",
    package: {
      name: "Greeter",
      version: "0.1.0",
      summary: "test",
      license: "MIT",
      minimum_lmr_version: "0.1.0",
      platforms: ["all"]
    },
    files: ["../Greeter.lmr"],
    dependencies: [],
    exports: [],
    native_bindings: []
  };
  const errors = validateManifest(sampleRoot, manifest);
  assert.ok(errors.some((error) => error.includes("ruta de módulo sale del paquete")));
});

test("rejects duplicate exports and malformed manifest roots", () => {
  const manifest = {
    schema_version: "1.0.0",
    package: {
      name: "Greeter",
      version: "0.1.0",
      summary: "test",
      license: "MIT",
      minimum_lmr_version: "0.1.0",
      platforms: ["all"]
    },
    files: ["Greeter.lmr"],
    dependencies: [],
    exports: [
      {kind: "function", name: "greet", signature: "greet(string): string"},
      {kind: "function", name: "greet", signature: "greet(string): string"}
    ],
    native_bindings: []
  };
  assert.ok(validateManifest(sampleRoot, manifest).some((error) => error.includes("exports no debe repetir")));
  assert.deepEqual(validateManifest(sampleRoot, null), ["la raíz del manifiesto debe ser un objeto JSON"]);
});

test("rejects duplicate platforms and malformed repository URIs", () => {
  const manifest = {
    schema_version: "1.0.0",
    package: {
      name: "Greeter",
      version: "0.1.0",
      summary: "test",
      license: "MIT",
      minimum_lmr_version: "0.1.0",
      platforms: ["windows", "windows"],
      repository: "relative/repository"
    },
    files: ["Greeter.lmr"],
    dependencies: [],
    exports: [],
    native_bindings: []
  };
  const errors = validateManifest(sampleRoot, manifest);
  assert.ok(errors.some((error) => error.includes("no debe repetir plataformas")));
  assert.ok(errors.some((error) => error.includes("URI absoluta válida")));
});
