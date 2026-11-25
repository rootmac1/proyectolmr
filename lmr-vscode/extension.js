const vscode = require("vscode");
const { spawn } = require("child_process");
const fs = require("fs");
const path = require("path");
const { formatLmr } = require("./formatter");

const diagnostics = vscode.languages.createDiagnosticCollection("lmr");
const activeChecks = new Map();

const keywordCompletions = [
  "begin", "end", "if", "then", "else", "while", "do", "for", "to", "in",
  "func", "return", "const", "import", "from", "array", "map", "class",
  "new", "extends", "override", "record", "enum", "extern", "try", "catch",
  "throw", "spawn", "and", "or", "not", "true", "false"
];

const builtinSignatures = [
  "write(...)", "writeln(...)", "readln()", "len(value)", "contains(map, key)",
  "intToString(value)", "floatToString(value)", "toUpper(text)", "toLower(text)",
  "SliceString(text, start[, end])", "posString(text, search)", "readFile(path)",
  "writeFile(path, contents)", "sleep(milliseconds)", "sqrt(value)", "pow(base, exponent)",
  "abs(value)", "sin(radians)", "cos(radians)", "tan(radians)", "asin(value)",
  "acos(value)", "atan(value)", "atan2(y, x)", "exp(value)", "log(value)",
  "log10(value)", "floor(value)", "ceil(value)", "round(value)", "fmod(value, divisor)",
  "jsonParse(text)", "dispose(value)"
];

function moduleExports() {
  try {
    const manifestPath = path.join(__dirname, "modules", "manifest.json");
    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
    return manifest.modules.flatMap((module) =>
      module.exports.map((item) => ({
        name: item.name,
        kind: item.kind,
        signature: item.signature || `${item.name}`,
        module: module.id
      }))
    );
  } catch (_) {
    return [];
  }
}

function provideCompletions() {
  const items = [];
  for (const word of keywordCompletions) {
    const item = new vscode.CompletionItem(word, vscode.CompletionItemKind.Keyword);
    item.detail = "Palabra clave de LMR";
    items.push(item);
  }
  for (const signature of builtinSignatures) {
    const name = signature.slice(0, signature.indexOf("("));
    const item = new vscode.CompletionItem(name, vscode.CompletionItemKind.Function);
    item.detail = signature;
    item.documentation = new vscode.MarkdownString("Función incorporada de LMR.");
    items.push(item);
  }
  for (const exported of moduleExports()) {
    const kind = exported.kind === "class"
      ? vscode.CompletionItemKind.Class
      : exported.kind === "record"
        ? vscode.CompletionItemKind.Struct
        : vscode.CompletionItemKind.Function;
    const item = new vscode.CompletionItem(exported.name, kind);
    item.detail = exported.signature;
    item.documentation = new vscode.MarkdownString(`Módulo **${exported.module}**.`);
    items.push(item);
  }
  return items;
}

function diagnosticFromLine(line, document) {
  const match = line.match(/Error\s+(léxico|sintáctico|semántico|interno)\s+en\s+(.+):(\d+):(\d+):\s*(.*)/i);
  if (!match) return null;

  const [, category, sourcePath, lineText, columnText, detail] = match;
  const lineNumber = Math.max(1, Number(lineText));
  const columnNumber = Math.max(1, Number(columnText));
  const diagnosticPath = path.resolve(sourcePath.trim());
  const currentPath = path.resolve(document.uri.fsPath);
  const sameFile = process.platform === "win32"
    ? diagnosticPath.toLowerCase() === currentPath.toLowerCase()
    : diagnosticPath === currentPath;
  const message = sameFile
    ? `${category}: ${detail || line.trim()}`
    : `${category} en ${sourcePath.trim()}: ${detail || line.trim()}`;
  const position = sameFile
    ? new vscode.Position(lineNumber - 1, columnNumber - 1)
    : new vscode.Position(0, 0);
  const range = sameFile
    ? new vscode.Range(position, position.translate(0, 1))
    : new vscode.Range(position, position);
  return new vscode.Diagnostic(range, message, vscode.DiagnosticSeverity.Error);
}

function scheduleDiagnostics(document) {
  if (document.languageId !== "lmr" || document.uri.scheme !== "file") return;
  const uri = document.uri.toString();
  const previous = activeChecks.get(uri);
  if (previous) {
    clearTimeout(previous.timer);
    if (previous.child) previous.child.kill();
  }

  if (!vscode.workspace.getConfiguration("lmr").get("diagnosticsOnType", true)) {
    activeChecks.delete(uri);
    diagnostics.delete(document.uri);
    return;
  }

  const state = { timer: null, child: null };
  activeChecks.set(uri, state);
  state.timer = setTimeout(() => {
    const compiler = vscode.workspace.getConfiguration("lmr").get("compilerPath", "lmr");
    const workspace = vscode.workspace.getWorkspaceFolder(document.uri);
    const child = spawn(compiler, ["check", "--stdin", document.uri.fsPath], {
      cwd: workspace ? workspace.uri.fsPath : path.dirname(document.uri.fsPath),
      windowsHide: true
    });
    state.child = child;
    let output = "";
    child.stdout.setEncoding("utf8").on("data", (chunk) => { output += chunk; });
    child.stderr.setEncoding("utf8").on("data", (chunk) => { output += chunk; });
    child.stdin.end(document.getText());

    child.on("error", (error) => {
      if (activeChecks.get(uri) !== state) return;
      activeChecks.delete(uri);
      diagnostics.set(document.uri, [new vscode.Diagnostic(
        new vscode.Range(0, 0, 0, 1),
        `No se pudo iniciar LMR (${compiler}): ${error.message}. Configura lmr.compilerPath.`,
        vscode.DiagnosticSeverity.Error
      )]);
    });

    child.on("close", (code) => {
      if (activeChecks.get(uri) !== state) return;
      activeChecks.delete(uri);
      const parsed = output.split(/\r?\n/)
        .map((line) => diagnosticFromLine(line, document))
        .filter(Boolean);
      if (code !== 0 && parsed.length === 0) {
        const message = output.trim() || `LMR terminó con código ${code}`;
        parsed.push(new vscode.Diagnostic(
          new vscode.Range(0, 0, 0, 1),
          message,
          vscode.DiagnosticSeverity.Error
        ));
      }
      diagnostics.set(document.uri, parsed);
    });
  }, 450);
}

function activate(context) {
  context.subscriptions.push(diagnostics);
  context.subscriptions.push(vscode.languages.registerCompletionItemProvider(
    { language: "lmr" },
    { provideCompletionItems },
    ".",
    "("
  ));
  context.subscriptions.push(vscode.languages.registerDocumentFormattingEditProvider(
    { language: "lmr" },
    {
      provideDocumentFormattingEdits(document, options) {
        const source = document.getText();
        const formatted = formatLmr(source, options);
        if (source === formatted) return [];
        const fullRange = new vscode.Range(document.positionAt(0), document.positionAt(source.length));
        return [vscode.TextEdit.replace(fullRange, formatted)];
      }
    }
  ));
  context.subscriptions.push(vscode.workspace.onDidOpenTextDocument(scheduleDiagnostics));
  context.subscriptions.push(vscode.workspace.onDidSaveTextDocument(scheduleDiagnostics));
  context.subscriptions.push(vscode.workspace.onDidChangeTextDocument((event) => {
    scheduleDiagnostics(event.document);
  }));
  context.subscriptions.push(vscode.workspace.onDidCloseTextDocument((document) => {
    const uri = document.uri.toString();
    const active = activeChecks.get(uri);
    if (active) {
      clearTimeout(active.timer);
      if (active.child) active.child.kill();
      activeChecks.delete(uri);
    }
    diagnostics.delete(document.uri);
  }));

  for (const document of vscode.workspace.textDocuments) scheduleDiagnostics(document);
}

function deactivate() {
  for (const active of activeChecks.values()) {
    clearTimeout(active.timer);
    if (active.child) active.child.kill();
  }
  activeChecks.clear();
}

module.exports = { activate, deactivate };
