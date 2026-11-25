const test = require("node:test");
const assert = require("node:assert/strict");
const { formatLmr } = require("../formatter");

test("indents nested begin/end blocks", () => {
  const source = "begin\nint n\nif n == 1 then\nbegin\nwriteln(n)\nend\nend.";
  const expected = "begin\n    int n\n    if n == 1 then\n    begin\n        writeln(n)\n    end\nend.";
  assert.equal(formatLmr(source), expected);
});

test("ignores block keywords inside strings and comments", () => {
  const source = "begin\nstring s\ns = \"end begin\" // begin\n/* end\n   begin */\nwriteln(s)\nend.";
  const expected = "begin\n    string s\n    s = \"end begin\" // begin\n    /* end\n    begin */\n    writeln(s)\nend.";
  assert.equal(formatLmr(source), expected);
});

test("preserves tab indentation settings and trailing newline", () => {
  assert.equal(formatLmr("begin\nvalue\nend.\n", { insertSpaces: false }), "begin\n\tvalue\nend.\n");
});

test("preserves CRLF line endings", () => {
  assert.equal(formatLmr("begin\r\nvalue\r\nend.\r\n"), "begin\r\n    value\r\nend.\r\n");
});

test("formatting is idempotent", () => {
  const source = "begin\n    if true then\n    begin\n        writeln(\"ok\")\n    end\nend.\n";
  assert.equal(formatLmr(formatLmr(source)), formatLmr(source));
});
