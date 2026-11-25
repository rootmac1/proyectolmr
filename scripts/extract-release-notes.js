const fs = require("node:fs");
const path = require("node:path");

function extractReleaseNotes(changelog, tag) {
  if (typeof tag !== "string" || !tag.startsWith("v")) {
    throw new Error("la etiqueta debe comenzar con v");
  }
  const version = tag.slice(1);
  const heading = `## ${version} —`;
  const lines = changelog.split(/\r?\n/);
  const matches = [];
  for (let i = 0; i < lines.length; i += 1) {
    if (lines[i].startsWith(heading)) matches.push(i);
  }
  if (matches.length !== 1) {
    throw new Error(`se esperaba una sección única para ${version} en CHANGELOG.md`);
  }

  const start = matches[0];
  let end = lines.length;
  for (let i = start + 1; i < lines.length; i += 1) {
    if (/^## /.test(lines[i])) {
      end = i;
      break;
    }
  }
  return `${lines.slice(start, end).join("\n").trim()}\n`;
}

if (require.main === module) {
  const tag = process.argv[2];
  if (!tag) {
    console.error("Uso: node scripts/extract-release-notes.js <etiqueta>");
    process.exitCode = 2;
  } else {
    try {
      const changelog = fs.readFileSync(path.join(__dirname, "..", "CHANGELOG.md"), "utf8");
      process.stdout.write(extractReleaseNotes(changelog, tag));
    } catch (error) {
      console.error(`No se pudieron extraer las notas del release: ${error.message}`);
      process.exitCode = 1;
    }
  }
}

module.exports = { extractReleaseNotes };
