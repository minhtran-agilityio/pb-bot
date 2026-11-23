// Run: node --test apps/pod-shape/src/components/app/ArtifactPreviewFrame.test.mjs
// Guards the sandbox of ArtifactPreviewFrame (HANDOFF §3 "Prototype preview"). Reads the source; no dependency.
// FRAME_FILE (env) points the test at another copy of the part, to prove the test fails on a broken one.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const file = process.env.FRAME_FILE ?? new URL("./ArtifactPreviewFrame.tsx", import.meta.url);
const raw = readFileSync(file, "utf8");
// Drop comments so a note cannot hide or fake a match.
const src = raw.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "");

// Parse the FIRST <iframe ...> opening tag into attributes: [{ name, kind: "string"|"expr"|"bare"|"spread", value }].
function parseFirstIframe(text) {
  const start = text.indexOf("<iframe");
  if (start < 0) return null;
  let i = start + "<iframe".length;
  const attrs = [];
  while (i < text.length) {
    while (/\s/.test(text[i])) i++;
    if (text.startsWith("/>", i) || text[i] === ">") return { attrs, text: text.slice(start, i + (text[i] === ">" ? 1 : 2)) };
    if (text[i] === "{") {
      const end = balanced(text, i);
      attrs.push({ name: "{spread}", kind: "spread", value: text.slice(i, end) });
      i = end;
      continue;
    }
    const m = /^[A-Za-z_:][\w:.-]*/.exec(text.slice(i));
    if (!m) return null;
    const name = m[0];
    i += name.length;
    if (text[i] !== "=") {
      attrs.push({ name, kind: "bare", value: null });
      continue;
    }
    i++;
    if (text[i] === '"' || text[i] === "'") {
      const q = text[i];
      const end = text.indexOf(q, i + 1);
      attrs.push({ name, kind: "string", value: text.slice(i + 1, end) });
      i = end + 1;
    } else if (text[i] === "{") {
      const end = balanced(text, i);
      attrs.push({ name, kind: "expr", value: text.slice(i, end) });
      i = end;
    } else return null;
  }
  return null;
}
function balanced(text, i) {
  let depth = 0;
  for (; i < text.length; i++) {
    if (text[i] === "{") depth++;
    else if (text[i] === "}" && --depth === 0) return i + 1;
  }
  return text.length;
}

const frame = parseFirstIframe(src);

test("the part has an iframe element that parses", () => assert.ok(frame, "first <iframe> not found or not parseable"));
test("the first iframe has exactly the attributes className, sandbox, referrerPolicy, src, title", () => {
  assert.deepEqual(frame.attrs.map((a) => a.name).sort(), ["className", "referrerPolicy", "sandbox", "src", "title"]);
});
test('the first iframe sandbox is the string literal "allow-scripts"', () => {
  const s = frame.attrs.find((a) => a.name === "sandbox");
  assert.equal(s.kind, "string");
  assert.equal(s.value, "allow-scripts");
});
test("the first iframe text has no other allow- token", () => {
  const tokens = frame.text.match(/allow-[\w-]*/g) ?? [];
  assert.deepEqual(tokens, ["allow-scripts"]);
});
test("exactly one iframe", () => assert.equal((src.match(/<iframe\b/g) ?? []).length, 1));
test('exactly one sandbox="allow-scripts" in the file', () => assert.equal(src.split('sandbox="allow-scripts"').length - 1, 1));
test("no sandbox={ expression", () => assert.ok(!/sandbox\s*=\s*\{/.test(src)));
test("no other sandbox token anywhere", () => {
  for (const t of ["allow-same-origin", "allow-forms", "allow-popups", "allow-top-navigation"]) {
    assert.ok(!src.includes(t), `${t} found`);
  }
});
test("no srcDoc", () => assert.ok(!/srcDoc/i.test(src)));
test("no props spread anywhere in the part", () => assert.ok(!/\{\s*\.\.\./.test(src)));
// The part is pure JSX: nothing may run at runtime that could change the iframe or listen to it.
test("no runtime import (only import type)", () => {
  const imports = src.match(/^\s*import\b[^\n]*/gm) ?? [];
  for (const l of imports) assert.ok(/^\s*import\s+type\b/.test(l), `runtime import: ${l.trim()}`);
});
test("no hook call, ref, document. or window.", () => {
  assert.ok(!/\buse[A-Z]\w*\s*\(/.test(src), "hook call found");
  assert.ok(!/\bref\s*=/.test(src), "ref= found");
  assert.ok(!/\bdocument\./.test(src), "document. found");
  assert.ok(!/\bwindow\./.test(src), "window. found");
});
test("the word sandbox appears exactly once in the file", () => assert.equal((src.match(/sandbox/g) ?? []).length, 1));
test("no onmessage in any case", () => assert.ok(!/onmessage/i.test(src)));
test('the first iframe referrerPolicy is the string literal "no-referrer"', () => {
  const r = frame.attrs.find((a) => a.name === "referrerPolicy");
  assert.equal(r.kind, "string");
  assert.equal(r.value, "no-referrer");
});
test("no message channel to the page", () => {
  assert.ok(!/addEventListener\(\s*["']message["']/.test(src), 'addEventListener("message" found');
  for (const w of ["onMessage", "postMessage", "contentWindow"]) assert.ok(!src.includes(w), `${w} found`);
});
