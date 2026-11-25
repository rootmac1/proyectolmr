const fs = require("node:fs");
const path = require("node:path");

const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;

function validateReleaseMetadata(files) {
  const errors = [];
  const cargoVersion = files.cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (!cargoVersion || !SEMVER.test(cargoVersion)) errors.push("Cargo.toml no contiene una versión SemVer válida");

  const lockVersion = files.cargoLock.match(/\[\[package\]\]\s*\nname\s*=\s*"lmr"\s*\nversion\s*=\s*"([^"]+)"/)?.[1];
  if (cargoVersion && lockVersion !== cargoVersion) errors.push("la versión de Cargo.lock no coincide con Cargo.toml");
  if (cargoVersion && !files.readme.includes(`versión \`${cargoVersion}\``)) {
    errors.push("README.md no indica la versión actual");
  }
  if (cargoVersion && !files.versioning.includes(`están en \`${cargoVersion}\``)) {
    errors.push("VERSIONING.md no indica la versión actual");
  }
  if (cargoVersion && !files.changelog.split(/\r?\n/).some((line) => line.startsWith(`## ${cargoVersion} —`))) {
    errors.push("CHANGELOG.md no contiene una sección para la versión actual");
  }
  return errors;
}

function validateProject(root) {
  const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");
  return validateReleaseMetadata({
    cargoToml: read("Cargo.toml"),
    cargoLock: read("Cargo.lock"),
    readme: read("README.md"),
    versioning: read("VERSIONING.md"),
    changelog: read("CHANGELOG.md")
  });
}

if (require.main === module) {
  const root = path.resolve(process.argv[2] || path.join(__dirname, ".."));
  let errors;
  try {
    errors = validateProject(root);
  } catch (error) {
    console.error(`No se pudieron leer los metadatos de publicación: ${error.message}`);
    process.exitCode = 2;
  }
  if (errors) {
    if (errors.length) {
      for (const error of errors) console.error(`❌ ${error}`);
      process.exitCode = 1;
    } else {
      console.log(`✅ Metadatos de publicación sincronizados: ${root}`);
    }
  }
}

module.exports = { validateProject, validateReleaseMetadata };
