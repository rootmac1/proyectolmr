function blockWords(line, state) {
  const words = [];
  let quote = null;
  let escaped = false;
  let blockComment = state.blockComment;

  for (let i = 0; i < line.length; i += 1) {
    const char = line[i];
    const next = line[i + 1];

    if (blockComment) {
      if (char === "*" && next === "/") {
        blockComment = false;
        i += 1;
      }
      continue;
    }
    if (quote) {
      if (escaped) escaped = false;
      else if (char === "\\") escaped = true;
      else if (char === quote) quote = null;
      continue;
    }
    if ((char === "\"" || char === "'") && !quote) {
      quote = char;
      continue;
    }
    if (char === "/" && next === "/") break;
    if (char === "/" && next === "*") {
      blockComment = true;
      i += 1;
      continue;
    }
    if (/[A-Za-z_]/.test(char)) {
      let end = i + 1;
      while (end < line.length && /[A-Za-z0-9_]/.test(line[end])) end += 1;
      const word = line.slice(i, end);
      if (word === "begin" || word === "end") words.push(word);
      i = end - 1;
    }
  }

  state.blockComment = blockComment;
  return words;
}

function formatLmr(source, options = {}) {
  const tabSize = Number.isInteger(options.tabSize) && options.tabSize > 0 ? options.tabSize : 4;
  const indent = options.insertSpaces === false ? "\t" : " ".repeat(tabSize);
  const lineEnding = source.includes("\r\n") ? "\r\n" : source.includes("\r") && !source.includes("\n") ? "\r" : "\n";
  const lines = source.split(/\r\n|\n|\r/);
  const output = [];
  const state = { blockComment: false };
  let depth = 0;

  for (const line of lines) {
    const content = line.trim();
    if (content.length === 0) {
      output.push("");
      blockWords(line, state);
      continue;
    }

    const words = blockWords(content, state);
    const leadingEnd = /^end\b/.test(content) && words[0] === "end";
    if (leadingEnd) depth = Math.max(0, depth - 1);
    output.push(indent.repeat(depth) + content);

    let skippedLeadingEnd = false;
    for (const word of words) {
      if (leadingEnd && !skippedLeadingEnd && word === "end") {
        skippedLeadingEnd = true;
        continue;
      }
      depth = word === "begin" ? depth + 1 : Math.max(0, depth - 1);
    }
  }

  return output.join(lineEnding);
}

module.exports = { formatLmr };
