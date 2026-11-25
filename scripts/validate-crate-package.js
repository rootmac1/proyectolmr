const { spawnSync } = require("node:child_process");
const path = require("node:path");

const rootFiles = new Set([
  "CHANGELOG.md",
  "CONTRIBUTING.md",
  ".cargo_vcs_info.json",
  "Cargo.lock",
  "Cargo.toml",
  "Cargo.toml.orig",
  "GUIA_LMR_ES.md",
  "LICENSE",
  "README.md",
  "VERSIONING.md",
  "build.rs",
]);

function normalizePackageFiles(files) {
  return files
    .map((file) => file.trim().replaceAll("\\", "/").replace(/^\.\//, ""))
    .filter(Boolean);
}

function isAllowedPackageFile(file) {
  if (rootFiles.has(file)) return true;
  if (file === "assets/brand/lmr.ico") return true;
  if (/^docs\/[^/]+$/.test(file)) return true;
  if (/^examples\/community\/[^/]+$/.test(file)) return true;
  if (/^modules\/(?:[^/]+\.lmr|manifest\.json)$/.test(file)) return true;
  if (/^src\/[^/]+\.(?:rs|c)$/.test(file)) return true;
  return false;
}

function validatePackageFiles(files) {
  const normalized = normalizePackageFiles(files);
  const unexpected = normalized.filter((file) => !isAllowedPackageFile(file));
  const required = [
    "Cargo.toml",
    "Cargo.toml.orig",
    "LICENSE",
    "README.md",
    "build.rs",
    "src/main.rs",
    "src/trade_bridge.c",
    "modules/manifest.json",
  ];
  const missing = required.filter((file) => !normalized.includes(file));

  return { files: normalized, missing, unexpected };
}

function main() {
  const projectRoot = path.resolve(__dirname, "..");
  const result = spawnSync(
    "cargo",
    ["package", "--list", "--locked", "--allow-dirty"],
    { cwd: projectRoot, encoding: "utf8" },
  );

  if (result.error) throw result.error;
  if (result.status !== 0) {
    process.stderr.write(result.stderr || "cargo package --list falló\n");
    process.exitCode = result.status || 1;
    return;
  }

  const report = validatePackageFiles(result.stdout.split(/\r?\n/));
  if (report.missing.length || report.unexpected.length) {
    if (report.missing.length) {
      console.error(`Faltan archivos requeridos del crate: ${report.missing.join(", ")}`);
    }
    if (report.unexpected.length) {
      console.error(`Archivos fuera de la lista pública: ${report.unexpected.join(", ")}`);
    }
    process.exitCode = 1;
    return;
  }

  console.log(`Contenido del crate válido: ${report.files.length} archivos permitidos.`);
}

if (require.main === module) {
  main();
}

module.exports = { normalizePackageFiles, isAllowedPackageFile, validatePackageFiles };
