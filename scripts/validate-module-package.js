const fs = require("node:fs");
const path = require("node:path");

const VERSION = /^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;
const IDENTIFIER = /^[A-Za-z][A-Za-z0-9_]*$/;

function isInsideDirectory(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative !== "" && relative !== ".." &&
    !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative);
}

function validateManifest(root, manifest) {
  const errors = [];
  const fail = (message) => errors.push(message);

  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) {
    return ["la raíz del manifiesto debe ser un objeto JSON"];
  }
  if (manifest.schema_version !== "1.0.0") fail("schema_version debe ser 1.0.0");
  const pkg = manifest.package;
  if (!pkg || typeof pkg !== "object") return ["falta el objeto package"];
  if (!IDENTIFIER.test(pkg.name || "")) fail("package.name debe ser un identificador LMR");
  if (!VERSION.test(pkg.version || "")) fail("package.version debe ser SemVer");
  if (!pkg.summary || typeof pkg.summary !== "string") fail("package.summary es obligatorio");
  if (!pkg.license || typeof pkg.license !== "string") fail("package.license es obligatorio");
  if (!VERSION.test(pkg.minimum_lmr_version || "")) fail("package.minimum_lmr_version debe ser SemVer");
  if (pkg.repository !== undefined) {
    if (typeof pkg.repository !== "string") {
      fail("package.repository debe ser una URI absoluta");
    } else {
      try {
        new URL(pkg.repository);
      } catch {
        fail("package.repository debe ser una URI absoluta válida");
      }
    }
  }
  const supportedPlatforms = new Set(["all", "windows", "linux", "macos"]);
  if (!Array.isArray(pkg.platforms) || pkg.platforms.length === 0 || pkg.platforms.some((platform) => !supportedPlatforms.has(platform))) {
    fail("package.platforms debe listar all, windows, linux o macos");
  }
  if (Array.isArray(pkg.platforms) && new Set(pkg.platforms).size !== pkg.platforms.length) {
    fail("package.platforms no debe repetir plataformas");
  }

  if (!Array.isArray(manifest.files) || manifest.files.length === 0) {
    fail("files debe listar al menos un archivo .lmr");
    return errors;
  }
  const packageRoot = path.resolve(root);
  const realPackageRoot = fs.realpathSync(packageRoot);
  const sourceParts = [];
  const seenFiles = new Set();
  for (const relativeFile of manifest.files) {
    if (typeof relativeFile !== "string" || !relativeFile.endsWith(".lmr") || path.isAbsolute(relativeFile)) {
      fail(`ruta de módulo inválida: ${relativeFile}`);
      continue;
    }
    const absoluteFile = path.resolve(packageRoot, relativeFile);
    if (!isInsideDirectory(packageRoot, absoluteFile)) {
      fail(`ruta de módulo sale del paquete: ${relativeFile}`);
      continue;
    }
    if (!fs.existsSync(absoluteFile) || !fs.statSync(absoluteFile).isFile()) {
      fail(`no existe el archivo de módulo: ${relativeFile}`);
      continue;
    }
    const realFile = fs.realpathSync(absoluteFile);
    if (!isInsideDirectory(realPackageRoot, realFile)) {
      fail(`el archivo de módulo apunta fuera del paquete: ${relativeFile}`);
      continue;
    }
    if (seenFiles.has(realFile)) {
      fail(`archivo de módulo duplicado: ${relativeFile}`);
      continue;
    }
    seenFiles.add(realFile);
    sourceParts.push(fs.readFileSync(absoluteFile, "utf8"));
  }
  const source = sourceParts.join("\n");

  if (!Array.isArray(manifest.exports)) fail("exports debe ser una lista");
  else for (const item of manifest.exports) {
    if (!item || !IDENTIFIER.test(item.name || "")) {
      fail("cada export necesita un nombre válido");
      continue;
    }
    const declaration = item.kind === "function" ? `func\\s+${item.name}\\s*\\(`
      : item.kind === "class" ? `class\\s+${item.name}\\b`
        : item.kind === "record" ? `record\\s+${item.name}\\b`
          : item.kind === "enum" ? `enum\\s+${item.name}\\b` : null;
    if (!declaration || !new RegExp(`^\\s*${declaration}`, "m").test(source)) {
      fail(`no se encontró la exportación ${item.name} en los archivos fuente`);
    }
    if (typeof item.signature !== "string" || item.signature.length === 0) {
      fail(`falta la firma de ${item.name}`);
    }
  }

  if (!Array.isArray(manifest.native_bindings)) fail("native_bindings debe ser una lista");
  else for (const binding of manifest.native_bindings) {
    if (typeof binding !== "string" || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(binding) || !new RegExp(`^\\s*extern\\s+func\\s+${binding}\\s*\\(`, "m").test(source)) {
      fail(`no se encontró la declaración extern de ${binding}`);
    }
  }

  if (!Array.isArray(manifest.dependencies)) fail("dependencies debe ser una lista");
  else for (const dependency of manifest.dependencies) {
    if (!dependency || !IDENTIFIER.test(dependency.name || "") || !VERSION.test(dependency.version || "")) {
      fail("cada dependencia debe tener name y version SemVer");
    }
    if (pkg.name === dependency.name) fail("un paquete no puede depender de sí mismo");
  }
  if (Array.isArray(pkg.platforms) && pkg.platforms.includes("all") && pkg.platforms.length > 1) {
    fail("package.platforms no debe combinar all con plataformas específicas");
  }
  if (Array.isArray(manifest.dependencies)) {
    const dependencyNames = manifest.dependencies.map((dependency) => dependency?.name).filter(Boolean);
    if (new Set(dependencyNames).size !== dependencyNames.length) fail("dependencies no debe repetir paquetes");
  }
  if (Array.isArray(manifest.exports)) {
    const exportNames = manifest.exports.map((item) => item?.name).filter(Boolean);
    if (new Set(exportNames).size !== exportNames.length) fail("exports no debe repetir nombres");
  }
  if (Array.isArray(manifest.native_bindings)) {
    if (new Set(manifest.native_bindings).size !== manifest.native_bindings.length) fail("native_bindings no debe repetir símbolos");
  }
  return errors;
}

function validatePackage(packageDirectory) {
  const root = path.resolve(packageDirectory);
  const manifestPath = path.join(root, "lmr-module.json");
  if (!fs.existsSync(manifestPath)) return ["no se encontró lmr-module.json"];
  let manifest;
  try {
    manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  } catch (error) {
    return [`JSON inválido: ${error.message}`];
  }
  return validateManifest(root, manifest);
}

if (require.main === module) {
  const target = process.argv[2];
  if (!target) {
    console.error("Uso: node scripts/validate-module-package.js <directorio-del-paquete>");
    process.exitCode = 2;
  } else {
    const errors = validatePackage(target);
    if (errors.length) {
      for (const error of errors) console.error(`❌ ${error}`);
      process.exitCode = 1;
    } else {
      console.log(`✅ Paquete válido: ${path.resolve(target)}`);
    }
  }
}

module.exports = { validateManifest, validatePackage };
