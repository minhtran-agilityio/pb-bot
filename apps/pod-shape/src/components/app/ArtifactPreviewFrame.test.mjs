// Run: node --test apps/pod-shape/src/components/app/ArtifactPreviewFrame.test.mjs
// Guards the sandbox of ArtifactPreviewFrame (HANDOFF §3 "Prototype preview"). Reads the source; no dependency.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const file = process.env.FRAME_FILE ?? new URL("./ArtifactPreviewFrame.tsx", import.meta.url);
const raw = readFileSync(file, "utf8");
// Drop comments so a note cannot hide or fake a match.
const src = raw.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "");
const iframe = src.match(/<iframe[\s\S]*?\/>/g) ?? [];

test("exactly one iframe", () => assert.equal(iframe.length, 1));
test('exactly one sandbox="allow-scripts"', () => assert.equal(src.split('sandbox="allow-scripts"').length - 1, 1));
test("no sandbox={ expression", () => assert.ok(!/sandbox\s*=\s*\{/.test(src)));
test("no other sandbox token", () => {
  for (const t of ["allow-same-origin", "allow-forms", "allow-popups", "allow-top-navigation"]) {
    assert.ok(!src.includes(t), `${t} found`);
  }
});
test("no srcDoc", () => assert.ok(!/srcDoc/i.test(src)));
test("no spread on the iframe", () => assert.ok(!iframe.some((e) => /\{\s*\.\.\./.test(e))));
test("no props spread anywhere in the part", () => assert.ok(!/\{\s*\.\.\./.test(src)));
